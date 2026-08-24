//! SPEC-004 section 4 Runtime discovery and endpoint security (macOS).
//!
//! Resolves the OS-provided per-user runtime directory rather than trusting
//! an arbitrary environment path, verifies it with no-follow filesystem
//! operations before use, and defines the control-socket path inside it.
//! This module never trusts a symlink, a PID text file, or attacker-
//! supplied metadata as proof of Runtime identity.

use std::{
    ffi::{CString, OsString},
    fs::{DirBuilder, Metadata},
    io,
    os::unix::{
        ffi::OsStringExt,
        fs::{DirBuilderExt, MetadataExt},
    },
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub enum DiscoveryError {
    ConfstrFailed,
    NotADirectory,
    NotOwnedByEffectiveUser,
    GroupOrWorldWritable,
    Io(io::Error),
    PathTooLongForSocket,
}

impl From<io::Error> for DiscoveryError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

/// Darwin `sockaddr_un.sun_path` capacity, including the terminating NUL.
const SUN_PATH_MAX: usize = 104;

pub const CONTROL_SOCKET_NAME: &str = "control.sock";

/// Resolves `$DARWIN_USER_TEMP_DIR/seyal-runtime`, the verified per-user
/// runtime directory that owns the control socket. Never reads
/// `TMPDIR`/`$HOME`-derived environment values as authority; only the OS
/// `confstr(3)` mechanism is trusted (SPEC-004 section 4.1).
#[cfg(target_os = "macos")]
pub fn darwin_user_runtime_dir() -> Result<PathBuf, DiscoveryError> {
    // SAFETY: `confstr` is called first with a null buffer to discover the
    // required length (a standard two-call confstr idiom), then with a
    // correctly sized owned buffer; both calls only write into memory this
    // function owns.
    let required =
        unsafe { libc::confstr(libc::_CS_DARWIN_USER_TEMP_DIR, std::ptr::null_mut(), 0) };
    if required == 0 {
        return Err(DiscoveryError::ConfstrFailed);
    }
    let mut buffer = vec![0u8; required];
    // SAFETY: `buffer` is exactly `required` bytes, matching the size
    // `confstr` reported it needs (including the NUL terminator).
    let written = unsafe {
        libc::confstr(
            libc::_CS_DARWIN_USER_TEMP_DIR,
            buffer.as_mut_ptr().cast::<libc::c_char>(),
            buffer.len(),
        )
    };
    if written == 0 || written > buffer.len() {
        return Err(DiscoveryError::ConfstrFailed);
    }
    // Trim the NUL terminator(s) confstr wrote.
    if let Some(nul_index) = buffer.iter().position(|&b| b == 0) {
        buffer.truncate(nul_index);
    }
    let path = PathBuf::from(OsString::from_vec(buffer));
    Ok(path.join("seyal-runtime"))
}

/// Verifies (and creates if absent) the runtime directory with no-follow
/// filesystem operations, rejecting a symlink leaf, wrong ownership, or
/// group/world-writable mode (SPEC-004 section 4.1).
pub fn ensure_verified_runtime_dir(dir: &Path) -> Result<(), DiscoveryError> {
    match std::fs::symlink_metadata(dir) {
        Ok(metadata) => verify_directory_metadata(&metadata)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let mut builder = DirBuilder::new();
            builder.recursive(false).mode(0o700);
            // A racing concurrent creator is treated as success only if the
            // resulting metadata still verifies below.
            match builder.create(dir) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(DiscoveryError::Io(error)),
            }
            let metadata = std::fs::symlink_metadata(dir)?;
            verify_directory_metadata(&metadata)?;
        }
        Err(error) => return Err(DiscoveryError::Io(error)),
    }
    Ok(())
}

fn verify_directory_metadata(metadata: &Metadata) -> Result<(), DiscoveryError> {
    if metadata.is_symlink() || !metadata.is_dir() {
        return Err(DiscoveryError::NotADirectory);
    }
    // SAFETY-relevant policy check (not memory safety): reject any
    // ownership other than the current effective UID so a pre-staged
    // directory owned by another local user is never trusted.
    // SAFETY: no unsafe code; `libc::geteuid` is always sound to call.
    let effective_uid = unsafe { libc::geteuid() };
    if metadata.uid() != effective_uid {
        return Err(DiscoveryError::NotOwnedByEffectiveUser);
    }
    if metadata.mode() & 0o077 != 0 {
        return Err(DiscoveryError::GroupOrWorldWritable);
    }
    Ok(())
}

/// Builds the control-socket path inside `runtime_dir`, failing explicitly
/// (never truncating) if it would not fit `sockaddr_un.sun_path`.
pub fn control_socket_path(runtime_dir: &Path) -> Result<PathBuf, DiscoveryError> {
    let path = runtime_dir.join(CONTROL_SOCKET_NAME);
    let as_cstring = CString::new(path.as_os_str().as_encoded_bytes().to_vec())
        .map_err(|_| DiscoveryError::PathTooLongForSocket)?;
    if as_cstring.as_bytes_with_nul().len() > SUN_PATH_MAX {
        return Err(DiscoveryError::PathTooLongForSocket);
    }
    Ok(path)
}

