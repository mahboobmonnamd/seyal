use std::time::Duration;

use seyal_terminal::TerminalState;

use crate::{
    ChildExit, CommandSpec, ExecError, ReadOutcome, Readiness, TerminationPolicy, WindowSize,
    WriteOutcome, endpoint::TerminalEndpoint,
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

    pub fn read_output(&mut self, buffer: &mut [u8]) -> Result<ReadOutcome, ExecError> {
        let outcome = self.endpoint.read(buffer)?;
        if let ReadOutcome::Bytes(count) = outcome
            && count > 0
        {
            self.terminal.feed(&buffer[..count]);
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

    pub fn terminate(&mut self, policy: TerminationPolicy) -> Result<ChildExit, ExecError> {
        self.endpoint.terminate(policy)
    }
}
