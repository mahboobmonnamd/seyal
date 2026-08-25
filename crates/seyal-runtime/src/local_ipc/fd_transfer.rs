//! `SCM_RIGHTS` file-descriptor passing over a Unix-domain socket (macOS).
//!
//! Isolates the raw `sendmsg`/`recvmsg` ancillary-data plumbing SPEC-004
//! requires for projection descriptor transfer (section 4/8) to this one
//! module. Every unsafe block is narrowly scoped and documented.

use std::{
    io,
    os::fd::{FromRawFd, OwnedFd, RawFd},
};

const ANCILLARY_BUFFER_BYTES: usize = 256;

// `cmsghdr` requires native alignment. Keep the stack buffer more strictly
// aligned than Darwin requires so the CMSG macros never observe an unaligned
// control header.
#[repr(align(16))]
struct AncillaryBuffer([u8; ANCILLARY_BUFFER_BYTES]);

impl AncillaryBuffer {
    fn new() -> Self {
        Self([0; ANCILLARY_BUFFER_BYTES])
    }
}

fn cmsg_space_for_fd_count(count: usize) -> usize {
    let bytes = count
        .checked_mul(std::mem::size_of::<RawFd>())
        .expect("SCM_RIGHTS descriptor count overflow");
    let bytes = u32::try_from(bytes).expect("SCM_RIGHTS payload exceeds Darwin CMSG limit");
    // SAFETY: `CMSG_SPACE` only computes a size from its integer argument;
    // it performs no memory access.
    unsafe { libc::CMSG_SPACE(bytes) as usize }
}

fn cmsg_len(payload_bytes: usize) -> usize {
    let payload_bytes =
        u32::try_from(payload_bytes).expect("SCM_RIGHTS payload exceeds Darwin CMSG limit");
    // SAFETY: `CMSG_LEN` only computes a size from its integer argument;
    // it performs no memory access.
    unsafe { libc::CMSG_LEN(payload_bytes) as usize }
}

/// Sends `bytes` on `socket`, optionally attaching exactly one file
/// descriptor as an `SCM_RIGHTS` ancillary message in the same stream send.
///
/// The common no-descriptor path intentionally uses `send(2)` directly and
/// performs no ancillary-buffer allocation. Projection generation wakes use
/// this path, so ordinary display notification does not allocate merely to
/// construct an empty control message.
pub fn send_with_fd(socket: RawFd, bytes: &[u8], fd: Option<RawFd>) -> io::Result<usize> {
    #[cfg(feature = "test-fault-injection")]
    if fd.is_some()
        && crate::test_fault::take(crate::test_fault::FaultPoint::SendAttachedDescriptor)
    {
        return Err(io::Error::other(
            "injected Pass-5 Attached descriptor send failure",
        ));
    }

    let Some(fd) = fd else {
        // SAFETY: `bytes` is a valid slice for the duration of the call and
        // `socket` is a live caller-owned descriptor.
        let result = unsafe {
            libc::send(
                socket,
                bytes.as_ptr().cast::<libc::c_void>(),
                bytes.len(),
                0,
            )
        };
        if result < 0 {
            return Err(io::Error::last_os_error());
        }
        return Ok(result as usize);
    };

    let control_len = cmsg_space_for_fd_count(1);
    if control_len > ANCILLARY_BUFFER_BYTES {
        return Err(io::Error::other(
            "SCM_RIGHTS control message exceeds fixed ancillary buffer",
        ));
    }

    let mut iov = libc::iovec {
        iov_base: bytes.as_ptr() as *mut libc::c_void,
        iov_len: bytes.len(),
    };
    let mut cmsg_buffer = AncillaryBuffer::new();
    // SAFETY: `libc::msghdr` is a plain-old-data C struct; a zeroed value is
    // a valid empty starting point and all fields consumed by sendmsg are set
    // below before use.
    let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;
    msg.msg_control = cmsg_buffer.0.as_mut_ptr().cast::<libc::c_void>();
    msg.msg_controllen = control_len as _;

    // SAFETY: the control buffer is aligned and sized by CMSG_SPACE for one
    // `RawFd`, so CMSG_FIRSTHDR returns either a pointer inside it or null.
    let cmsg = unsafe { libc::CMSG_FIRSTHDR(&msg) };
    if cmsg.is_null() {
        return Err(io::Error::other(
            "SCM_RIGHTS control buffer did not produce a cmsghdr",
        ));
    }
    // SAFETY: `cmsg` points into the aligned buffer above, which has room for
    // exactly one descriptor payload.
    unsafe {
        (*cmsg).cmsg_level = libc::SOL_SOCKET;
        (*cmsg).cmsg_type = libc::SCM_RIGHTS;
        (*cmsg).cmsg_len = cmsg_len(std::mem::size_of::<RawFd>()) as _;
        std::ptr::write_unaligned(libc::CMSG_DATA(cmsg).cast::<RawFd>(), fd);
    }

    // SAFETY: `msg` is fully initialized above and `socket` remains owned by
    // the caller.
    let result = unsafe { libc::sendmsg(socket, &msg, 0) };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(result as usize)
}

