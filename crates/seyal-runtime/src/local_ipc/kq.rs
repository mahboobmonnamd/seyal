//! Minimal macOS `kqueue` wrapper for the local attachment control-socket
//! event loop. This is a small, independent copy of the same primitive
//! `seyal-exec`'s PTY reactor uses internally (that crate does not expose
//! its kqueue for foreign descriptors); it exists only so `local_ipc` never
//! needs a thread/process per connection and never busy-polls.

use std::{
    io,
    os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd},
    ptr,
    time::Duration,
};

pub const MAX_EVENTS: usize = 256;

pub struct Kqueue {
    fd: OwnedFd,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Readiness {
    Readable,
    Writable,
}

#[derive(Clone, Copy, Debug)]
pub struct Event {
    pub token: u64,
    pub readiness: Readiness,
    pub hangup: bool,
}

impl Kqueue {
    pub fn create() -> io::Result<Self> {
        // SAFETY: `kqueue()` takes no arguments and returns a newly owned
        // descriptor (or -1 on error).
        let raw = unsafe { libc::kqueue() };
        if raw < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: `raw` was just returned by a successful `kqueue()` call
        // and is not owned anywhere else yet.
        let fd = unsafe { OwnedFd::from_raw_fd(raw) };
        set_close_on_exec(fd.as_raw_fd())?;
        Ok(Self { fd })
    }

    pub fn register_read(&self, fd: RawFd, token: u64) -> io::Result<()> {
        self.change(fd, libc::EVFILT_READ, libc::EV_ADD | libc::EV_ENABLE, token)
    }

    pub fn register_write(&self, fd: RawFd, token: u64) -> io::Result<()> {
        self.change(
            fd,
            libc::EVFILT_WRITE,
            libc::EV_ADD | libc::EV_ENABLE,
            token,
        )
    }

    pub fn deregister_write(&self, fd: RawFd) -> io::Result<()> {
        match self.change(fd, libc::EVFILT_WRITE, libc::EV_DELETE, 0) {
            Ok(()) => Ok(()),
            Err(error)
                if matches!(error.raw_os_error(), Some(libc::ENOENT) | Some(libc::EBADF)) =>
            {
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    pub fn deregister_all(&self, fd: RawFd) -> io::Result<()> {
        let _ = self.change(fd, libc::EVFILT_READ, libc::EV_DELETE, 0);
        let _ = self.change(fd, libc::EVFILT_WRITE, libc::EV_DELETE, 0);
        Ok(())
    }

    pub fn wait(&self, timeout: Option<Duration>, out: &mut [Event]) -> io::Result<usize> {
        let mut raw_events = [empty_kevent(); MAX_EVENTS];
        let limit = out.len().min(MAX_EVENTS);
        let timespec = timeout.map(|duration| libc::timespec {
            tv_sec: duration.as_secs().min(i64::MAX as u64) as libc::time_t,
            tv_nsec: duration.subsec_nanos() as libc::c_long,
        });
        let timeout_ptr = timespec
            .as_ref()
            .map_or(ptr::null(), |value| value as *const libc::timespec);

        // SAFETY: `raw_events` has room for `limit <= MAX_EVENTS` fully
        // initialized `kevent` entries; `self.fd` is a live owned kqueue
        // descriptor; `timeout_ptr` is either null or points at a local,
        // still-live `timespec` for the duration of the call.
        let count = unsafe {
            libc::kevent(
                self.fd.as_raw_fd(),
                ptr::null(),
                0,
                raw_events.as_mut_ptr(),
                limit as i32,
                timeout_ptr,
            )
        };
        if count < 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::EINTR) {
                return Ok(0);
            }
            return Err(error);
        }
        let count = count as usize;
        for (destination, source) in out.iter_mut().zip(raw_events[..count].iter()) {
            let readiness = if source.filter == libc::EVFILT_WRITE {
                Readiness::Writable
            } else {
                Readiness::Readable
            };
            *destination = Event {
                token: source.udata as usize as u64,
                readiness,
                hangup: source.flags & libc::EV_EOF != 0,
            };
        }
        Ok(count)
    }

    fn change(&self, fd: RawFd, filter: i16, flags: u16, token: u64) -> io::Result<()> {
        let event = libc::kevent {
            ident: fd as usize,
            filter,
            flags,
            fflags: 0,
            data: 0,
            // Opaque integer round-trip carrier; never dereferenced.
            udata: token as usize as *mut libc::c_void,
        };
        // SAFETY: `event` is one fully initialized change record and there
        // is no output-event buffer for a pure registration call.
        let rc = unsafe {
            libc::kevent(
                self.fd.as_raw_fd(),
                &event,
                1,
                ptr::null_mut(),
                0,
                ptr::null(),
            )
        };
        if rc == 0 {
            return Ok(());
        }
        Err(io::Error::last_os_error())
    }
}

const fn empty_kevent() -> libc::kevent {
    libc::kevent {
        ident: 0,
        filter: 0,
        flags: 0,
        fflags: 0,
        data: 0,
        udata: ptr::null_mut(),
    }
}

fn set_close_on_exec(fd: RawFd) -> io::Result<()> {
    // SAFETY: `fd` is an owned, live descriptor for the duration of this
    // call.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: same live fd; `flags` came from the successful `F_GETFD`.
    if unsafe { libc::fcntl(fd, libc::F_SETFD, flags | libc::FD_CLOEXEC) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixStream;

    #[test]
    fn wait_reports_a_readable_socket_after_data_is_written() {
        let kq = Kqueue::create().unwrap();
        let (a, b) = UnixStream::pair().unwrap();
        kq.register_read(b.as_raw_fd(), 42).unwrap();

        use std::io::Write;
        let mut a = a;
        a.write_all(b"x").unwrap();

        let mut events = [Event {
            token: 0,
            readiness: Readiness::Readable,
            hangup: false,
        }; 8];
        let count = kq.wait(Some(Duration::from_secs(1)), &mut events).unwrap();
        assert!(count >= 1);
        assert!(
            events[..count]
                .iter()
                .any(|event| event.token == 42 && event.readiness == Readiness::Readable)
        );
    }

    #[test]
    fn wait_times_out_with_no_ready_descriptors() {
        let kq = Kqueue::create().unwrap();
        let (_a, b) = UnixStream::pair().unwrap();
        kq.register_read(b.as_raw_fd(), 7).unwrap();
        let mut events = [Event {
            token: 0,
            readiness: Readiness::Readable,
            hangup: false,
        }; 8];
        let count = kq
            .wait(Some(Duration::from_millis(20)), &mut events)
            .unwrap();
        assert_eq!(count, 0);
    }
}
