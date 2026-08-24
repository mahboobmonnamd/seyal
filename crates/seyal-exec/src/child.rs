use std::{
    process::{Child, ExitStatus},
    thread,
    time::{Duration, Instant},
};

#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;

use crate::{ExecError, platform};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChildExit {
    Exited(i32),
    Signaled(i32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminationPolicy {
    graceful_wait: Duration,
    kill_wait: Duration,
}

impl TerminationPolicy {
    pub fn new(graceful_wait: Duration, kill_wait: Duration) -> Self {
        Self {
            graceful_wait,
            kill_wait,
        }
    }

    pub fn graceful_wait(self) -> Duration {
        self.graceful_wait
    }

    pub fn kill_wait(self) -> Duration {
        self.kill_wait
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignalDisposition {
    Delivered,
    ProcessGone,
    AlreadyReaped(ChildExit),
}

pub(crate) struct ChildLifecycle {
    child: Child,
    process_group: i32,
    reaped: Option<ChildExit>,
}

impl ChildLifecycle {
    pub(crate) fn new(child: Child) -> Self {
        let process_group = child.id() as i32;
        Self {
            child,
            process_group,
            reaped: None,
        }
    }

    pub(crate) fn id(&self) -> u32 {
        self.child.id()
    }

    pub(crate) fn try_wait(&mut self) -> Result<Option<ChildExit>, ExecError> {
        if let Some(exit) = self.reaped {
            return Ok(Some(exit));
        }
        match self.child.try_wait()? {
            Some(status) => {
                let exit = classify(status);
                self.reaped = Some(exit);
                Ok(Some(exit))
            }
            None => Ok(None),
        }
    }

    pub(crate) fn signal_terminate(&mut self) -> Result<SignalDisposition, ExecError> {
        self.signal(platform::Signal::Terminate)
    }

    pub(crate) fn signal_kill(&mut self) -> Result<SignalDisposition, ExecError> {
        self.signal(platform::Signal::Kill)
    }

    fn signal(&mut self, signal: platform::Signal) -> Result<SignalDisposition, ExecError> {
        if let Some(exit) = self.try_wait()? {
            return Ok(SignalDisposition::AlreadyReaped(exit));
        }
        let outcome = platform::signal_owned_process_group(
            self.child.id() as i32,
            self.process_group,
            signal,
        )?;
        Ok(match outcome {
            platform::SignalOutcome::Delivered => SignalDisposition::Delivered,
            platform::SignalOutcome::Gone => SignalDisposition::ProcessGone,
        })
    }

    pub(crate) fn terminate(&mut self, policy: TerminationPolicy) -> Result<ChildExit, ExecError> {
        if let Some(exit) = self.try_wait()? {
            return Ok(exit);
        }

        match self.signal_terminate()? {
            SignalDisposition::AlreadyReaped(exit) => return Ok(exit),
            SignalDisposition::Delivered => {
                if let Some(exit) = self.wait_for_exit(policy.graceful_wait)? {
                    return Ok(exit);
                }
            }
            SignalDisposition::ProcessGone => {
                if let Some(exit) = self.try_wait()? {
                    return Ok(exit);
                }
            }
        }

        if let Some(exit) = self.try_wait()? {
            return Ok(exit);
        }

        match self.signal_kill()? {
            SignalDisposition::AlreadyReaped(exit) => return Ok(exit),
            SignalDisposition::Delivered | SignalDisposition::ProcessGone => {}
        }

        self.wait_for_exit(policy.kill_wait)?
            .ok_or(ExecError::TerminationTimedOut)
    }

    fn wait_for_exit(&mut self, timeout: Duration) -> Result<Option<ChildExit>, ExecError> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(exit) = self.try_wait()? {
                return Ok(Some(exit));
            }
            if Instant::now() >= deadline {
                return Ok(None);
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            thread::sleep(remaining.min(Duration::from_millis(5)));
        }
    }
}

fn classify(status: ExitStatus) -> ChildExit {
    if let Some(code) = status.code() {
        return ChildExit::Exited(code);
    }

    #[cfg(unix)]
    if let Some(signal) = status.signal() {
        return ChildExit::Signaled(signal);
    }

    ChildExit::Exited(1)
}
