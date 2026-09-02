use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{Receiver, SyncSender, TryRecvError, sync_channel},
    },
    time::{Duration, Instant},
};

use seyal_exec::{
    ChildExit, CommandSpec, ExecutionReactor, ReactorEvent, ReactorEventKind, ReadOutcome,
    RegistrationToken, SignalDisposition, TerminalExecution, WindowSize, WriteOutcome,
};

#[cfg(target_os = "macos")]
use crate::blocks::{
    BlockId as CommandBlockId, BlockTimeline as CommandBlockTimeline, MAX_COMMAND_BYTES,
};
use crate::{
    AttachmentId, BlockSummary, CapabilityPolicy, ExecutionId, InputIngress, RuntimeError,
    RuntimeId, WorkspaceId,
    block::BlockTimeline as ExecutionBlockTimeline,
    input::{AcceptedInput, ControlMessage},
    singleton::SingletonGuard,
};
#[cfg(target_os = "macos")]
use seyal_exec::{ShellIntegrationEvent, ShellIntegrationToken};

// TEMPORARY diagnostic instrumentation for investigating a CI-only,
// non-locally-reproducible failure in
// detached_output_is_in_authoritative_snapshot_and_exited_child_is_not_recreated
// (pass8_runtime_matrix.rs). Silent unless SEYAL_DEBUG_DRAIN is set; zero
// runtime cost otherwise beyond one relaxed env lookup cached in a OnceLock.
// Must be removed before this PR is considered done -- tracked in
// docs/pass9-merge-todo.md.
fn debug_drain_enabled() -> bool {
    // TEMPORARY: force-enabled because this session's `gh`/git credential
    // lacks the `workflow` OAuth scope needed to set SEYAL_DEBUG_DRAIN in
    // .github/workflows/foundation-quality.yml for one diagnostic CI run.
    // Revert to the env-var gate (or remove entirely) once root-caused.
    true
}

fn debug_drain_epoch() -> Instant {
    static EPOCH: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    *EPOCH.get_or_init(Instant::now)
}

macro_rules! debug_drain {
    ($($arg:tt)*) => {
        if debug_drain_enabled() {
            eprintln!("[drain-debug t={:?}] {}", debug_drain_epoch().elapsed(), format!($($arg)*));
        }
    };
}

#[cfg(target_os = "macos")]
mod local;
#[cfg(target_os = "macos")]
use local::LocalIpcState;

const EVENT_CAPACITY: usize = 128;
const READ_BUFFER_SIZE: usize = 16 * 1024;
const CONTROL_DISPATCH_QUANTUM: usize = 64;
const ROLLBACK_REAP_TICK: Duration = Duration::from_millis(10);
/// A `PrimaryExited` reactor event is kernel-confirmed (`NOTE_EXIT`) but
/// `waitpid(WNOHANG)` can still momentarily report the child as running — the
/// exit notification and reap-ability are not the same instant. This is the
/// retry cadence for re-attempting the reap rather than discarding the
/// one-shot notification and stranding the execution with no deadline.
const PRIMARY_EXIT_REAP_RETRY: Duration = Duration::from_millis(10);
/// PTY EOF is terminal-I/O state, not process-exit truth. A short bounded
/// exponential probe covers the narrow race where a process exits around
/// NOTE_EXIT registration and the first `try_wait` has not become reapable
/// yet. If the child is genuinely still alive, probing stops completely and
/// the still-armed process-exit knote remains authoritative.
const PTY_EOF_REAP_PROBE_INITIAL: Duration = Duration::from_millis(10);
const PTY_EOF_REAP_PROBE_MAX: Duration = Duration::from_millis(320);
const PTY_EOF_REAP_PROBE_LIMIT: u8 = 6;

#[cfg(target_os = "macos")]
fn issue_shell_integration_token() -> Result<ShellIntegrationToken, RuntimeError> {
    let mut token = [0u8; 16];
    let mut source = std::fs::File::open("/dev/urandom")?;
    use std::io::Read;
    source.read_exact(&mut token)?;
    Ok(ShellIntegrationToken::from_bytes(token))
}

#[cfg(all(test, target_os = "macos"))]
mod composer_wrapper_tests {
    use super::*;

    #[test]
    fn zsh_hook_command_binds_markers_to_nonce_without_eval_wrapper() {
        let token = ShellIntegrationToken::from_bytes([0xabu8; 16]);
        let wrapped = zsh_composer_command("printf 'ok'; false", token);
        assert!(wrapped.contains("__seyal_block__ abababababababababababababababab"));
        assert!(wrapped.contains("133;C;%s"));
        assert!(wrapped.contains("133;D;%s;%s"));
        assert!(!wrapped.contains("eval "));
    }

    #[test]
    fn only_zsh_is_block_capable_and_other_shells_remain_raw() {
        assert_eq!(
            shell_integration_mode(&CommandSpec::new("/bin/zsh")),
            ShellIntegrationMode::ZshHook
        );
        assert_eq!(
            shell_integration_mode(&CommandSpec::new("/bin/sh")),
            ShellIntegrationMode::Unsupported
        );
    }

