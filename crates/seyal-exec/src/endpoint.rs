use std::{
    io::{Read, Write},
    process::Stdio,
    time::{Duration, Instant},
};

use crate::{
    ChildExit, CommandSpec, ExecError, Readiness, TerminationPolicy, WindowSize,
    child::ChildLifecycle,
    platform,
    readiness::{Interest, wait},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadOutcome {
    Bytes(usize),
    WouldBlock,
    Eof,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteOutcome {
    Bytes(usize),
    WouldBlock,
}

pub struct TerminalEndpoint {
    master: platform::MasterHandle,
    child: ChildLifecycle,
}

impl TerminalEndpoint {
    pub fn spawn(command_spec: &CommandSpec, size: WindowSize) -> Result<Self, ExecError> {
        let pair = platform::open_pty(size)?;
        let stdin = pair.slave.try_clone()?;
        let stdout = pair.slave.try_clone()?;
        let stderr = pair.slave.try_clone()?;

        let mut command = command_spec.command();
        command
            .stdin(Stdio::from(stdin))
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr));
        platform::configure_child(&mut command)?;

        let child = command.spawn()?;
        drop(pair.slave);

        Ok(Self {
            master: pair.master,
            child: ChildLifecycle::new(child),
        })
    }

    pub fn child_id(&self) -> u32 {
        self.child.id()
    }

    pub fn read(&mut self, buffer: &mut [u8]) -> Result<ReadOutcome, ExecError> {
        if buffer.is_empty() {
            return Ok(ReadOutcome::Bytes(0));
        }
        match self.master.read(buffer) {
            Ok(0) => Ok(ReadOutcome::Eof),
            Ok(read) => Ok(ReadOutcome::Bytes(read)),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                Ok(ReadOutcome::WouldBlock)
            }
            Err(error) if error.raw_os_error() == Some(libc_eio()) => Ok(ReadOutcome::Eof),
            Err(error) => Err(error.into()),
        }
    }

    pub fn write(&mut self, bytes: &[u8]) -> Result<WriteOutcome, ExecError> {
        if bytes.is_empty() {
            return Ok(WriteOutcome::Bytes(0));
        }
        match self.master.write(bytes) {
            Ok(written) => Ok(WriteOutcome::Bytes(written)),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                Ok(WriteOutcome::WouldBlock)
            }
            Err(error) => Err(error.into()),
        }
    }

    pub fn write_all_bounded(&mut self, bytes: &[u8], timeout: Duration) -> Result<(), ExecError> {
        let deadline = Instant::now() + timeout;
        let mut written = 0;

        while written < bytes.len() {
            match self.write(&bytes[written..])? {
                WriteOutcome::Bytes(0) | WriteOutcome::WouldBlock => {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    if remaining.is_zero() {
                        return Err(ExecError::IoTimedOut("PTY write"));
                    }
                    let readiness = self.wait_writable(remaining)?;
                    if !readiness.ready && !readiness.hangup {
                        return Err(ExecError::IoTimedOut("PTY write"));
                    }
                }
                WriteOutcome::Bytes(count) => written += count,
            }
        }
        Ok(())
    }

    pub fn wait_readable(&self, timeout: Duration) -> Result<Readiness, ExecError> {
        wait(&self.master, Interest::Read, timeout)
    }

    pub fn wait_writable(&self, timeout: Duration) -> Result<Readiness, ExecError> {
        wait(&self.master, Interest::Write, timeout)
    }

    pub fn set_window_size(&self, size: WindowSize) -> Result<(), ExecError> {
        platform::set_winsize(&self.master, size)
    }

    pub fn window_size(&self) -> Result<WindowSize, ExecError> {
        platform::get_winsize(&self.master)
    }

    pub fn try_wait(&mut self) -> Result<Option<ChildExit>, ExecError> {
        self.child.try_wait()
    }

    pub fn terminate(&mut self, policy: TerminationPolicy) -> Result<ChildExit, ExecError> {
        self.child.terminate(policy)
    }
}

#[cfg(target_os = "macos")]
const fn libc_eio() -> i32 {
    libc::EIO
}

#[cfg(not(target_os = "macos"))]
const fn libc_eio() -> i32 {
    i32::MIN
}
