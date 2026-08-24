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

    pub(crate) fn terminate(&mut self, policy: TerminationPolicy) -> Result<ChildExit, ExecError> {
        if let Some(exit) = self.try_wait()? {
            return Ok(exit);
        }

        match platform::signal_owned_process_group(
            self.child.id() as i32,
            self.process_group,
            platform::Signal::Terminate,
        )? {
            platform::SignalOutcome::Delivered => {
                if let Some(exit) = self.wait_for_exit(policy.graceful_wait)? {
                    return Ok(exit);
                }
            }
            platform::SignalOutcome::Gone => {
                if let Some(exit) = self.try_wait()? {
                    return Ok(exit);
                }
            }
        }

        if let Some(exit) = self.try_wait()? {
            return Ok(exit);
        }

        match platform::signal_owned_process_group(
            self.child.id() as i32,
            self.process_group,
            platform::Signal::Kill,
        )? {
            platform::SignalOutcome::Delivered | platform::SignalOutcome::Gone => {}
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
