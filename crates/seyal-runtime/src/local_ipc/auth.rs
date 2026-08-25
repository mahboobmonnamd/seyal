//! SPEC-004 section 4.3 peer authentication (macOS).
//!
//! Immediately after `accept`, before any client frame is acted upon, the
//! Runtime obtains peer credentials via the kernel `getpeereid` mechanism
//! and rejects a UID mismatch without creating any attachment.

use std::os::fd::RawFd;

#[derive(Debug)]
pub struct PeerCredentialError(std::io::Error);

impl From<PeerCredentialError> for std::io::Error {
    fn from(value: PeerCredentialError) -> Self {
        value.0
    }
}

/// Returns the peer effective UID of a connected Unix-domain socket using
/// the Darwin kernel peer-credential facility.
pub fn peer_effective_uid(socket: RawFd) -> Result<libc::uid_t, PeerCredentialError> {
    let mut euid: libc::uid_t = 0;
    let mut egid: libc::gid_t = 0;
    // SAFETY: `socket` is a live, connected Unix-domain socket descriptor
    // owned by the caller for the duration of this call; `getpeereid` only
    // writes into the two local out-parameters.
    let result = unsafe { libc::getpeereid(socket, &mut euid, &mut egid) };
    if result != 0 {
        return Err(PeerCredentialError(std::io::Error::last_os_error()));
    }
    Ok(euid)
}

/// Verifies the connected peer's effective UID equals the Runtime's own
/// effective UID. A mismatch (or a credential-lookup failure) must reject
/// the connection before any attachment is created (SPEC-004 section 4.3).
pub fn verify_same_user_peer(socket: RawFd) -> Result<(), PeerCredentialError> {
    let peer_uid = peer_effective_uid(socket)?;
    // SAFETY: always sound to call; reads only the calling process's own
    // credentials.
    let own_uid = unsafe { libc::geteuid() };
    if peer_uid != own_uid {
        return Err(PeerCredentialError(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "peer effective UID does not match Runtime effective UID",
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::fd::AsRawFd;
    use std::os::unix::net::UnixStream;

    #[test]
    fn same_process_socketpair_reports_own_effective_uid() {
        let (a, _b) = UnixStream::pair().unwrap();
        let uid = peer_effective_uid(a.as_raw_fd()).unwrap();
        // SAFETY: always sound to call.
        let own_uid = unsafe { libc::geteuid() };
        assert_eq!(uid, own_uid);
    }

    #[test]
    fn verify_same_user_peer_accepts_a_same_process_socketpair() {
        let (a, _b) = UnixStream::pair().unwrap();
        assert!(verify_same_user_peer(a.as_raw_fd()).is_ok());
    }

    #[test]
    fn peer_effective_uid_rejects_a_non_socket_descriptor() {
        // A regular file is a valid, open, live descriptor that is
        // definitely not a socket; `getpeereid` must fail cleanly rather
        // than panic or read out of bounds.
        let file = std::fs::File::open("/dev/null").unwrap();
        let result = peer_effective_uid(file.as_raw_fd());
        assert!(result.is_err());
    }
}
