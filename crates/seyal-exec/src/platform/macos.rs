use std::{
    fs::File,
    io,
    mem::MaybeUninit,
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::process::CommandExt,
    },
    process::Command,
    ptr,
    time::{Duration, Instant},
};

use crate::{
    ExecError, WindowSize,
    platform::{PtyPair, Signal, SignalOutcome},
    readiness::{Interest, Readiness},
};

pub(crate) fn open_pty(size: WindowSize) -> Result<PtyPair, ExecError> {
    let mut master_fd = -1;
    let mut slave_fd = -1;
    let winsize = to_native_winsize(size);

    // SAFETY: openpty initializes the two descriptor outputs on success. The
    // termios pointer is null so the kernel's normal interactive discipline is
    // retained. Every successful descriptor is immediately wrapped by OwnedFd.
    let rc = unsafe {
        libc::openpty(
            &mut master_fd,
            &mut slave_fd,
            ptr::null_mut(),
            ptr::null(),
            &winsize,
        )
    };
    if rc != 0 {
        return Err(io::Error::last_os_error().into());
    }

    // SAFETY: openpty returned success and ownership of both descriptors.
    let master = unsafe { OwnedFd::from_raw_fd(master_fd) };
    // SAFETY: same ownership transfer as the master descriptor.
    let slave = unsafe { OwnedFd::from_raw_fd(slave_fd) };

    set_close_on_exec(master.as_raw_fd())?;
    set_close_on_exec(slave.as_raw_fd())?;
    set_nonblocking(master.as_raw_fd())?;

    Ok(PtyPair {
        master: File::from(master),
        slave: File::from(slave),
    })
}

pub(crate) fn configure_child(command: &mut Command) -> Result<(), ExecError> {
    // SAFETY: the closure executes after fork and before exec. It performs only
    // the async-signal-safe session/controlling-terminal syscalls required for
    // the PTY child. No allocation, locking, logging or Rust runtime service is
    // used inside the closure.
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() < 0 {
                return Err(io::Error::last_os_error());
            }
            if libc::ioctl(libc::STDIN_FILENO, libc::TIOCSCTTY as _, 0) < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    Ok(())
}

pub(crate) fn wait(
    master: &File,
    interest: Interest,
    timeout: Duration,
) -> Result<Readiness, ExecError> {
    let events = match interest {
        Interest::Read => libc::POLLIN,
        Interest::Write => libc::POLLOUT,
    };
    let deadline = Instant::now() + timeout;

    loop {
        let mut descriptor = libc::pollfd {
            fd: master.as_raw_fd(),
            events,
            revents: 0,
        };
        let timeout_ms = remaining_timeout_ms(deadline);

        // SAFETY: descriptor points to one valid pollfd for the duration of the
        // call and the owned master remains alive.
        let rc = unsafe { libc::poll(&mut descriptor, 1, timeout_ms) };
        if rc > 0 {
            return Ok(Readiness {
                ready: descriptor.revents & events != 0,
                hangup: descriptor.revents & (libc::POLLHUP | libc::POLLERR) != 0,
            });
        }
        if rc == 0 {
            return Ok(Readiness::timeout());
        }

        let error = io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::EINTR) {
            if Instant::now() >= deadline {
                return Ok(Readiness::timeout());
            }
            continue;
        }
        return Err(error.into());
    }
}

pub(crate) fn set_winsize(master: &File, size: WindowSize) -> Result<(), ExecError> {
    let winsize = to_native_winsize(size);
    // SAFETY: master is an owned live PTY descriptor and winsize points to a
    // fully initialized libc::winsize value.
    let rc = unsafe { libc::ioctl(master.as_raw_fd(), libc::TIOCSWINSZ as _, &winsize) };
    if rc < 0 {
        return Err(io::Error::last_os_error().into());
    }
    Ok(())
}

pub(crate) fn get_winsize(master: &File) -> Result<WindowSize, ExecError> {
    let mut winsize = MaybeUninit::<libc::winsize>::zeroed();
    // SAFETY: ioctl initializes the winsize structure on success.
    let rc = unsafe {
        libc::ioctl(
            master.as_raw_fd(),
            libc::TIOCGWINSZ as _,
            winsize.as_mut_ptr(),
        )
    };
    if rc < 0 {
        return Err(io::Error::last_os_error().into());
    }
    // SAFETY: the successful ioctl initialized the full winsize structure.
    let winsize = unsafe { winsize.assume_init() };
    WindowSize::new(
        winsize.ws_col,
        winsize.ws_row,
        winsize.ws_xpixel,
        winsize.ws_ypixel,
    )
}

pub(crate) fn signal_owned_process_group(
    pid: i32,
    expected_process_group: i32,
    signal: Signal,
) -> Result<SignalOutcome, ExecError> {
    // SAFETY: getpgid only observes kernel process metadata for the supplied pid.
    let actual = unsafe { libc::getpgid(pid) };
    if actual < 0 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ESRCH) {
            return Ok(SignalOutcome::Gone);
        }
        return Err(error.into());
    }
    if actual != expected_process_group {
        return Err(ExecError::ProcessGroupMismatch {
            expected: expected_process_group,
            actual,
        });
    }

    let native_signal = match signal {
        Signal::Terminate => libc::SIGTERM,
        Signal::Kill => libc::SIGKILL,
    };

    // SAFETY: the negative id targets exactly the verified owned process group.
    let rc = unsafe { libc::kill(-expected_process_group, native_signal) };
    if rc == 0 {
        return Ok(SignalOutcome::Delivered);
    }
    let error = io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH) {
        return Ok(SignalOutcome::Gone);
    }
    Err(error.into())
}

fn set_nonblocking(fd: i32) -> Result<(), ExecError> {
    // SAFETY: fd is owned and live for both fcntl calls.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 {
        return Err(io::Error::last_os_error().into());
    }
    // SAFETY: same live owned descriptor; flags came from F_GETFL.
    if unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(io::Error::last_os_error().into());
    }
    Ok(())
}

fn set_close_on_exec(fd: i32) -> Result<(), ExecError> {
    // SAFETY: fd is owned and live for both fcntl calls.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    if flags < 0 {
        return Err(io::Error::last_os_error().into());
    }
    // SAFETY: same live owned descriptor; flags came from F_GETFD.
    if unsafe { libc::fcntl(fd, libc::F_SETFD, flags | libc::FD_CLOEXEC) } < 0 {
        return Err(io::Error::last_os_error().into());
    }
    Ok(())
}

fn to_native_winsize(size: WindowSize) -> libc::winsize {
    libc::winsize {
        ws_row: size.rows(),
        ws_col: size.columns(),
        ws_xpixel: size.pixel_width(),
        ws_ypixel: size.pixel_height(),
    }
}

fn remaining_timeout_ms(deadline: Instant) -> i32 {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return 0;
    }
    remaining
        .as_millis()
        .max(1)
        .min(i32::MAX as u128) as i32
}