/// Receives into `buffer`, returning the byte count and at most one
/// transferred descriptor. Anything other than zero or one complete
/// `SCM_RIGHTS` descriptor is returned as [`RecvFd::Malformed`]. Every
/// descriptor delivered by the kernel on a malformed/truncated path is owned
/// and closed before this function returns.
///
/// Runtime uses this same receive primitive for C→Runtime traffic even though
/// descriptors are never legal in that direction; the connection layer treats
/// both [`RecvFd::One`] and [`RecvFd::Malformed`] as protocol-fatal.
pub fn recv_with_fd(socket: RawFd, buffer: &mut [u8]) -> io::Result<(usize, RecvFd)> {
    let mut iov = libc::iovec {
        iov_base: buffer.as_mut_ptr().cast::<libc::c_void>(),
        iov_len: buffer.len(),
    };
    let mut cmsg_buffer = AncillaryBuffer::new();
    // SAFETY: see `send_with_fd`'s identical zeroing rationale.
    let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;
    msg.msg_control = cmsg_buffer.0.as_mut_ptr().cast::<libc::c_void>();
    msg.msg_controllen = cmsg_buffer.0.len() as _;

    // SAFETY: `msg` is fully initialized with an aligned control buffer and
    // `socket` is a live caller-owned descriptor.
    let result = unsafe { libc::recvmsg(socket, &mut msg, 0) };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }

    let mut first_fd: Option<OwnedFd> = None;
    let mut received_count = 0usize;
    let mut malformed = msg.msg_flags & libc::MSG_CTRUNC != 0;
    let header_len = cmsg_len(0);

    // Walk even when MSG_CTRUNC is set so every complete descriptor the
    // kernel did place in our buffer is closed before reporting malformed.
    // SAFETY: `msg` was just populated by a successful recvmsg and its control
    // pointer/length still refer to `cmsg_buffer`.
    let mut cmsg = unsafe { libc::CMSG_FIRSTHDR(&msg) };
    while !cmsg.is_null() {
        // SAFETY: `cmsg` comes from the CMSG walker for `msg`'s live buffer.
        let (level, kind, len) = unsafe {
            (
                (*cmsg).cmsg_level,
                (*cmsg).cmsg_type,
                (*cmsg).cmsg_len as usize,
            )
        };
        if level == libc::SOL_SOCKET && kind == libc::SCM_RIGHTS {
            if len < header_len {
                malformed = true;
            } else {
                let payload_bytes = len - header_len;
                let fd_bytes = std::mem::size_of::<RawFd>();
                if payload_bytes == 0 || !payload_bytes.is_multiple_of(fd_bytes) {
                    malformed = true;
                }
                let count = payload_bytes / fd_bytes;
                for index in 0..count {
                    // SAFETY: `index` stays inside the complete descriptor
                    // payload described by this cmsghdr.
                    let raw_fd = unsafe {
                        std::ptr::read_unaligned(
                            libc::CMSG_DATA(cmsg)
                                .cast::<u8>()
                                .add(index * fd_bytes)
                                .cast::<RawFd>(),
                        )
                    };
                    if raw_fd < 0 {
                        malformed = true;
                        continue;
                    }
                    // SAFETY: SCM_RIGHTS returns a fresh descriptor owned by
                    // this process; wrapping it immediately gives deterministic
                    // close-on-drop behavior on every rejection path.
                    let owned = unsafe { OwnedFd::from_raw_fd(raw_fd) };
                    received_count += 1;
                    if received_count == 1 {
                        first_fd = Some(owned);
                    } else {
                        malformed = true;
                        drop(owned);
                    }
                }
            }
        }
        // SAFETY: `cmsg` is a valid pointer produced by the same CMSG walker.
        cmsg = unsafe { libc::CMSG_NXTHDR(&msg, cmsg) };
    }

    if malformed {
        drop(first_fd);
        return Ok((result as usize, RecvFd::Malformed));
    }

    match (received_count, first_fd) {
        (0, None) => Ok((result as usize, RecvFd::None)),
        (1, Some(fd)) => Ok((result as usize, RecvFd::One(fd))),
        _ => Ok((result as usize, RecvFd::Malformed)),
    }
}

