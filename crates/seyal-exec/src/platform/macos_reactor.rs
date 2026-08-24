use std::{
    io,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    ptr,
    sync::Arc,
    time::Duration,
};

use crate::ExecError;

pub(crate) const MAX_NATIVE_EVENTS: usize = 128;
const CONTROL_IDENT: usize = 1;

#[derive(Clone)]
pub(crate) struct KqueueHandle(Arc<OwnedFd>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NativeFilter {
    Read,
    Write,
    ProcessExit,
    Control,
    Other,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct NativeEvent {
    pub(crate) token: u64,
    pub(crate) filter: NativeFilter,
    pub(crate) hangup: bool,
}

pub(crate) struct NativeEventBuffer {
    events: [libc::kevent; MAX_NATIVE_EVENTS],
}

impl NativeEventBuffer {
    pub(crate) fn new() -> Self {
        Self {
            events: [empty_kevent(); MAX_NATIVE_EVENTS],
        }
    }
}

pub(crate) fn create_kqueue() -> Result<KqueueHandle, ExecError> {
    // SAFETY: kqueue has no pointer arguments and returns a newly owned fd.
    let fd = unsafe { libc::kqueue() };
    if fd < 0 {
        return Err(io::Error::last_os_error().into());
    }
    // SAFETY: successful kqueue transfers ownership of the new fd.
    let fd = unsafe { OwnedFd::from_raw_fd(fd) };
    set_close_on_exec(fd.as_raw_fd())?;
    let handle = KqueueHandle(Arc::new(fd));
    change(
        &handle,
        CONTROL_IDENT,
        libc::EVFILT_USER,
        libc::EV_ADD | libc::EV_CLEAR,
        0,
        0,
        false,
    )?;
    Ok(handle)
}

pub(crate) fn register_read(kqueue: &KqueueHandle, fd: i32, token: u64) -> Result<(), ExecError> {
    change(
        kqueue,
        fd as usize,
        libc::EVFILT_READ,
        libc::EV_ADD | libc::EV_ENABLE,
        0,
        token,
        false,
    )
}

pub(crate) fn register_write(kqueue: &KqueueHandle, fd: i32, token: u64) -> Result<(), ExecError> {
    change(
        kqueue,
        fd as usize,
        libc::EVFILT_WRITE,
        libc::EV_ADD | libc::EV_ENABLE,
        0,
        token,
        false,
    )
}

pub(crate) fn register_process_exit(
    kqueue: &KqueueHandle,
    pid: i32,
    token: u64,
) -> Result<(), ExecError> {
    change(
        kqueue,
        pid as usize,
        libc::EVFILT_PROC,
        libc::EV_ADD | libc::EV_ENABLE,
        libc::NOTE_EXIT,
        token,
        false,
    )
}

pub(crate) fn deregister_read(kqueue: &KqueueHandle, fd: i32) -> Result<(), ExecError> {
    change(
        kqueue,
        fd as usize,
        libc::EVFILT_READ,
        libc::EV_DELETE,
        0,
        0,
        true,
    )
}

pub(crate) fn deregister_write(kqueue: &KqueueHandle, fd: i32) -> Result<(), ExecError> {
    change(
        kqueue,
        fd as usize,
        libc::EVFILT_WRITE,
        libc::EV_DELETE,
        0,
        0,
        true,
    )
}

pub(crate) fn deregister_process_exit(kqueue: &KqueueHandle, pid: i32) -> Result<(), ExecError> {
    change(
        kqueue,
        pid as usize,
        libc::EVFILT_PROC,
        libc::EV_DELETE,
        0,
        0,
        true,
    )
}

pub(crate) fn trigger_control(kqueue: &KqueueHandle) -> Result<(), ExecError> {
    change(
        kqueue,
        CONTROL_IDENT,
        libc::EVFILT_USER,
        0,
        libc::NOTE_TRIGGER,
        0,
        false,
    )
}

pub(crate) fn wait_events(
    kqueue: &KqueueHandle,
    buffer: &mut NativeEventBuffer,
    timeout: Option<Duration>,
    out: &mut [NativeEvent; MAX_NATIVE_EVENTS],
) -> Result<usize, ExecError> {
    let timespec = timeout.map(duration_to_timespec);
    let timeout_ptr = timespec
        .as_ref()
        .map_or(ptr::null(), |value| value as *const libc::timespec);

    // SAFETY: the output array contains MAX_NATIVE_EVENTS fully initialized
    // kevent values and remains exclusively borrowed for the syscall. kqueue
    // is an owned live descriptor. timeout_ptr is either null or points to the
    // local initialized timespec for the duration of the call.
    let count = unsafe {
        libc::kevent(
            kqueue.0.as_raw_fd(),
            ptr::null(),
            0,
            buffer.events.as_mut_ptr(),
            buffer.events.len() as i32,
            timeout_ptr,
        )
    };
    if count < 0 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::EINTR) {
            return Ok(0);
        }
        return Err(error.into());
    }

    for (destination, source) in out.iter_mut().zip(buffer.events[..count as usize].iter()) {
        let filter = if source.filter == libc::EVFILT_READ {
            NativeFilter::Read
        } else if source.filter == libc::EVFILT_WRITE {
            NativeFilter::Write
        } else if source.filter == libc::EVFILT_PROC {
            NativeFilter::ProcessExit
        } else if source.filter == libc::EVFILT_USER && source.ident == CONTROL_IDENT {
            NativeFilter::Control
        } else {
            NativeFilter::Other
        };
        *destination = NativeEvent {
            token: source.udata as usize as u64,
            filter,
            hangup: source.flags & libc::EV_EOF != 0,
        };
    }
    Ok(count as usize)
}

fn change(
    kqueue: &KqueueHandle,
    ident: usize,
    filter: i16,
    flags: u16,
    fflags: u32,
    token: u64,
    allow_gone: bool,
) -> Result<(), ExecError> {
    let event = libc::kevent {
        ident,
        filter,
        flags,
        fflags,
        data: 0,
        // This is an opaque integer round-trip carrier. It is never
        // dereferenced and never points at Rust-owned storage.
        udata: token as usize as *mut libc::c_void,
    };
    // SAFETY: event points to one initialized change record; there is no event
    // output buffer for this registration operation.
    let rc = unsafe {
        libc::kevent(
            kqueue.0.as_raw_fd(),
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
    let error = io::Error::last_os_error();
    if allow_gone
        && error
            .raw_os_error()
            .is_some_and(|code| matches!(code, libc::ENOENT | libc::EBADF | libc::ESRCH))
    {
        return Ok(());
    }
    Err(error.into())
}

fn set_close_on_exec(fd: i32) -> Result<(), ExecError> {
    // SAFETY: fd is an owned live kqueue descriptor.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    if flags < 0 {
        return Err(io::Error::last_os_error().into());
    }
    // SAFETY: fd remains owned/live and flags came from F_GETFD.
    if unsafe { libc::fcntl(fd, libc::F_SETFD, flags | libc::FD_CLOEXEC) } < 0 {
        return Err(io::Error::last_os_error().into());
    }
    Ok(())
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

fn duration_to_timespec(duration: Duration) -> libc::timespec {
    libc::timespec {
        tv_sec: duration.as_secs().min(i64::MAX as u64) as libc::time_t,
        tv_nsec: duration.subsec_nanos() as libc::c_long,
    }
}
