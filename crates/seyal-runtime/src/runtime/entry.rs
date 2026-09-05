use std::{
    collections::{HashSet, VecDeque},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

use seyal_exec::{RegistrationToken, TerminalExecution};

#[cfg(target_os = "macos")]
use crate::command_block_timeline::{CommandBlockId, CommandBlockTimeline};
use crate::{AttachmentId, ExecutionId, WorkspaceId};
#[cfg(target_os = "macos")]
use seyal_exec::ShellIntegrationToken;

use super::config::PtyEofReapProbe;
use super::lifecycle::{ExecutionLifecycle, Lifecycle};
#[cfg(target_os = "macos")]
use super::shell_integration::ShellIntegrationMode;
use crate::input::AcceptedInput;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExecutionSummary {
    pub id: ExecutionId,
    pub workspace_id: WorkspaceId,
    pub attachment_count: usize,
    pub lifecycle: ExecutionLifecycle,
}

pub(in crate::runtime) struct Entry {
    pub(in crate::runtime) execution: TerminalExecution,
    pub(in crate::runtime) token: RegistrationToken,
    pub(in crate::runtime) workspace_id: WorkspaceId,
    pub(in crate::runtime) attachments: HashSet<AttachmentId>,
    pub(in crate::runtime) lifecycle: Lifecycle,
    pub(in crate::runtime) pty_eof_reap_probe: Option<PtyEofReapProbe>,
    pub(in crate::runtime) pending_input: VecDeque<AcceptedInput>,
    pub(in crate::runtime) reserved_input: Arc<AtomicUsize>,
    pub(in crate::runtime) ingress_active: Arc<AtomicBool>,
    /// Accepted composer commands awaiting trusted OSC-133 `CommandStarted`.
    /// This is metadata only; PTY input continues through `pending_input`.
    #[cfg(target_os = "macos")]
    pub(in crate::runtime) pending_composer_commands: VecDeque<PendingComposerCommand>,
    #[cfg(target_os = "macos")]
    pub(in crate::runtime) shell_integration_mode: ShellIntegrationMode,
    #[cfg(target_os = "macos")]
    pub(in crate::runtime) block_timeline: CommandBlockTimeline,
    #[cfg(target_os = "macos")]
    pub(in crate::runtime) active_block: Option<CommandBlockId>,
    #[cfg(target_os = "macos")]
    pub(in crate::runtime) active_block_token: Option<ShellIntegrationToken>,
    #[cfg(target_os = "macos")]
    pub(in crate::runtime) block_revision: u64,
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug)]
pub(super) struct PendingComposerCommand {
    pub(super) token: ShellIntegrationToken,
}

#[cfg(target_os = "macos")]
impl PendingComposerCommand {
    pub(super) fn new(token: ShellIntegrationToken, _command: &str) -> Self {
        Self { token }
    }
}

impl Entry {
    pub(super) fn summary(&self, id: ExecutionId) -> ExecutionSummary {
        ExecutionSummary {
            id,
            workspace_id: self.workspace_id,
            attachment_count: self.attachments.len(),
            lifecycle: self.lifecycle.public(),
        }
    }

    pub(super) fn next_deadline(&self) -> Option<std::time::Instant> {
        [
            self.lifecycle.deadline(),
            self.pty_eof_reap_probe.map(|probe| probe.deadline),
        ]
        .into_iter()
        .flatten()
        .min()
    }

    pub(super) fn terminal_io_active(&self) -> bool {
        self.lifecycle.accepts_input() && self.ingress_active.load(Ordering::Acquire)
    }
}