    #[test]
    fn busy_composer_admission_is_a_correlated_result_not_a_transport_error() {
        assert_eq!(ComposerAdmission::Busy, ComposerAdmission::Busy);
        assert_ne!(ComposerAdmission::Busy, ComposerAdmission::Unsupported);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg(target_os = "macos")]
enum ShellIntegrationMode {
    ZshHook,
    Unsupported,
}

#[cfg(target_os = "macos")]
fn shell_integration_mode(command: &CommandSpec) -> ShellIntegrationMode {
    match command.program().to_string_lossy().as_ref() {
        "/bin/zsh" | "zsh" => ShellIntegrationMode::ZshHook,
        _ => ShellIntegrationMode::Unsupported,
    }
}

#[cfg(target_os = "macos")]
fn zsh_composer_command(command: &str, token: ShellIntegrationToken) -> String {
    let mut token_hex = String::with_capacity(32);
    token.write_hex(&mut token_hex);
    format!(
        "if (( ! $+functions[_seyal_block_preexec] )); then autoload -Uz add-zsh-hook; _seyal_active_token=; _seyal_block_preexec() {{ if [[ \"$1\" == __seyal_block__\\ * ]]; then local _seyal_marker=${{1#* }}; _seyal_marker=${{_seyal_marker%%[;\\n]*}}; _seyal_active_token=$_seyal_marker; printf '\\033]133;C;%s\\007' \"$_seyal_active_token\"; fi }}; _seyal_block_precmd() {{ if [[ -n \"$_seyal_active_token\" ]]; then local _seyal_status=$?; printf '\\033]133;D;%s;%s\\007' \"$_seyal_active_token\" \"$_seyal_status\"; _seyal_active_token=; fi }}; add-zsh-hook preexec _seyal_block_preexec; add-zsh-hook precmd _seyal_block_precmd; __seyal_block__() {{ :; }}; fi; __seyal_block__ {token_hex}; {command}"
    )
}

#[derive(Clone, Copy, Debug)]
struct PtyEofReapProbe {
    deadline: Instant,
    delay: Duration,
    remaining: u8,
}

impl PtyEofReapProbe {
    fn new(now: Instant) -> Self {
        Self {
            deadline: now + PTY_EOF_REAP_PROBE_INITIAL,
            delay: PTY_EOF_REAP_PROBE_INITIAL,
            remaining: PTY_EOF_REAP_PROBE_LIMIT,
        }
    }

