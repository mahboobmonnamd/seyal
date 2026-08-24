use std::{fs::File, process::Command, time::Duration};

use crate::{
    ExecError, WindowSize,
    readiness::{Interest, Readiness},
};

pub(crate) type MasterHandle = File;

pub(crate) struct PtyPair {
    pub(crate) master: File,
    pub(crate) slave: File,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Signal {
    Terminate,
    Kill,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SignalOutcome {
    Delivered,
    Gone,
}

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod macos;

#[cfg(target_os = "macos")]
pub(crate) use macos::{
    configure_child, get_winsize, open_pty, set_winsize, signal_owned_process_group, wait,
};

#[cfg(not(target_os = "macos"))]
pub(crate) fn open_pty(_size: WindowSize) -> Result<PtyPair, ExecError> {
    Err(ExecError::UnsupportedPlatform(
        "local PTY execution is implemented for macOS only in M001",
    ))
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn configure_child(_command: &mut Command) -> Result<(), ExecError> {
    Err(ExecError::UnsupportedPlatform(
        "local PTY execution is implemented for macOS only in M001",
    ))
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn wait(
    _master: &MasterHandle,
    _interest: Interest,
    _timeout: Duration,
) -> Result<Readiness, ExecError> {
    Err(ExecError::UnsupportedPlatform(
        "local PTY execution is implemented for macOS only in M001",
    ))
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn set_winsize(
    _master: &MasterHandle,
    _size: WindowSize,
) -> Result<(), ExecError> {
    Err(ExecError::UnsupportedPlatform(
        "local PTY execution is implemented for macOS only in M001",
    ))
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn get_winsize(_master: &MasterHandle) -> Result<WindowSize, ExecError> {
    Err(ExecError::UnsupportedPlatform(
        "local PTY execution is implemented for macOS only in M001",
    ))
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn signal_owned_process_group(
    _pid: i32,
    _expected_process_group: i32,
    _signal: Signal,
) -> Result<SignalOutcome, ExecError> {
    Err(ExecError::UnsupportedPlatform(
        "local PTY execution is implemented for macOS only in M001",
    ))
}
