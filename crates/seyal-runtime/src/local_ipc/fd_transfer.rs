//! `SCM_RIGHTS` file-descriptor passing over a Unix-domain socket (macOS).
//!
//! Isolates the raw `sendmsg`/`recvmsg` ancillary-data plumbing SPEC-004
//! requires for projection descriptor transfer (section 4/8) to this one
//! module. Every unsafe block is narrowly scoped and documented.

use std::{
    io,
    os::fd::{FromRawFd, OwnedFd, RawFd},
};

fn cmsg_space_for_one_fd() -> usize {
    // SAFETY: `CMSG_SPACE` only computes a size from its integer argument;
    // it performs no memory access.
    unsafe { libc::CMSG_SPACE(std::mem::size_of::<RawFd>() as u32) as usize }
}

/// Sends `bytes` on `socket`, optionally attaching exactly one file
/// descriptor as an `SCM_RIGHTS` ancillary message in the same datagram of
/// control data (SPEC-004 requires exactly one descriptor per
/// `Attached`/`ProjectionReplaced` frame, never more/fewer).
pub fn send_with_fd(socket: RawFd, bytes: &[u8], fd: Option<RawFd>) -> io::Result<usize> {
    let mut iov = libc::iovec {
        iov_base: bytes.as_ptr() as *mut libc::c_void,
        iov_len: bytes.len(),
    };

    let mut cmsg_buffer = vec![0u8; cmsg_space_for_one_fd()];
    // SAFETY: `libc::msghdr` is a plain-old-data C struct; a zeroed value
    // is a valid (empty) starting point that every field below is then set
    // on explicitly before use.
    let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;

    if let Some(fd) = fd {
        msg.msg_control = cmsg_buffer.as_mut_ptr().cast::<libc::c_void>();
        msg.msg_controllen = cmsg_buffer.len() as libc::socklen_t;

        // SAFETY: `msg.msg_control` points at `cmsg_buffer`, sized by
        // `CMSG_SPACE(size_of::<RawFd>())` above, so `CMSG_FIRSTHDR`
        // returns a pointer within that buffer.
        let cmsg = unsafe { libc::CMSG_FIRSTHDR(&msg) };
        debug_assert!(!cmsg.is_null());
        // SAFETY: `cmsg` is non-null and points into `cmsg_buffer`, which
        // has room for exactly one `SCM_RIGHTS` header plus one `RawFd`.
        unsafe {
            (*cmsg).cmsg_level = libc::SOL_SOCKET;
            (*cmsg).cmsg_type = libc::SCM_RIGHTS;
            (*cmsg).cmsg_len = libc::CMSG_LEN(std::mem::size_of::<RawFd>() as u32) as _;
            std::ptr::write_unaligned(libc::CMSG_DATA(cmsg).cast::<RawFd>(), fd);
        }
    }

    // SAFETY: `msg` is fully initialized above (iovec, and control data only
    // when `fd` is `Some`); `socket` is a live, caller-owned descriptor.
    let result = unsafe { libc::sendmsg(socket, &msg, 0) };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(result as usize)
}

/// Receives into `buffer`, returning the byte count and at most one
/// transferred descriptor. Rejects (by returning [`RecvFd::Malformed`],
/// having already closed any received descriptors) anything other than
/// exactly zero or one `SCM_RIGHTS` descriptor of the expected size, per
/// SPEC-004 section 6.5 ("missing, extra or wrong-context descriptors are
/// protocol-fatal").
pub fn recv_with_fd(socket: RawFd, buffer: &mut [u8]) -> io::Result<(usize, RecvFd)> {
    let mut iov = libc::iovec {
        iov_base: buffer.as_mut_ptr().cast::<libc::c_void>(),
        iov_len: buffer.len(),
    };
    let mut cmsg_buffer = vec![0u8; cmsg_space_for_one_fd()];
    // SAFETY: see `send_with_fd`'s identical zeroing rationale.
    let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;
    msg.msg_control = cmsg_buffer.as_mut_ptr().cast::<libc::c_void>();
    msg.msg_controllen = cmsg_buffer.len() as libc::socklen_t;

    // SAFETY: `msg` is fully initialized (iovec + control buffer sized for
    // exactly one `RawFd`); `socket` is a live, caller-owned descriptor.
    let result = unsafe { libc::recvmsg(socket, &mut msg, 0) };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }

    if msg.msg_flags & libc::MSG_CTRUNC != 0 {
        // The kernel had to truncate ancillary data: never trust a
        // partially delivered descriptor set.
        return Ok((result as usize, RecvFd::Malformed));
    }

    let mut received: Vec<RawFd> = Vec::new();
    // SAFETY: `msg` was just populated by a successful `recvmsg` above and
    // `msg.msg_control`/`msg_controllen` describe `cmsg_buffer`.
    let mut cmsg = unsafe { libc::CMSG_FIRSTHDR(&msg) };
    while !cmsg.is_null() {
        // SAFETY: `cmsg` is non-null and was produced by
        // `CMSG_FIRSTHDR`/`CMSG_NXTHDR` walking `msg`'s control buffer.
        let (level, kind, len) =
            unsafe { ((*cmsg).cmsg_level, (*cmsg).cmsg_type, (*cmsg).cmsg_len) };
        if level == libc::SOL_SOCKET && kind == libc::SCM_RIGHTS {
            let expected_len =
                unsafe { libc::CMSG_LEN(std::mem::size_of::<RawFd>() as u32) } as usize;
            if len as usize == expected_len {
                // SAFETY: `len` matches exactly one `RawFd`'s worth of
                // ancillary payload at `CMSG_DATA(cmsg)`.
                let fd = unsafe { std::ptr::read_unaligned(libc::CMSG_DATA(cmsg).cast::<RawFd>()) };
                received.push(fd);
            } else {
                // An unexpected size (e.g. multiple descriptors packed into
                // one header) is never trusted; close anything received
                // and report malformed.
                return Ok((result as usize, close_and_report_malformed(received)));
            }
        }
        // SAFETY: `cmsg` is a valid pointer into `msg`'s control buffer.
        cmsg = unsafe { libc::CMSG_NXTHDR(&msg, cmsg) };
    }

    match received.len() {
        0 => Ok((result as usize, RecvFd::None)),
        1 => {
            // SAFETY: `received[0]` was just returned by the kernel as a
            // freshly transferred, exclusively owned descriptor.
            let owned = unsafe { OwnedFd::from_raw_fd(received[0]) };
            Ok((result as usize, RecvFd::One(owned)))
        }
        _ => Ok((result as usize, close_and_report_malformed(received))),
    }
}

fn close_and_report_malformed(received: Vec<RawFd>) -> RecvFd {
    for fd in received {
        // SAFETY: each `fd` was freshly transferred to this process and not
        // yet owned/tracked anywhere else; closing it here reclaims it
        // rather than leaking on a rejected/malformed transfer.
        unsafe {
            libc::close(fd);
        }
    }
    RecvFd::Malformed
}

pub enum RecvFd {
    None,
    One(OwnedFd),
    /// More than one descriptor, or a truncated/wrong-sized control
    /// message, was received. All received descriptors are already closed.
    Malformed,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::fd::AsRawFd;
    use std::os::unix::net::UnixStream;

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
}
