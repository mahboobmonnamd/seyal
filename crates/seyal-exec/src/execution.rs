use std::time::Duration;

use seyal_terminal::{LineId, TerminalState};

use crate::{
    ChildExit, CommandSpec, ExecError, ProjectionDamage, ReadOutcome, Readiness, SignalDisposition,
    TerminalProjectionSnapshot, TerminalProjectionUpdate, TerminationPolicy, WindowSize,
    WriteOutcome, endpoint::TerminalEndpoint, projection,
};

pub struct TerminalExecution {
    endpoint: TerminalEndpoint,
    terminal: TerminalState,
    initial_primary_line_id: Option<LineId>,
}

impl TerminalExecution {
    pub fn spawn(command: &CommandSpec, size: WindowSize) -> Result<Self, ExecError> {
        let terminal = TerminalState::new(size.columns(), size.rows())?;
        // Capture the canonical primary-screen identity before any child bytes
        // can be admitted. This is a narrow read-only history-identity seam for
        // Workspace metadata; it is not a copied grid/transcript authority.
        let initial_primary_line_id = terminal.line_id(0);
        let endpoint = TerminalEndpoint::spawn(command, size)?;
        Ok(Self {
            endpoint,
            terminal,
            initial_primary_line_id,
        })
    }

    pub fn child_id(&self) -> u32 {
        self.endpoint.child_id()
    }

    pub fn terminal(&self) -> &TerminalState {
        &self.terminal
    }

    /// Canonical primary-screen logical line that existed when this execution
    /// was created. It is immutable across scroll, resize, alternate-screen
    /// entry/exit, projection resync, and GUI detach/reattach.
    pub fn initial_primary_line_id(&self) -> Option<LineId> {
        self.initial_primary_line_id
    }

    /// Copies the complete current canonical visible terminal state into an
    /// owned, projection-neutral snapshot without consuming canonical damage.
    /// Attach/reconnect/resync intentionally use this expensive recovery seam.
    pub fn projection_snapshot(&self) -> TerminalProjectionSnapshot {
        projection::snapshot(&self.terminal, self.terminal.damage_generation())
    }

    /// Consumes canonical damage exactly once and copies only the affected row
    /// range for steady-state display fanout. A full canonical damage record
    /// still produces the complete visible state, as required after resize or
    /// other full invalidation.
    pub fn take_projection_update(&mut self) -> Option<TerminalProjectionUpdate> {
        let damage = self.terminal.take_damage()?;
        Some(projection::update(
            &self.terminal,
            damage.generation,
            ProjectionDamage {
                full: damage.full,
                first_row: damage.first_row,
                last_row: damage.last_row,
            },
        ))
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

    pub fn signal_terminate(&mut self) -> Result<SignalDisposition, ExecError> {
        self.endpoint.signal_terminate()
    }

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