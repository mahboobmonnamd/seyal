#[cfg(target_os = "macos")]
use std::{io, os::fd::RawFd};

#[cfg(target_os = "macos")]
pub(crate) fn try_lock_exclusive(fd: RawFd) -> io::Result<bool> {
    // SAFETY: flock observes/updates advisory locking state for the live owned
    // descriptor. It does not take ownership or dereference user pointers.
    let rc = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };
    if rc == 0 {
        return Ok(true);
    }
    let error = io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::EWOULDBLOCK) {
        return Ok(false);
    }
    Err(error)
}

#[cfg(target_os = "macos")]
pub(crate) fn unlock(fd: RawFd) -> io::Result<()> {
    // SAFETY: `fd` is still owned/live by the SingletonGuard when Drop calls
    // this. LOCK_UN changes only advisory lock state and does not take
    // ownership of the descriptor.
    if unsafe { libc::flock(fd, libc::LOCK_UN) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn set_close_on_exec(fd: RawFd) -> io::Result<()> {
    // SAFETY: fd is owned/live for both fcntl operations.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: same live fd and flags came from F_GETFD.
    if unsafe { libc::fcntl(fd, libc::F_SETFD, flags | libc::FD_CLOEXEC) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
