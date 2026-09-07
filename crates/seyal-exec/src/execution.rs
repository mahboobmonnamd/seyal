use std::{collections::VecDeque, time::Duration};

use seyal_terminal::{
    HostPresentationEvent, LineId, PreparedResize, ProtocolReply, ShellIntegrationEvent,
    TerminalState, MAX_PROTOCOL_REPLIES,
};

use crate::{
    endpoint::TerminalEndpoint, projection, ChildExit, CommandSpec, ExecError, ProjectionDamage,
    ReadOutcome, Readiness, SignalDisposition, TerminalProjectionSnapshot,
    TerminalProjectionUpdate, TerminationPolicy, WindowSize, WriteOutcome,
};

struct PendingProtocolReply {
    reply: ProtocolReply,
    offset: u8,
}

pub struct TerminalExecution {
    endpoint: TerminalEndpoint,
    terminal: TerminalState,
    initial_primary_line_id: Option<LineId>,
    pending_protocol_replies: VecDeque<PendingProtocolReply>,
}

impl TerminalExecution {
    pub fn spawn(command: &CommandSpec, size: WindowSize) -> Result<Self, ExecError> {
        let terminal = TerminalState::new(size.columns(), size.rows())?;
        // Capture the canonical primary-screen logical anchor before the child
        // can emit bytes, scroll, enter alternate screen, or otherwise mutate
        // the projection. Pass 8 stores this immutable identity only; it does
        // not expose mutable terminal/grid state or copy transcript content.
        let initial_primary_line_id = terminal.line_id(0);
        let endpoint = TerminalEndpoint::spawn(command, size)?;
        Ok(Self {
            endpoint,
            terminal,
            initial_primary_line_id,
            pending_protocol_replies: VecDeque::with_capacity(MAX_PROTOCOL_REPLIES),
        })
    }

    pub fn child_id(&self) -> u32 {
        self.endpoint.child_id()
    }

    pub fn terminal(&self) -> &TerminalState {
        &self.terminal
    }

    /// Stable canonical primary-screen logical line present when this
    /// execution was created. It is immutable across scroll, resize,
    /// alternate-screen transitions, projection resync, detach and reattach.
    pub fn initial_primary_line_id(&self) -> Option<LineId> {
        self.initial_primary_line_id
    }

    /// Transfers one bounded trusted shell-integration event observed by the
    /// canonical VT parser. No terminal cells or parser state leave here.
    pub fn take_shell_integration_event(&mut self) -> Option<ShellIntegrationEvent> {
        self.terminal.take_shell_integration_event()
    }

    /// Transfers one bounded, untrusted host-presentation event (OSC title/CWD/hyperlink).
    pub fn take_host_presentation_event(&mut self) -> Option<HostPresentationEvent> {
        self.terminal.take_host_presentation_event()
    }

    pub fn retained_history_bytes(&self) -> usize {
        self.terminal.primary_history_resident_bytes()
    }

    pub fn oldest_history_segment_age(&self) -> Option<u64> {
        self.terminal.primary_history_oldest_segment_age()
    }

    pub fn evict_oldest_history_segment(&mut self) -> usize {
        self.terminal.evict_oldest_primary_history_segment()
    }

    /// Transfers one complete terminal-generated protocol reply for PTY write.
    /// Prefer [`write_protocol_replies`] from Runtime write service. Returns
    /// `None` while a partially written reply remains at the front of the queue.
    pub fn take_protocol_reply(&mut self) -> Option<ProtocolReply> {
        self.capture_protocol_replies();
        let pending = self.pending_protocol_replies.front()?;
        if pending.offset != 0 {
            return None;
        }
        self.pending_protocol_replies
            .pop_front()
            .map(|pending| pending.reply)
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
            // Capture replies even when feed returns a sticky fault: query
            // handlers may have enqueued protocol bytes earlier in the same
            // chunk, and those must still reach the child PTY.
            let feed_result = self.terminal.feed(&buffer[..count]);
            self.capture_protocol_replies();
            feed_result?;
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

    /// True when terminal-generated replies remain queued for the PTY.
    pub fn has_pending_protocol_replies(&self) -> bool {
        !self.pending_protocol_replies.is_empty()
    }

    /// Writes queued protocol replies toward the child PTY, preferring them
    /// ahead of host-originated input. Returns bytes written this call.
    pub fn write_protocol_replies(&mut self, max_bytes: usize) -> Result<usize, ExecError> {
        let mut written_total = 0usize;
        while written_total < max_bytes {
            let Some(front) = self.pending_protocol_replies.front_mut() else {
                break;
            };
            let remaining = &front.reply.as_bytes()[usize::from(front.offset)..];
            if remaining.is_empty() {
                self.pending_protocol_replies.pop_front();
                continue;
            }
            let quantum = max_bytes - written_total;
            let slice = &remaining[..remaining.len().min(quantum)];
            match self.endpoint.write(slice)? {
                WriteOutcome::Bytes(0) | WriteOutcome::WouldBlock => break,
                WriteOutcome::Bytes(count) => {
                    front.offset = front.offset.saturating_add(count as u8);
                    written_total += count;
                    if usize::from(front.offset) >= front.reply.as_bytes().len() {
                        self.pending_protocol_replies.pop_front();
                    }
                }
            }
        }
        Ok(written_total)
    }

    pub fn clear_pending_protocol_replies(&mut self) {
        self.pending_protocol_replies.clear();
    }

    pub fn wait_readable(&self, timeout: Duration) -> Result<Readiness, ExecError> {
        self.endpoint.wait_readable(timeout)
    }

    pub fn wait_writable(&self, timeout: Duration) -> Result<Readiness, ExecError> {
        self.endpoint.wait_writable(timeout)
    }

    /// Cross-layer resize transaction: prepare canonical state, commit PTY
    /// geometry, then infallibly commit TerminalState. Geometry cannot diverge
    /// on any `Result` return path.
    pub fn resize(&mut self, size: WindowSize) -> Result<(), ExecError> {
        let prepared: PreparedResize = self.terminal.prepare_resize(size.columns(), size.rows())?;
        self.endpoint.set_window_size(size)?;
        self.terminal.commit_resize(prepared);
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

    fn capture_protocol_replies(&mut self) {
        while let Some(reply) = self.terminal.take_protocol_reply() {
            if self.pending_protocol_replies.len() >= MAX_PROTOCOL_REPLIES {
                break;
            }
            self.pending_protocol_replies
                .push_back(PendingProtocolReply { reply, offset: 0 });
        }
        // Drop any surplus still sitting on TerminalState so a stalled write
        // path cannot grow unbounded parser-side effects.
        while self.terminal.take_protocol_reply().is_some() {}
    }
}