pub enum RecvFd {
    None,
    One(OwnedFd),
    /// More than one descriptor, a truncated ancillary set or a malformed
    /// descriptor control message was received. All received descriptors are
    /// already closed.
    Malformed,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::fd::AsRawFd;
    use std::os::unix::net::UnixStream;

    fn send_fds(socket: RawFd, bytes: &[u8], fds: &[RawFd]) -> io::Result<usize> {
        let control_len = cmsg_space_for_fd_count(fds.len());
        assert!(control_len <= ANCILLARY_BUFFER_BYTES);
        let mut iov = libc::iovec {
            iov_base: bytes.as_ptr() as *mut libc::c_void,
            iov_len: bytes.len(),
        };
        let mut control = AncillaryBuffer::new();
        // SAFETY: zeroed msghdr is initialized before sendmsg below.
        let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
        msg.msg_iov = &mut iov;
        msg.msg_iovlen = 1;
        msg.msg_control = control.0.as_mut_ptr().cast();
        msg.msg_controllen = control_len as _;
        // SAFETY: aligned control buffer is sized by CMSG_SPACE for `fds`.
        let cmsg = unsafe { libc::CMSG_FIRSTHDR(&msg) };
        assert!(!cmsg.is_null());
        // SAFETY: CMSG_DATA has room for exactly `fds.len()` RawFd values.
        unsafe {
            (*cmsg).cmsg_level = libc::SOL_SOCKET;
            (*cmsg).cmsg_type = libc::SCM_RIGHTS;
            (*cmsg).cmsg_len = cmsg_len(std::mem::size_of_val(fds)) as _;
            for (index, fd) in fds.iter().copied().enumerate() {
                std::ptr::write_unaligned(
                    libc::CMSG_DATA(cmsg)
                        .cast::<u8>()
                        .add(index * std::mem::size_of::<RawFd>())
                        .cast::<RawFd>(),
                    fd,
                );
            }
        }
        // SAFETY: msghdr/control payload are fully initialized above.
        let result = unsafe { libc::sendmsg(socket, &msg, 0) };
        if result < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(result as usize)
        }
    }

    #[test]
    fn send_and_recv_round_trip_bytes_without_a_descriptor() {
        let (a, b) = UnixStream::pair().unwrap();
        let sent = send_with_fd(a.as_raw_fd(), b"hello", None).unwrap();
        assert_eq!(sent, 5);
        let mut buffer = [0u8; 16];
        let (received, fd) = recv_with_fd(b.as_raw_fd(), &mut buffer).unwrap();
        assert_eq!(&buffer[..received], b"hello");
        assert!(matches!(fd, RecvFd::None));
    }

    #[test]
    fn send_and_recv_round_trip_a_single_transferred_descriptor() {
        let (a, b) = UnixStream::pair().unwrap();
        let file = std::fs::File::open("/dev/null").unwrap();
        send_with_fd(a.as_raw_fd(), b"fd", Some(file.as_raw_fd())).unwrap();

        let mut buffer = [0u8; 16];
        let (received, fd) = recv_with_fd(b.as_raw_fd(), &mut buffer).unwrap();
        assert_eq!(&buffer[..received], b"fd");
        match fd {
            RecvFd::One(owned) => {
                assert_ne!(owned.as_raw_fd(), file.as_raw_fd());
            }
            _ => panic!("expected exactly one transferred descriptor"),
        }
    }

    #[test]
    fn multiple_transferred_descriptors_are_rejected() {
        let (a, b) = UnixStream::pair().unwrap();
        let first = std::fs::File::open("/dev/null").unwrap();
        let second = std::fs::File::open("/dev/null").unwrap();
        send_fds(
            a.as_raw_fd(),
            b"fds",
            &[first.as_raw_fd(), second.as_raw_fd()],
        )
        .unwrap();

        let mut buffer = [0u8; 16];
        let (received, fd) = recv_with_fd(b.as_raw_fd(), &mut buffer).unwrap();
        assert_eq!(&buffer[..received], b"fds");
        assert!(matches!(fd, RecvFd::Malformed));
    }
}
