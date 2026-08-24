use std::time::Duration;

use seyal_terminal::TerminalState;

use crate::{
    ChildExit, CommandSpec, ExecError, ReadOutcome, Readiness, SignalDisposition,
    TerminalProjectionSnapshot, TerminationPolicy, WindowSize, WriteOutcome,
    endpoint::TerminalEndpoint, projection,
};

pub struct TerminalExecution {
    endpoint: TerminalEndpoint,
    terminal: TerminalState,
}

impl TerminalExecution {
    pub fn spawn(command: &CommandSpec, size: WindowSize) -> Result<Self, ExecError> {
        let terminal = TerminalState::new(size.columns(), size.rows())?;
        let endpoint = TerminalEndpoint::spawn(command, size)?;
        Ok(Self { endpoint, terminal })
    }

    pub fn child_id(&self) -> u32 {
        self.endpoint.child_id()
    }

    pub fn terminal(&self) -> &TerminalState {
        &self.terminal
    }

    /// Copies the current canonical visible terminal state into an owned,
    /// projection-neutral snapshot without consuming canonical damage.
    ///
    /// Attach/resync use this seam so one client cannot steal the Runtime's
    /// single damage-consumer signal from other attached clients.
    pub fn projection_snapshot(&self) -> TerminalProjectionSnapshot {
        projection::snapshot(&self.terminal, self.terminal.damage_generation())
    }

    /// Single Runtime-side damage-consumption seam for display projection.
    ///
    /// If canonical state has changed since the previous call, consumes that
    /// coalesced damage generation once and returns a full current visible
    /// snapshot tagged with the consumed generation. Runtime then fans that
    /// one snapshot out to every attachment. Client-specific attach/resync
    /// must use [`Self::projection_snapshot`] instead.
    pub fn take_projection_update(&mut self) -> Option<TerminalProjectionSnapshot> {
        let damage = self.terminal.take_damage()?;
        Some(projection::snapshot(&self.terminal, damage.generation))
    }

    pub fn read_output(&mut self, buffer: &mut [u8]) -> Result<ReadOutcome, ExecError> {
        let outcome = self.endpoint.read(buffer)?;
        if let ReadOutcome::Bytes(count) = outcome
            && count > 0
        {
            self.terminal.feed(&buffer[..count])?;
        }
        Ok(outcome)
    }

    pub fn write_input(&mut self, bytes: &[u8]) -> Result<WriteOutcome, ExecError> {
        self.endpoint.write(bytes)
    }

    pub fn write_input_bounded(
        &mut self,
        bytes: &[u8],
        timeout: Duration,
    ) -> Result<(), ExecError> {
        self.endpoint.write_all_bounded(bytes, timeout)
    }

    pub fn wait_readable(&self, timeout: Duration) -> Result<Readiness, ExecError> {
        self.endpoint.wait_readable(timeout)
    }

    pub fn wait_writable(&self, timeout: Duration) -> Result<Readiness, ExecError> {
        self.endpoint.wait_writable(timeout)
    }

    pub fn resize(&mut self, size: WindowSize) -> Result<(), ExecError> {
        self.endpoint.set_window_size(size)?;
        self.terminal.resize(size.columns(), size.rows())?;
        Ok(())
    }

    pub fn window_size(&self) -> Result<WindowSize, ExecError> {
        self.endpoint.window_size()
    }

    pub fn try_wait(&mut self) -> Result<Option<ChildExit>, ExecError> {
        self.endpoint.try_wait()
    }

    /// Sends SIGTERM to the still-owned primary process group without waiting.
    /// Runtime uses this step from its deadline-driven lifecycle state machine.
    pub fn signal_terminate(&mut self) -> Result<SignalDisposition, ExecError> {
        self.endpoint.signal_terminate()
    }

    /// Sends SIGKILL to the still-owned primary process group without waiting.
    /// Runtime uses this only after the configured graceful deadline expires.
    pub fn signal_kill(&mut self) -> Result<SignalDisposition, ExecError> {
        self.endpoint.signal_kill()
    }

    pub fn terminate(&mut self, policy: TerminationPolicy) -> Result<ChildExit, ExecError> {
        self.endpoint.terminate(policy)
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn reactor_fd(&self) -> i32 {
        self.endpoint.master_fd()
    }
}