/// Verifies a candidate stale-socket path is a socket owned by the current
/// effective UID (never following a symlink) before it may be unlinked as
/// stale (SPEC-004 section 4.2). An unexpected type/ownership is a hard
/// security failure, not silently treated as "stale".
pub fn verify_stale_socket_before_unlink(path: &Path) -> Result<(), DiscoveryError> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(DiscoveryError::Io(error)),
    };
    if metadata.is_symlink() {
        return Err(DiscoveryError::NotADirectory);
    }
    use std::os::unix::fs::FileTypeExt;
    if !metadata.file_type().is_socket() {
        return Err(DiscoveryError::NotADirectory);
    }
    // SAFETY: no unsafe code; always sound to call.
    let effective_uid = unsafe { libc::geteuid() };
    if metadata.uid() != effective_uid {
        return Err(DiscoveryError::NotOwnedByEffectiveUser);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn temp_scope(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "seyal-discovery-test-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        path
    }

    #[test]
    fn darwin_user_runtime_dir_resolves_a_nonempty_path() {
        let dir = darwin_user_runtime_dir().unwrap();
        assert!(dir.ends_with("seyal-runtime"));
        assert!(dir.as_os_str().len() > "seyal-runtime".len());
    }

    #[test]
    fn ensure_verified_runtime_dir_creates_a_0700_directory() {
        let dir = temp_scope("create");
        ensure_verified_runtime_dir(&dir).unwrap();
        let metadata = std::fs::symlink_metadata(&dir).unwrap();
        assert!(metadata.is_dir());
        assert_eq!(metadata.mode() & 0o777, 0o700);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn ensure_verified_runtime_dir_rejects_a_symlink_leaf() {
        let real_dir = temp_scope("real");
        std::fs::create_dir(&real_dir).unwrap();
        let symlink_path = temp_scope("symlink");
        std::os::unix::fs::symlink(&real_dir, &symlink_path).unwrap();

        let result = ensure_verified_runtime_dir(&symlink_path);
        assert!(matches!(result, Err(DiscoveryError::NotADirectory)));

        std::fs::remove_file(&symlink_path).unwrap();
        std::fs::remove_dir_all(&real_dir).unwrap();
    }

    #[test]
    fn ensure_verified_runtime_dir_rejects_group_writable_existing_directory() {
        let dir = temp_scope("group-writable");
        let mut builder = DirBuilder::new();
        builder.recursive(false).mode(0o770);
        builder.create(&dir).unwrap();
        // `DirBuilder::mode` is subject to umask, so force the bits.
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o770)).unwrap();

        let result = ensure_verified_runtime_dir(&dir);
        assert!(matches!(result, Err(DiscoveryError::GroupOrWorldWritable)));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn control_socket_path_rejects_path_exceeding_sun_path_capacity() {
        let too_long = PathBuf::from("/").join("a".repeat(200));
        let result = control_socket_path(&too_long);
        assert!(matches!(result, Err(DiscoveryError::PathTooLongForSocket)));
    }

    #[test]
    fn verify_stale_socket_before_unlink_accepts_missing_path() {
        let dir = temp_scope("missing");
        assert!(verify_stale_socket_before_unlink(&dir).is_ok());
    }

    #[test]
    fn verify_stale_socket_before_unlink_rejects_symlink() {
        let real_dir = temp_scope("real-for-stale");
        std::fs::create_dir(&real_dir).unwrap();
        let symlink_path = temp_scope("stale-symlink");
        std::os::unix::fs::symlink(&real_dir, &symlink_path).unwrap();

        let result = verify_stale_socket_before_unlink(&symlink_path);
        assert!(matches!(result, Err(DiscoveryError::NotADirectory)));

        std::fs::remove_file(&symlink_path).unwrap();
        std::fs::remove_dir_all(&real_dir).unwrap();
    }

    #[test]
    fn verify_stale_socket_before_unlink_rejects_non_socket_regular_file() {
        let path = temp_scope("regular-file");
        std::fs::write(&path, b"not a socket").unwrap();

        let result = verify_stale_socket_before_unlink(&path);
        assert!(matches!(result, Err(DiscoveryError::NotADirectory)));

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn verify_stale_socket_before_unlink_accepts_an_owned_socket() {
        // Keep this path short: Darwin's `sockaddr_un.sun_path` capacity is
        // tiny (~104 bytes including the NUL terminator).
        let mut path = std::env::temp_dir();
        path.push(format!("syl{}.sock", std::process::id() % 100_000));
        let listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
        assert!(verify_stale_socket_before_unlink(&path).is_ok());
        drop(listener);
        std::fs::remove_file(&path).ok();
    }
}