    fn next(self, now: Instant) -> Option<Self> {
        if self.remaining <= 1 {
            return None;
        }
        let delay = self.delay.saturating_mul(2).min(PTY_EOF_REAP_PROBE_MAX);
        Some(Self {
            deadline: now + delay,
            delay,
            remaining: self.remaining - 1,
        })
    }
}

#[cfg(feature = "benchmark-instrumentation")]
#[derive(Clone, Copy, Debug, Default)]
pub struct BenchmarkRuntimeDiagnostics {
    pub pty_bytes_read: u64,
    pub pty_read_calls: u64,
    pub source_timestamp_samples: usize,
    pub latest_damage_generation: u64,
}

#[cfg(feature = "benchmark-instrumentation")]
#[derive(Default)]
struct BenchmarkRuntimeState {
    pty_bytes_read: u64,
    pty_read_calls: u64,
    source_times: HashMap<(ExecutionId, u64), Instant>,
}

#[derive(Clone, Debug)]
pub enum LocalIpcMode {
    Disabled,
    Enabled {
        runtime_dir_override: Option<PathBuf>,
    },
}

#[derive(Clone, Debug)]
pub struct RuntimeConfig {
    pub singleton_path: PathBuf,
    pub max_executions: usize,
    pub control_queue_capacity: usize,
    pub per_execution_input_bytes: usize,
    pub aggregate_input_bytes: usize,
    pub read_dispatch_bytes: usize,
    pub write_dispatch_bytes: usize,
    pub graceful_termination: Duration,
    pub forced_reap: Duration,
    pub final_drain: Duration,
    pub capability_policy: CapabilityPolicy,
    pub local_ipc: LocalIpcMode,
}

impl RuntimeConfig {
    pub fn m001() -> Result<Self, RuntimeError> {
        Ok(Self {
            singleton_path: std::env::temp_dir().join("seyal").join("runtime.lock"),
            max_executions: 512,
            control_queue_capacity: 1024,
            per_execution_input_bytes: 256 * 1024,
            aggregate_input_bytes: 8 * 1024 * 1024,
            read_dispatch_bytes: 64 * 1024,
            write_dispatch_bytes: 64 * 1024,
            graceful_termination: Duration::from_secs(1),
            forced_reap: Duration::from_secs(1),
            final_drain: Duration::from_millis(250),
            capability_policy: CapabilityPolicy::bundled()?,
            local_ipc: LocalIpcMode::Enabled {
                runtime_dir_override: None,
            },
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutionLifecycle {
    Running,
    TerminatingGraceful,
    TerminatingForced,
    /// The kernel has confirmed the primary process exited (`NOTE_EXIT`),
    /// but the reap (`waitpid`) has not yet completed. Always transient and
    /// retried on a short deadline; never a terminal state.
    PrimaryExitPending,
    DrainingAfterPrimaryExit,
    TerminationFailed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExecutionSummary {
    pub id: ExecutionId,
    pub workspace_id: WorkspaceId,
    pub attachment_count: usize,
    pub lifecycle: ExecutionLifecycle,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum BlockCompletion {
    None,
    Completed(BlockSummary),
    Failed,
}

#[derive(Clone, Copy, Debug)]
enum Lifecycle {
    Running,
    TerminatingGraceful {
        deadline: Instant,
    },
    TerminatingForced {
        deadline: Instant,
    },
    /// See `ExecutionLifecycle::PrimaryExitPending`.
    PrimaryExitPending {
        deadline: Instant,
    },
    DrainingAfterPrimaryExit {
        deadline: Instant,
        exit: ChildExit,
    },
    TerminationFailed,
}

impl Lifecycle {
    fn public(self) -> ExecutionLifecycle {
        match self {
            Self::Running => ExecutionLifecycle::Running,
            Self::TerminatingGraceful { .. } => ExecutionLifecycle::TerminatingGraceful,
            Self::TerminatingForced { .. } => ExecutionLifecycle::TerminatingForced,
            Self::PrimaryExitPending { .. } => ExecutionLifecycle::PrimaryExitPending,
            Self::DrainingAfterPrimaryExit { .. } => ExecutionLifecycle::DrainingAfterPrimaryExit,
            Self::TerminationFailed => ExecutionLifecycle::TerminationFailed,
        }
    }

    fn deadline(self) -> Option<Instant> {
        match self {
            Self::TerminatingGraceful { deadline }
            | Self::TerminatingForced { deadline }
            | Self::PrimaryExitPending { deadline }
            | Self::DrainingAfterPrimaryExit { deadline, .. } => Some(deadline),
            Self::Running | Self::TerminationFailed => None,
        }
    }

    fn accepts_input(self) -> bool {
        matches!(self, Self::Running)
    }
}

/// Result of a Pass 7.1 composer admission attempt. Busy is a correlated
/// application result, not a transport failure, so the Pane keeps its draft
/// and remains connected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg(target_os = "macos")]
pub(crate) enum ComposerAdmission {
    Accepted(CommandBlockId),
    Busy,
    Unsupported,
}

struct Entry {
    execution: TerminalExecution,
    token: RegistrationToken,
    workspace_id: WorkspaceId,
    attachments: HashSet<AttachmentId>,
    lifecycle: Lifecycle,
    pty_eof_reap_probe: Option<PtyEofReapProbe>,
    pending_input: VecDeque<AcceptedInput>,
    reserved_input: Arc<AtomicUsize>,
    ingress_active: Arc<AtomicBool>,
    /// Accepted composer commands awaiting trusted OSC-133 `CommandStarted`.
    /// This is metadata only; PTY input continues through `pending_input`.
    #[cfg(target_os = "macos")]
    pending_composer_commands: VecDeque<PendingComposerCommand>,
    #[cfg(target_os = "macos")]
    shell_integration_mode: ShellIntegrationMode,
    #[cfg(target_os = "macos")]
    block_timeline: CommandBlockTimeline,
    #[cfg(target_os = "macos")]
    active_block: Option<CommandBlockId>,
    #[cfg(target_os = "macos")]
    active_block_token: Option<ShellIntegrationToken>,
    #[cfg(target_os = "macos")]
    block_revision: u64,
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug)]
struct PendingComposerCommand {
    token: ShellIntegrationToken,
}

#[cfg(target_os = "macos")]
impl PendingComposerCommand {
    fn new(token: ShellIntegrationToken, _command: &str) -> Self {
        Self { token }
    }
}

impl Entry {
    fn summary(&self, id: ExecutionId) -> ExecutionSummary {
        ExecutionSummary {
            id,
            workspace_id: self.workspace_id,
            attachment_count: self.attachments.len(),
            lifecycle: self.lifecycle.public(),
        }
    }

    fn next_deadline(&self) -> Option<Instant> {
        [
            self.lifecycle.deadline(),
            self.pty_eof_reap_probe.map(|probe| probe.deadline),
        ]
        .into_iter()
        .flatten()
        .min()
    }

    fn terminal_io_active(&self) -> bool {
        self.lifecycle.accepts_input() && self.ingress_active.load(Ordering::Acquire)
    }
}

pub struct Runtime {
    id: RuntimeId,
    default_workspace: WorkspaceId,
    #[allow(dead_code)]
    singleton: SingletonGuard,
    reactor: ExecutionReactor,
    entries: HashMap<ExecutionId, Entry>,
    execution_blocks: ExecutionBlockTimeline,
    by_token: HashMap<RegistrationToken, ExecutionId>,
    control_tx: SyncSender<ControlMessage>,
    control_rx: Receiver<ControlMessage>,
    aggregate_reserved: Arc<AtomicUsize>,
    config: RuntimeConfig,
    events: [ReactorEvent; EVENT_CAPACITY],
    read_buffer: [u8; READ_BUFFER_SIZE],
    shutting_down: bool,
    rollback_reap: Vec<TerminalExecution>,
    #[cfg(target_os = "macos")]
    local_ipc: Option<LocalIpcState>,
    #[cfg(feature = "benchmark-instrumentation")]
    benchmark: BenchmarkRuntimeState,
}

impl Runtime {
    pub fn new(config: RuntimeConfig) -> Result<Self, RuntimeError> {
        let singleton = SingletonGuard::acquire(&config.singleton_path)?;
        let reactor = ExecutionReactor::new()?;
        #[cfg(target_os = "macos")]
        let mut reactor = reactor;
        let (control_tx, control_rx) = sync_channel(config.control_queue_capacity);
        #[cfg(target_os = "macos")]
        let local_ipc = match &config.local_ipc {
            LocalIpcMode::Disabled => None,
            LocalIpcMode::Enabled {
                runtime_dir_override,
            } => {
                // The singleton-holding Runtime is the sole owner allowed to
                // create/repair its local IPC directory. Client discovery is
                // verification-only and can therefore never race to establish
                // a second endpoint authority.
                let runtime_dir = match runtime_dir_override {
                    Some(dir) => dir.clone(),
                    None => {
                        crate::local_ipc::discovery::darwin_user_runtime_dir().map_err(|_| {
                            RuntimeError::Io(std::io::Error::other("local IPC discovery failed"))
                        })?
                    }
                };
                crate::local_ipc::discovery::create_verified_runtime_dir(&runtime_dir).map_err(
                    |_| {
                        RuntimeError::Io(std::io::Error::other(
                            "local IPC directory creation/verification failed",
                        ))
                    },
                )?;
                Some(LocalIpcState::bind(&mut reactor, Some(runtime_dir))?)
            }
        };
        Ok(Self {
            id: RuntimeId::new(),
            default_workspace: WorkspaceId::m001_default(),
            singleton,
            reactor,
            entries: HashMap::new(),
            execution_blocks: ExecutionBlockTimeline::default(),
            by_token: HashMap::new(),
            control_tx,
            control_rx,
            aggregate_reserved: Arc::new(AtomicUsize::new(0)),
            config,
            events: [ReactorEvent::EMPTY; EVENT_CAPACITY],
            read_buffer: [0; READ_BUFFER_SIZE],
            shutting_down: false,
            rollback_reap: Vec::new(),
            #[cfg(target_os = "macos")]
            local_ipc,
            #[cfg(feature = "benchmark-instrumentation")]
            benchmark: BenchmarkRuntimeState::default(),
        })
    }

    pub fn local_ipc_socket_path(&self) -> Option<&Path> {
        #[cfg(target_os = "macos")]
        {
            self.local_ipc
                .as_ref()
                .map(|state| state.socket_path.as_path())
        }
        #[cfg(not(target_os = "macos"))]
        {
            None
        }
    }

    pub fn id(&self) -> RuntimeId {
        self.id
    }

    pub fn default_workspace_id(&self) -> WorkspaceId {
        self.default_workspace
    }

    pub fn execution_count(&self) -> usize {
        self.entries.len()
    }

    /// Number of retained Pass 8 execution-level metadata records. This is
    /// deliberately distinct from the Pass 7.1 per-command timeline owned by
    /// each macOS execution entry.
    pub fn block_count(&self) -> usize {
        self.execution_blocks.len()
    }

    pub fn block(&self, id: ExecutionId) -> Option<BlockSummary> {
        self.execution_blocks.get(id)
    }

    pub fn aggregate_accepted_but_unwritten_bytes(&self) -> usize {
        self.aggregate_reserved.load(Ordering::Acquire)
    }

    #[cfg(feature = "benchmark-instrumentation")]
    pub fn reset_benchmark_runtime_counters(&mut self) {
        self.benchmark = BenchmarkRuntimeState {
            source_times: HashMap::with_capacity(4096),
            ..BenchmarkRuntimeState::default()
        };
    }

    #[cfg(feature = "benchmark-instrumentation")]
    pub fn benchmark_source_timestamp(
        &self,
        execution_id: ExecutionId,
        generation: u64,
    ) -> Option<Instant> {
        self.benchmark
            .source_times
            .get(&(execution_id, generation))
            .copied()
    }

    #[cfg(feature = "benchmark-instrumentation")]
    pub fn benchmark_runtime_diagnostics(
        &self,
        execution_id: ExecutionId,
    ) -> BenchmarkRuntimeDiagnostics {
        BenchmarkRuntimeDiagnostics {
            pty_bytes_read: self.benchmark.pty_bytes_read,
            pty_read_calls: self.benchmark.pty_read_calls,
            source_timestamp_samples: self
                .benchmark
                .source_times
                .keys()
                .filter(|(id, _)| *id == execution_id)
                .count(),
            latest_damage_generation: self
                .entries
                .get(&execution_id)
                .map_or(0, |entry| entry.execution.terminal().damage_generation()),
        }
    }

    pub fn list(&self) -> Vec<ExecutionSummary> {
        let mut summaries = self
            .entries
            .iter()
            .map(|(&id, entry)| entry.summary(id))
            .collect::<Vec<_>>();
        summaries.sort_by_key(|summary| summary.id);
        summaries
    }

    pub fn lookup(&self, id: ExecutionId) -> Option<ExecutionSummary> {
        self.entries.get(&id).map(|entry| entry.summary(id))
    }

    pub fn execution(&self, id: ExecutionId) -> Option<&TerminalExecution> {
        self.entries.get(&id).map(|entry| &entry.execution)
    }

    pub fn create_execution(
        &mut self,
        command: CommandSpec,
        size: WindowSize,
    ) -> Result<ExecutionId, RuntimeError> {
        if self.shutting_down {
            return Err(RuntimeError::ExecutionNotRunning);
        }
        if self.entries.len() >= self.config.max_executions {
            return Err(RuntimeError::CapacityExceeded);
        }

        let command = self.config.capability_policy.apply(command);
        let mut execution = TerminalExecution::spawn(&command, size)?;
        let initial_primary_line_id = execution.initial_primary_line_id().map(|line| line.0);
        let token = match self.reactor.register(&execution) {
            Ok(token) => token,
            Err(error) => {
                self.kill_unpublished(execution);
                return Err(error.into());
            }
        };

        match execution.try_wait() {
            Ok(Some(exit)) => {
                self.reactor.deregister(token)?;
                return Err(RuntimeError::ChildExitedBeforePublication(exit));
            }
            Ok(None) => {}
            Err(error) => {
                let _ = self.reactor.deregister(token);
                self.kill_unpublished(execution);
                return Err(error.into());
            }
        }

        let id = ExecutionId::new();
        let reserved_input = Arc::new(AtomicUsize::new(0));
        let ingress_active = Arc::new(AtomicBool::new(true));
        let entry = Entry {
            execution,
            token,
            workspace_id: self.default_workspace,
            attachments: HashSet::new(),
            lifecycle: Lifecycle::Running,
            pty_eof_reap_probe: None,
            pending_input: VecDeque::new(),
            reserved_input,
            ingress_active,
            #[cfg(target_os = "macos")]
            pending_composer_commands: VecDeque::new(),
            #[cfg(target_os = "macos")]
            shell_integration_mode: shell_integration_mode(&command),
            #[cfg(target_os = "macos")]
            block_timeline: CommandBlockTimeline::default(),
            #[cfg(target_os = "macos")]
            active_block: None,
            #[cfg(target_os = "macos")]
            active_block_token: None,
            #[cfg(target_os = "macos")]
            block_revision: 0,
        };
        let previous = self.by_token.insert(token, id);
        debug_assert!(previous.is_none());
        let previous = self.entries.insert(id, entry);
        debug_assert!(previous.is_none());

        // Pass 8 execution metadata is optional Workspace presentation
        // metadata. Admission happens only after TerminalExecution publication
        // and can never roll back or terminate an otherwise valid execution.
        if let Some(start_line_id) = initial_primary_line_id {
            let _ = self
                .execution_blocks
                .admit(self.default_workspace, id, start_line_id);
        }
        Ok(id)
    }

    pub fn input_ingress(&self, id: ExecutionId) -> Result<InputIngress, RuntimeError> {
        let entry = self
            .entries
            .get(&id)
            .ok_or(RuntimeError::UnknownExecution)?;
        if !entry.terminal_io_active() {
            return Err(RuntimeError::ExecutionNotRunning);
        }
        Ok(InputIngress::new(
            id,
            Arc::clone(&entry.ingress_active),
            self.control_tx.clone(),
            self.reactor.waker(),
            Arc::clone(&self.aggregate_reserved),
            self.config.aggregate_input_bytes,
            Arc::clone(&entry.reserved_input),
            self.config.per_execution_input_bytes,
        ))
    }

    /// Admit one complete Pane-composer command. This deliberately uses a
    /// distinct Runtime operation from raw terminal input: only a trusted
    /// OSC-133 start event can turn this pending metadata into a Block.
    #[cfg(target_os = "macos")]
    pub(crate) fn submit_composer_command(
        &mut self,
        id: ExecutionId,
        command: String,
    ) -> Result<ComposerAdmission, RuntimeError> {
        if command.is_empty() || command.len() > MAX_COMMAND_BYTES {
            return Err(RuntimeError::CapacityExceeded);
        }
        let can_admit = self.entries.get(&id).is_some_and(|entry| {
            entry.terminal_io_active()
                && entry.pending_composer_commands.is_empty()
                && entry.active_block.is_none()
        });
        if !can_admit {
            return Ok(ComposerAdmission::Busy);
        }
        let mode = self
            .entries
            .get(&id)
            .map(|entry| entry.shell_integration_mode)
            .ok_or(RuntimeError::UnknownExecution)?;
        if mode == ShellIntegrationMode::Unsupported {
            // Unsupported shells remain fully usable through the ordinary raw
            // PTY path, but never receive synthetic Block metadata.
            let mut bytes = Vec::with_capacity(command.len() + 1);
            bytes.extend_from_slice(command.as_bytes());
            bytes.push(b'\r');
            self.input_ingress(id)?.try_submit(bytes)?;
            return Ok(ComposerAdmission::Unsupported);
        }
        let token = issue_shell_integration_token()?;
        let wrapped = zsh_composer_command(&command, token);
        let mut bytes = Vec::with_capacity(wrapped.len() + 1);
        bytes.extend_from_slice(wrapped.as_bytes());
        bytes.push(b'\r');
        self.input_ingress(id)?.try_submit(bytes)?;
        let entry = self
            .entries
            .get_mut(&id)
            .ok_or(RuntimeError::UnknownExecution)?;
        let cursor = entry.execution.terminal().cursor();
        let start_line = entry
            .execution
            .terminal()
            .line_id(cursor.row)
            .map(|line| line.0)
            .unwrap_or(1);
        let block_id = entry
            .block_timeline
            .start(command.clone(), start_line)
            .map_err(|_| RuntimeError::CapacityExceeded)?;
        entry.active_block = Some(block_id);
        // The control admission above is bounded and synchronous. Record only
        // after success so rejected composer input cannot manufacture a Block.
        entry
            .pending_composer_commands
            .push_back(PendingComposerCommand::new(token, &command));
        Ok(ComposerAdmission::Accepted(block_id))
    }

    pub fn attach(&mut self, id: ExecutionId) -> Result<AttachmentId, RuntimeError> {
        let entry = self
            .entries
            .get_mut(&id)
            .ok_or(RuntimeError::UnknownExecution)?;
        let attachment = AttachmentId::new();
        entry.attachments.insert(attachment);
        Ok(attachment)
    }

    pub fn detach(
        &mut self,
        id: ExecutionId,
        attachment: AttachmentId,
    ) -> Result<(), RuntimeError> {
        let entry = self
            .entries
            .get_mut(&id)
            .ok_or(RuntimeError::UnknownExecution)?;
        if !entry.attachments.remove(&attachment) {
            return Err(RuntimeError::UnknownAttachment);
        }
        Ok(())
    }

    pub fn resize(&mut self, id: ExecutionId, size: WindowSize) -> Result<(), RuntimeError> {
        let entry = self
            .entries
            .get_mut(&id)
            .ok_or(RuntimeError::UnknownExecution)?;
        if !entry.terminal_io_active() {
            return Err(RuntimeError::ExecutionNotRunning);
        }
        entry.execution.resize(size)?;
        Ok(())
    }

    pub fn request_termination(&mut self, id: ExecutionId) -> Result<(), RuntimeError> {
        let now = Instant::now();
        let entry = self
            .entries
            .get_mut(&id)
            .ok_or(RuntimeError::UnknownExecution)?;
        if !matches!(entry.lifecycle, Lifecycle::Running) {
            return Ok(());
        }
        entry.pty_eof_reap_probe = None;
        entry.ingress_active.store(false, Ordering::Release);
        entry.pending_input.clear();
        self.reactor.set_writable(entry.token, false)?;
        match entry.execution.signal_terminate()? {
            SignalDisposition::AlreadyReaped(exit) => {
                entry.lifecycle = Lifecycle::DrainingAfterPrimaryExit {
                    deadline: now + self.config.final_drain,
                    exit,
                };
            }
            SignalDisposition::Delivered | SignalDisposition::ProcessGone => {
                entry.lifecycle = Lifecycle::TerminatingGraceful {
                    deadline: now + self.config.graceful_termination,
                };
            }
        }
        Ok(())
    }

    pub fn begin_shutdown(&mut self) -> Result<(), RuntimeError> {
        if self.shutting_down {
            return Ok(());
        }
        self.shutting_down = true;
        let ids = self.entries.keys().copied().collect::<Vec<_>>();
        for id in ids {
            self.request_termination(id)?;
        }
        self.reactor.waker().wake()?;
        Ok(())
    }

    pub fn shutdown_complete(&self) -> bool {
        self.shutting_down
            && self.entries.is_empty()
            && self.execution_blocks.len() == 0
            && self.rollback_reap.is_empty()
    }

    pub fn poll_once(&mut self, max_wait: Option<Duration>) -> Result<usize, RuntimeError> {
        self.reap_failed_creations()?;
        let timeout = self.bound_wait_by_deadline(max_wait);
        let count = self.reactor.wait(&mut self.events, timeout)?;
        let mut processed = 0usize;
        for index in 0..count {
            let event = self.events[index];
            match event.kind {
                ReactorEventKind::Control => {
                    processed += self.drain_control()?;
                }
                ReactorEventKind::Readable => {
                    if let Some(id) = event
                        .token
                        .and_then(|token| self.by_token.get(&token).copied())
                    {
                        self.service_reads(id)?;
                        processed += 1;
                    }
                }
                ReactorEventKind::Writable => {
                    if let Some(id) = event
                        .token
                        .and_then(|token| self.by_token.get(&token).copied())
                    {
                        self.service_writes(id)?;
                        processed += 1;
                    }
                }
                ReactorEventKind::PrimaryExited => {
                    if let Some(id) = event
                        .token
                        .and_then(|token| self.by_token.get(&token).copied())
                    {
                        self.observe_primary_exit(id)?;
                        processed += 1;
                    }
                }
                ReactorEventKind::AuxiliaryReadable | ReactorEventKind::AuxiliaryWritable =>
                {
                    #[cfg(target_os = "macos")]
                    if let Some(token) = event.token {
                        self.service_local_reactor_event(token, event.kind, event.hangup)?;
                        processed += 1;
                    }
                }
            }
        }
        self.process_deadlines()?;
        self.reap_failed_creations()?;
        #[cfg(target_os = "macos")]
        self.publish_display_updates();
        Ok(processed)
    }

    pub fn run_until_empty(&mut self, overall_deadline: Instant) -> Result<(), RuntimeError> {
        while !self.entries.is_empty() || !self.rollback_reap.is_empty() {
            let now = Instant::now();
            if now >= overall_deadline {
                return Err(RuntimeError::ShutdownIncomplete);
            }
            self.poll_once(Some(overall_deadline.saturating_duration_since(now)))?;
        }
        Ok(())
    }

    fn drain_control(&mut self) -> Result<usize, RuntimeError> {
        let mut handled = 0usize;
        while handled < CONTROL_DISPATCH_QUANTUM {
            match self.control_rx.try_recv() {
                Ok(ControlMessage::Input(input)) => {
                    let id = input.execution_id;
                    if let Some(entry) = self.entries.get_mut(&id)
                        && entry.terminal_io_active()
                    {
                        entry.pending_input.push_back(input);
                        self.service_writes(id)?;
                    }
                    handled += 1;
                }
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
            }
        }
        if handled == CONTROL_DISPATCH_QUANTUM {
            self.reactor.waker().wake()?;
        }
        Ok(handled)
    }

    fn mark_terminal_io_closed(&mut self, id: ExecutionId) -> Result<(), RuntimeError> {
        let token = {
            let entry = self
                .entries
                .get_mut(&id)
                .ok_or(RuntimeError::UnknownExecution)?;
            entry.ingress_active.store(false, Ordering::Release);
            entry.pending_input.clear();
            entry.token
        };
        self.reactor.set_writable(token, false)?;
        self.reactor.set_readable(token, false)?;
        Ok(())
    }

    fn service_reads(&mut self, id: ExecutionId) -> Result<(), RuntimeError> {
        let mut consumed = 0usize;
        let mut drain_complete = false;
        while consumed < self.config.read_dispatch_bytes {
            let remaining = self.config.read_dispatch_bytes - consumed;
            let read_len = remaining.min(self.read_buffer.len());
            let (outcome, _generation_before, _generation_after) = {
                let entry = self
                    .entries
                    .get_mut(&id)
                    .ok_or(RuntimeError::UnknownExecution)?;
                let generation_before = entry.execution.terminal().damage_generation();
                let outcome = entry
                    .execution
                    .read_output(&mut self.read_buffer[..read_len])?;
                let generation_after = entry.execution.terminal().damage_generation();
                (outcome, generation_before, generation_after)
            };
            debug_drain!(
                "service_reads id={id:?} outcome={outcome:?} lifecycle={:?}",
                self.entries.get(&id).map(|entry| entry.lifecycle)
            );
            match outcome {
                ReadOutcome::Eof => {
                    // PTY EOF proves only that terminal I/O is gone. A child
                    // may deliberately close fd 0/1/2 and remain alive, so do
                    // not mutate process lifecycle from this signal. Disarm
                    // the level-triggered read filter while preserving the
                    // independently registered process-exit watch.
                    self.mark_terminal_io_closed(id)?;
                    if matches!(
                        self.entries.get(&id).map(|entry| entry.lifecycle),
                        Some(Lifecycle::Running | Lifecycle::PrimaryExitPending { .. })
                    ) {
                        let exit = {
                            let entry = self
                                .entries
                                .get_mut(&id)
                                .ok_or(RuntimeError::UnknownExecution)?;
                            entry.execution.try_wait()?
                        };
                        if let Some(exit) = exit {
                            self.enter_drain(id, exit)?;
                        } else if let Some(entry) = self.entries.get_mut(&id)
                            && matches!(entry.lifecycle, Lifecycle::Running)
                            && entry.pty_eof_reap_probe.is_none()
                        {
                            entry.pty_eof_reap_probe = Some(PtyEofReapProbe::new(Instant::now()));
                        }
                    }
                    drain_complete = matches!(
                        self.entries.get(&id).map(|entry| entry.lifecycle),
                        Some(Lifecycle::DrainingAfterPrimaryExit { .. })
                    );
                    break;
                }
                ReadOutcome::Bytes(0) | ReadOutcome::WouldBlock => {
                    // A temporary empty nonblocking PTY read is not end-of-file.
                    // After the primary process exits, descendants or the PTY
                    // discipline may still make final tail bytes readable during
                    // the bounded final-drain window. Only EOF may finalize early;
                    // otherwise keep the execution until the drain deadline.
                    break;
                }
                ReadOutcome::Bytes(count) => {
                    consumed += count;
                    self.observe_shell_integration_events(id)?;
                    #[cfg(feature = "benchmark-instrumentation")]
                    {
                        self.benchmark.pty_bytes_read =
                            self.benchmark.pty_bytes_read.saturating_add(count as u64);
                        self.benchmark.pty_read_calls =
                            self.benchmark.pty_read_calls.saturating_add(1);
                        if _generation_after > _generation_before {
                            self.benchmark
                                .source_times
                                .insert((id, _generation_after), Instant::now());
                        }
                    }
                }
            }
        }
        if drain_complete {
            self.finalize(id)?;
        }
        Ok(())
    }

    /// Consume bounded canonical parser events after their bytes were applied
    /// to TerminalState. The Runtime records only trusted anchors; this path
    /// never reads a prompt, row text, or terminal cell payload.
    #[cfg(target_os = "macos")]
    fn observe_shell_integration_events(&mut self, id: ExecutionId) -> Result<(), RuntimeError> {
        let mut changed = false;
        {
            let entry = self
                .entries
                .get_mut(&id)
                .ok_or(RuntimeError::UnknownExecution)?;
            while let Some(event) = entry.execution.take_shell_integration_event() {
                let cursor = entry.execution.terminal().cursor();
                let Some(line_id) = entry.execution.terminal().line_id(cursor.row) else {
                    continue;
                };
                match event {
                    ShellIntegrationEvent::CommandStarted { token } => {
                        let Some(position) = entry
                            .pending_composer_commands
                            .iter()
                            .position(|pending| pending.token == token)
                        else {
                            // Direct/raw shell input remains intentionally
                            // unblocked and produces no guessed Block.
                            continue;
                        };
                        let pending = entry
                            .pending_composer_commands
                            .remove(position)
                            .expect("pending composer position remains valid");
                        if entry.active_block.is_some() {
                            entry.active_block_token = Some(pending.token);
                            changed = true;
                        }
                    }
                    ShellIntegrationEvent::CommandFinished { token, exit_status } => {
                        let Some(block_id) = entry.active_block else {
                            continue;
                        };
                        if entry.active_block_token != Some(token) {
                            continue;
                        }
                        if entry
                            .block_timeline
                            .complete(block_id, line_id.0, exit_status)
                            .is_ok()
                        {
                            entry.active_block = None;
                            entry.active_block_token = None;
                            changed = true;
                        }
                    }
                }
            }
            if changed {
                entry.block_revision = entry.block_revision.saturating_add(1);
            }
        }
        if changed {
            self.publish_block_timeline(id);
        }
        Ok(())
    }

    /// Non-macOS runtimes do not expose the local composer/block route, but
    /// still drain parser events so a raw execution cannot retain a bounded
    /// queue of shell-integration notifications indefinitely.
    #[cfg(not(target_os = "macos"))]
    fn observe_shell_integration_events(&mut self, id: ExecutionId) -> Result<(), RuntimeError> {
        let entry = self
            .entries
            .get_mut(&id)
            .ok_or(RuntimeError::UnknownExecution)?;
        while entry.execution.take_shell_integration_event().is_some() {}
        Ok(())
    }

    fn service_writes(&mut self, id: ExecutionId) -> Result<(), RuntimeError> {
        let mut written_total = 0usize;
        let token;
        let pending;
        {
            let entry = self
                .entries
                .get_mut(&id)
                .ok_or(RuntimeError::UnknownExecution)?;
            if !entry.terminal_io_active() {
                entry.pending_input.clear();
                token = entry.token;
                pending = false;
            } else {
                while written_total < self.config.write_dispatch_bytes {
                    let Some(front) = entry.pending_input.front_mut() else {
                        break;
                    };
                    let quantum = self.config.write_dispatch_bytes - written_total;
                    let slice = front.remaining();
                    let slice = &slice[..slice.len().min(quantum)];
                    match entry.execution.write_input(slice)? {
                        WriteOutcome::Bytes(0) | WriteOutcome::WouldBlock => break,
                        WriteOutcome::Bytes(count) => {
                            front.consume(count);
                            written_total += count;
                            if front.is_empty() {
                                entry.pending_input.pop_front();
                            }
                        }
                    }
                }
                token = entry.token;
                pending = !entry.pending_input.is_empty();
            }
        }
        self.reactor.set_writable(token, pending)?;
        Ok(())
    }

    fn observe_primary_exit(&mut self, id: ExecutionId) -> Result<(), RuntimeError> {
        debug_drain!("observe_primary_exit id={id:?}");
        let exit = {
            let entry = self
                .entries
                .get_mut(&id)
                .ok_or(RuntimeError::UnknownExecution)?;
            entry.pty_eof_reap_probe = None;
            entry.execution.try_wait()?
        };
        debug_drain!("observe_primary_exit id={id:?} try_wait={exit:?}");
        if let Some(exit) = exit {
            self.enter_drain(id, exit)?;
            debug_drain!("observe_primary_exit id={id:?} post-enter_drain pre-service_reads");
            self.service_reads(id)?;
            debug_drain!("observe_primary_exit id={id:?} post-service_reads");
        } else if let Some(entry) = self.entries.get_mut(&id)
            && matches!(entry.lifecycle, Lifecycle::Running)
        {
            // `PrimaryExited` is one-shot and will never repeat for this
            // registration. Unlike PTY EOF, this event is kernel-confirmed
            // process-exit truth, so a short reap retry is a valid lifecycle
            // transition rather than terminal-I/O state leakage.
            entry.lifecycle = Lifecycle::PrimaryExitPending {
                deadline: Instant::now() + PRIMARY_EXIT_REAP_RETRY,
            };
        }
        Ok(())
    }

    fn enter_drain(&mut self, id: ExecutionId, exit: ChildExit) -> Result<(), RuntimeError> {
        let entry = self
            .entries
            .get_mut(&id)
            .ok_or(RuntimeError::UnknownExecution)?;
        entry.pty_eof_reap_probe = None;
        entry.ingress_active.store(false, Ordering::Release);
        entry.pending_input.clear();
        self.reactor.set_writable(entry.token, false)?;
        entry.lifecycle = Lifecycle::DrainingAfterPrimaryExit {
            deadline: Instant::now() + self.config.final_drain,
            exit,
        };
        debug_drain!(
            "enter_drain id={id:?} exit={exit:?} final_drain={:?}",
            self.config.final_drain
        );
        Ok(())
    }

    fn process_deadlines(&mut self) -> Result<(), RuntimeError> {
        let now = Instant::now();

        let probe_due = self
            .entries
            .iter()
            .filter_map(|(&id, entry)| {
                entry
                    .pty_eof_reap_probe
                    .filter(|probe| probe.deadline <= now)
                    .map(|_| id)
            })
            .collect::<Vec<_>>();
        for id in probe_due {
            if !matches!(
                self.entries.get(&id).map(|entry| entry.lifecycle),
                Some(Lifecycle::Running)
            ) {
                if let Some(entry) = self.entries.get_mut(&id) {
                    entry.pty_eof_reap_probe = None;
                }
                continue;
            }
            let exit = {
                let entry = self
                    .entries
                    .get_mut(&id)
                    .ok_or(RuntimeError::UnknownExecution)?;
                entry.execution.try_wait()?
            };
            if let Some(exit) = exit {
                self.enter_drain(id, exit)?;
                self.service_reads(id)?;
            } else if let Some(entry) = self.entries.get_mut(&id) {
                entry.pty_eof_reap_probe =
                    entry.pty_eof_reap_probe.and_then(|probe| probe.next(now));
            }
        }

        let due = self
            .entries
            .iter()
            .filter_map(|(&id, entry)| entry.lifecycle.deadline().filter(|d| *d <= now).map(|_| id))
            .collect::<Vec<_>>();
        for id in due {
            let lifecycle = self.entries.get(&id).map(|entry| entry.lifecycle);
            match lifecycle {
                Some(Lifecycle::TerminatingGraceful { .. }) => {
                    let exit = {
                        let entry = self
                            .entries
                            .get_mut(&id)
                            .ok_or(RuntimeError::UnknownExecution)?;
                        entry.execution.try_wait()?
                    };
                    if let Some(exit) = exit {
                        self.enter_drain(id, exit)?;
                        self.service_reads(id)?;
                        continue;
                    }
                    let entry = self
                        .entries
                        .get_mut(&id)
                        .ok_or(RuntimeError::UnknownExecution)?;
                    match entry.execution.signal_kill()? {
                        SignalDisposition::AlreadyReaped(exit) => {
                            entry.lifecycle = Lifecycle::DrainingAfterPrimaryExit {
                                deadline: now + self.config.final_drain,
                                exit,
                            };
                        }
                        SignalDisposition::Delivered | SignalDisposition::ProcessGone => {
                            entry.lifecycle = Lifecycle::TerminatingForced {
                                deadline: now + self.config.forced_reap,
                            };
                        }
                    }
                }
                Some(Lifecycle::TerminatingForced { .. }) => {
                    let exit = {
                        let entry = self
                            .entries
                            .get_mut(&id)
                            .ok_or(RuntimeError::UnknownExecution)?;
                        entry.execution.try_wait()?
                    };
                    if let Some(exit) = exit {
                        self.enter_drain(id, exit)?;
                        self.service_reads(id)?;
                    } else if let Some(entry) = self.entries.get_mut(&id) {
                        entry.lifecycle = Lifecycle::TerminationFailed;
                    }
                }
                Some(Lifecycle::PrimaryExitPending { .. }) => {
                    let exit = {
                        let entry = self
                            .entries
                            .get_mut(&id)
                            .ok_or(RuntimeError::UnknownExecution)?;
                        entry.execution.try_wait()?
                    };
                    if let Some(exit) = exit {
                        self.enter_drain(id, exit)?;
                        self.service_reads(id)?;
                    } else if let Some(entry) = self.entries.get_mut(&id) {
                        entry.lifecycle = Lifecycle::PrimaryExitPending {
                            deadline: now + PRIMARY_EXIT_REAP_RETRY,
                        };
                    }
                }
                Some(Lifecycle::DrainingAfterPrimaryExit { exit, .. }) => {
                    let _ = exit;
                    debug_drain!("process_deadlines: drain deadline fired id={id:?}");
                    // Close the final-drain race: readiness and the deadline may
                    // become observable in the same scheduling turn. Give the PTY
                    // one last bounded production read before publishing the final
                    // display and completing execution metadata. EOF may finalize
                    // inside service_reads; otherwise the deadline remains the hard
                    // upper bound and finalize retires the execution below.
                    self.service_reads(id)?;
                    if self.entries.contains_key(&id) {
                        debug_drain!(
                            "process_deadlines: finalizing after deadline (not already finalized inside service_reads) id={id:?}"
                        );
                        self.finalize(id)?;
                    }
                }
                Some(Lifecycle::Running | Lifecycle::TerminationFailed) | None => {}
            }
        }

        #[cfg(target_os = "macos")]
        self.service_local_deadline(now)?;
        Ok(())
    }

    fn finalize(&mut self, id: ExecutionId) -> Result<(), RuntimeError> {
        debug_drain!("finalize id={id:?}");
        // The final PTY drain may have changed canonical state in this same
        // scheduling turn. Publish that state before completing Pass 8 Block
        // metadata and before any lifecycle-finalized notification.
        #[cfg(target_os = "macos")]
        {
            self.publish_display_updates();
            // Resync recovery is intentionally budgeted and may still be queued
            // even when this final turn has no fresh projection update. Admit an
            // authoritative current snapshot for every attached client before
            // Block completion and Lifecycle::Finalized are queued.
            self.publish_final_display_snapshot(id);
        }

        let Some(workspace_id) = self.entries.get(&id).map(|entry| entry.workspace_id) else {
            return Ok(());
        };
        let block_completion = match self.execution_blocks.complete(workspace_id, id) {
            Ok(Some(record)) => BlockCompletion::Completed(record),
            Ok(None) => BlockCompletion::None,
            Err(_) => BlockCompletion::Failed,
        };

        let Some(mut entry) = self.entries.remove(&id) else {
            self.execution_blocks.retire(id);
            return Ok(());
        };
        entry.ingress_active.store(false, Ordering::Release);
        entry.pending_input.clear();
        self.by_token.remove(&entry.token);
        let deregister_result = self.reactor.deregister(entry.token);
        drop(entry);

        #[cfg(target_os = "macos")]
        self.notify_local_ipc_execution_finalized(id, block_completion);
        #[cfg(not(target_os = "macos"))]
        match block_completion {
            BlockCompletion::Completed(record) => {
                let _ = record;
            }
            BlockCompletion::None | BlockCompletion::Failed => {}
        }

        // M001 retains no completed execution-level Block history. Retirement
        // happens in this same bounded turn after per-connection finalization
        // bytes were either admitted or the affected connection failed closed.
        self.execution_blocks.retire(id);
        deregister_result?;
        Ok(())
    }

    fn bound_wait_by_deadline(&self, requested: Option<Duration>) -> Option<Duration> {
        let now = Instant::now();
        let execution = self
            .entries
            .values()
            .filter_map(Entry::next_deadline)
            .min()
            .map(|deadline| deadline.saturating_duration_since(now));
        let rollback = (!self.rollback_reap.is_empty()).then_some(ROLLBACK_REAP_TICK);
        #[cfg(target_os = "macos")]
        let local = self
            .local_ipc_deadline()
            .map(|deadline| deadline.saturating_duration_since(now));
        #[cfg(not(target_os = "macos"))]
        let local: Option<Duration> = None;
        [requested, execution, rollback, local]
            .into_iter()
            .flatten()
            .min()
    }

    fn kill_unpublished(&mut self, mut execution: TerminalExecution) {
        let _ = execution.signal_kill();
        match execution.try_wait() {
            Ok(Some(_)) => {}
            Ok(None) | Err(_) => {
                self.rollback_reap.push(execution);
                let _ = self.reactor.waker().wake();
            }
        }
    }

    fn reap_failed_creations(&mut self) -> Result<(), RuntimeError> {
        let mut index = 0;
        while index < self.rollback_reap.len() {
            match self.rollback_reap[index].try_wait()? {
                Some(_) => {
                    self.rollback_reap.swap_remove(index);
                }
                None => index += 1,
            }
        }
        Ok(())
    }
}
