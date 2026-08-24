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

use crate::{
    AttachmentId, CapabilityPolicy, ExecutionId, InputIngress, ProjectionId, RuntimeError,
    RuntimeId, WorkspaceId,
    input::{AcceptedInput, ControlMessage},
    local_ipc::{
        attachment::AttachmentRegistry,
        connection::{ConnectionState as LocalIpcConnState, LocalIpcServer, ServerEvent},
        discovery,
        framing::{
            self, Attach as WireAttach, Attached as WireAttached, ErrorCode, ExecutionList,
            ExecutionListEntry, GenerationWake as WireGenerationWake, Lifecycle as WireLifecycle,
            MessageType, ProjectionReplaced as WireProjectionReplaced, Resize as WireResize, Role,
        },
    },
    projection::{
        layout::{MAX_CAPACITY_COLS, MAX_CAPACITY_ROWS, REGION_HEADER_LEN, RegionHeader},
        lifecycle::ProjectionRegion,
        producer,
        writer::Writer,
    },
    singleton::SingletonGuard,
};

const EVENT_CAPACITY: usize = 128;
const READ_BUFFER_SIZE: usize = 16 * 1024;
const CONTROL_DISPATCH_QUANTUM: usize = 64;
const ROLLBACK_REAP_TICK: Duration = Duration::from_millis(10);
/// Poll interval used only while at least one local control connection is
/// open, so control-plane messages are serviced with bounded latency
/// without busy-polling an idle Runtime that has no local clients at all.
/// See the module-level note on `Runtime::bound_wait_by_deadline` for the
/// known limitation this works around (two independent kqueues).
const LOCAL_IPC_POLL_INTERVAL: Duration = Duration::from_millis(20);

#[derive(Clone, Debug)]
pub enum LocalIpcMode {
    /// SPEC-004 local attachment protocol/projection is not started.
    Disabled,
    /// Bind the SPEC-004 control socket. `runtime_dir_override` lets tests
    /// avoid colliding on the real per-user discovery path the same way
    /// `RuntimeConfig::singleton_path` already does for the Pass-4 lock.
    Enabled { runtime_dir_override: Option<PathBuf> },
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
enum Lifecycle {
    Running,
    TerminatingGraceful { deadline: Instant },
    TerminatingForced { deadline: Instant },
    DrainingAfterPrimaryExit { deadline: Instant, exit: ChildExit },
    TerminationFailed,
}

impl Lifecycle {
    fn public(self) -> ExecutionLifecycle {
        match self {
            Self::Running => ExecutionLifecycle::Running,
            Self::TerminatingGraceful { .. } => ExecutionLifecycle::TerminatingGraceful,
            Self::TerminatingForced { .. } => ExecutionLifecycle::TerminatingForced,
            Self::DrainingAfterPrimaryExit { .. } => ExecutionLifecycle::DrainingAfterPrimaryExit,
            Self::TerminationFailed => ExecutionLifecycle::TerminationFailed,
        }
    }

    fn deadline(self) -> Option<Instant> {
        match self {
            Self::TerminatingGraceful { deadline }
            | Self::TerminatingForced { deadline }
            | Self::DrainingAfterPrimaryExit { deadline, .. } => Some(deadline),
            Self::Running | Self::TerminationFailed => None,
        }
    }

    fn accepts_input(self) -> bool {
        matches!(self, Self::Running)
    }
}

struct ConnectionMeta {
    attachment: Option<AttachmentId>,
}

struct ProjectionEntry {
    execution_id: ExecutionId,
    token: u64,
    region: ProjectionRegion,
    writer: Writer,
    projection_id: ProjectionId,
    capacity_rows: u16,
    capacity_cols: u16,
}

/// SPEC-004 local attachment protocol/projection state. This is entirely
/// best-effort/optional relative to Pass 1-4 terminal execution: a bind
/// failure here never prevents `Runtime::new` from succeeding, and no
/// method on this type ever blocks or is consulted on the PTY hot path.
struct LocalIpcState {
    server: LocalIpcServer,
    socket_path: PathBuf,
    attachments: AttachmentRegistry,
    connections: HashMap<u64, ConnectionMeta>,
    projections: HashMap<AttachmentId, ProjectionEntry>,
}

impl LocalIpcState {
    fn bind(runtime_dir_override: Option<PathBuf>) -> Result<Self, RuntimeError> {
        let runtime_dir = match runtime_dir_override {
            Some(dir) => dir,
            None => discovery::darwin_user_runtime_dir()
                .map_err(|_| RuntimeError::Io(std::io::Error::other("local IPC discovery failed")))?,
        };
        discovery::ensure_verified_runtime_dir(&runtime_dir)
            .map_err(|_| RuntimeError::Io(std::io::Error::other("local IPC directory verification failed")))?;
        let socket_path = discovery::control_socket_path(&runtime_dir)
            .map_err(|_| RuntimeError::Io(std::io::Error::other("local IPC socket path invalid")))?;
        discovery::verify_stale_socket_before_unlink(&socket_path)
            .map_err(|_| RuntimeError::Io(std::io::Error::other("stale local IPC socket verification failed")))?;
        let _ = std::fs::remove_file(&socket_path);
        let server = LocalIpcServer::bind(&socket_path, crate::local_ipc::connection::MAX_CONNECTIONS)?;
        Ok(Self {
            server,
            socket_path,
            attachments: AttachmentRegistry::new(),
            connections: HashMap::new(),
            projections: HashMap::new(),
        })
    }
}

impl Drop for LocalIpcState {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

struct Entry {
    execution: TerminalExecution,
    token: RegistrationToken,
    workspace_id: WorkspaceId,
    attachments: HashSet<AttachmentId>,
    lifecycle: Lifecycle,
    pending_input: VecDeque<AcceptedInput>,
    reserved_input: Arc<AtomicUsize>,
    ingress_active: Arc<AtomicBool>,
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
}

pub struct Runtime {
    id: RuntimeId,
    default_workspace: WorkspaceId,
    #[allow(dead_code)]
    singleton: SingletonGuard,
    reactor: ExecutionReactor,
    entries: HashMap<ExecutionId, Entry>,
    by_token: HashMap<RegistrationToken, ExecutionId>,
    control_tx: SyncSender<ControlMessage>,
    control_rx: Receiver<ControlMessage>,
    aggregate_reserved: Arc<AtomicUsize>,
    config: RuntimeConfig,
    events: [ReactorEvent; EVENT_CAPACITY],
    read_buffer: [u8; READ_BUFFER_SIZE],
    shutting_down: bool,
    rollback_reap: Vec<TerminalExecution>,
    local_ipc: Option<LocalIpcState>,
}

impl Runtime {
    pub fn new(config: RuntimeConfig) -> Result<Self, RuntimeError> {
        let singleton = SingletonGuard::acquire(&config.singleton_path)?;
        let reactor = ExecutionReactor::new()?;
        let (control_tx, control_rx) = sync_channel(config.control_queue_capacity);
        let local_ipc = match &config.local_ipc {
            LocalIpcMode::Disabled => None,
            LocalIpcMode::Enabled { runtime_dir_override } => {
                // Best-effort: a bind failure (permissions, sandboxed CI,
                // etc.) never prevents Pass 1-4 terminal execution from
                // working. `local_ipc_socket_path()` reports `None` in
                // that case so callers/tests can detect and skip Pass-5
                // behavior explicitly rather than silently hanging.
                LocalIpcState::bind(runtime_dir_override.clone()).ok()
            }
        };
        Ok(Self {
            id: RuntimeId::new(),
            default_workspace: WorkspaceId::m001_default(),
            singleton,
            reactor,
            entries: HashMap::new(),
            by_token: HashMap::new(),
            control_tx,
            control_rx,
            aggregate_reserved: Arc::new(AtomicUsize::new(0)),
            config,
            events: [ReactorEvent::EMPTY; EVENT_CAPACITY],
            read_buffer: [0; READ_BUFFER_SIZE],
            shutting_down: false,
            rollback_reap: Vec::new(),
            local_ipc,
        })
    }

    /// The bound SPEC-004 control-socket path, if local attachment is
    /// enabled and successfully bound.
    pub fn local_ipc_socket_path(&self) -> Option<&Path> {
        self.local_ipc.as_ref().map(|state| state.socket_path.as_path())
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

    pub fn aggregate_accepted_but_unwritten_bytes(&self) -> usize {
        self.aggregate_reserved.load(Ordering::Acquire)
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
            pending_input: VecDeque::new(),
            reserved_input,
            ingress_active,
        };
        let previous = self.by_token.insert(token, id);
        debug_assert!(previous.is_none());
        let previous = self.entries.insert(id, entry);
        debug_assert!(previous.is_none());
        Ok(id)
    }

    pub fn input_ingress(&self, id: ExecutionId) -> Result<InputIngress, RuntimeError> {
        let entry = self
            .entries
            .get(&id)
            .ok_or(RuntimeError::UnknownExecution)?;
        if !entry.lifecycle.accepts_input() {
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
        if !matches!(entry.lifecycle, Lifecycle::Running) {
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
        self.shutting_down && self.entries.is_empty() && self.rollback_reap.is_empty()
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
            }
        }
        self.process_deadlines()?;
        self.reap_failed_creations()?;
        self.service_local_ipc()?;
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
                        && entry.lifecycle.accepts_input()
                    {
                        entry.pending_input.push_back(input);
                        self.service_writes(id)?;
                    }
                    handled += 1;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }
        if handled == CONTROL_DISPATCH_QUANTUM {
            self.reactor.waker().wake()?;
        }
        Ok(handled)
    }

    fn service_reads(&mut self, id: ExecutionId) -> Result<(), RuntimeError> {
        let mut consumed = 0usize;
        let mut drain_complete = false;
        while consumed < self.config.read_dispatch_bytes {
            let remaining = self.config.read_dispatch_bytes - consumed;
            let read_len = remaining.min(self.read_buffer.len());
            let outcome = {
                let entry = self
                    .entries
                    .get_mut(&id)
                    .ok_or(RuntimeError::UnknownExecution)?;
                entry
                    .execution
                    .read_output(&mut self.read_buffer[..read_len])?
            };
            match outcome {
                ReadOutcome::Bytes(0) | ReadOutcome::WouldBlock | ReadOutcome::Eof => {
                    drain_complete = matches!(
                        self.entries.get(&id).map(|entry| entry.lifecycle),
                        Some(Lifecycle::DrainingAfterPrimaryExit { .. })
                    );
                    break;
                }
                ReadOutcome::Bytes(count) => consumed += count,
            }
        }
        if drain_complete {
            self.finalize(id)?;
        }
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
            if !entry.lifecycle.accepts_input() {
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
        }
        Ok(())
    }

    fn enter_drain(&mut self, id: ExecutionId, exit: ChildExit) -> Result<(), RuntimeError> {
        let entry = self
            .entries
            .get_mut(&id)
            .ok_or(RuntimeError::UnknownExecution)?;
        entry.ingress_active.store(false, Ordering::Release);
        entry.pending_input.clear();
        self.reactor.set_writable(entry.token, false)?;
        entry.lifecycle = Lifecycle::DrainingAfterPrimaryExit {
            deadline: Instant::now() + self.config.final_drain,
            exit,
        };
        Ok(())
    }

    fn process_deadlines(&mut self) -> Result<(), RuntimeError> {
        let now = Instant::now();
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
                Some(Lifecycle::DrainingAfterPrimaryExit { exit, .. }) => {
                    let _ = exit;
                    self.finalize(id)?;
                }
                Some(Lifecycle::Running | Lifecycle::TerminationFailed) | None => {}
            }
        }
        Ok(())
    }

    fn finalize(&mut self, id: ExecutionId) -> Result<(), RuntimeError> {
        let Some(mut entry) = self.entries.remove(&id) else {
            return Ok(());
        };
        entry.ingress_active.store(false, Ordering::Release);
        entry.pending_input.clear();
        self.by_token.remove(&entry.token);
        self.reactor.deregister(entry.token)?;
        drop(entry);
        self.notify_local_ipc_execution_finalized(id);
        Ok(())
    }

    fn bound_wait_by_deadline(&self, requested: Option<Duration>) -> Option<Duration> {
        let now = Instant::now();
        let lifecycle = self
            .entries
            .values()
            .filter_map(|entry| entry.lifecycle.deadline())
            .min()
            .map(|deadline| deadline.saturating_duration_since(now));
        let rollback = (!self.rollback_reap.is_empty()).then_some(ROLLBACK_REAP_TICK);
        // See the `LOCAL_IPC_POLL_INTERVAL` doc comment: this bounds the
        // otherwise-indefinite PTY reactor wait only while a local client
        // is actually connected, so control-plane messages are serviced
        // promptly without waking an idle, client-less Runtime at all.
        let local_ipc = self
            .local_ipc
            .as_ref()
            .filter(|state| state.server.connection_count() > 0)
            .map(|_| LOCAL_IPC_POLL_INTERVAL);
        [requested, lifecycle, rollback, local_ipc]
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

/// SPEC-004 local attachment/projection wiring. Kept in its own `impl`
/// block for readability; every method here is additive to Pass 1-4 and
/// never gates PTY progress on client behavior.
impl Runtime {
    fn take_execution_damage(&mut self, id: ExecutionId) -> Option<seyal_terminal::Damage> {
        self.entries.get_mut(&id)?.execution.take_damage()
    }

    fn send_error(&mut self, token: u64, code: ErrorCode, offending: u16) {
        if let Some(state) = self.local_ipc.as_mut() {
            let message = framing::ErrorMessage {
                error_code: code as u16,
                offending_message_type: offending,
                detail_code: 0,
            };
            let frame = framing::encode_frame(MessageType::Error, &message.encode());
            let _ = state.server.enqueue(token, frame, None);
        }
    }

    fn release_connection_attachment(&mut self, token: u64) {
        let Some(state) = self.local_ipc.as_mut() else {
            return;
        };
        if let Some(meta) = state.connections.remove(&token)
            && let Some(attachment_id) = meta.attachment
        {
            let _ = state.attachments.detach(attachment_id);
            state.projections.remove(&attachment_id);
        }
    }

    /// Notifies every live attachment of `execution_id` that it finalized
    /// and reclaims their projection resources. Client disconnect never
    /// affects the execution; this is the reverse direction only.
    fn notify_local_ipc_execution_finalized(&mut self, execution_id: ExecutionId) {
        let Some(state) = self.local_ipc.as_mut() else {
            return;
        };
        let attachment_ids = state.attachments.remove_all_for_execution(execution_id);
        for attachment_id in attachment_ids {
            if let Some(projection) = state.projections.remove(&attachment_id) {
                let message = framing::LifecycleMessage {
                    execution_id,
                    lifecycle: framing::Lifecycle::Finalized,
                };
                let frame = framing::encode_frame(MessageType::Lifecycle, &message.encode());
                let _ = state.server.enqueue(projection.token, frame, None);
                if let Some(meta) = state.connections.get_mut(&projection.token) {
                    meta.attachment = None;
                }
            }
        }
    }

    fn service_local_ipc(&mut self) -> Result<(), RuntimeError> {
        if self.local_ipc.is_none() {
            return Ok(());
        }
        let events = {
            let state = self.local_ipc.as_mut().unwrap();
            match state.server.poll(Some(Duration::ZERO)) {
                Ok(events) => events,
                Err(_) => return Ok(()),
            }
        };
        for event in events {
            match event {
                ServerEvent::Connected { token } => {
                    if let Some(state) = self.local_ipc.as_mut() {
                        state
                            .connections
                            .insert(token, ConnectionMeta { attachment: None });
                    }
                }
                ServerEvent::PeerRejected => {}
                ServerEvent::FramingError { token } | ServerEvent::Disconnected { token } => {
                    self.release_connection_attachment(token);
                }
                ServerEvent::Frame {
                    token,
                    message_type,
                    payload,
                } => {
                    self.dispatch_local_ipc_frame(token, message_type, &payload);
                }
            }
        }
        self.publish_projection_updates();
        Ok(())
    }

    fn dispatch_local_ipc_frame(&mut self, token: u64, message_type: u16, payload: &[u8]) {
        let Some(current_state) = self
            .local_ipc
            .as_ref()
            .and_then(|state| state.server.state_of(token))
        else {
            return;
        };
        let Some(kind) = MessageType::from_u16(message_type) else {
            self.send_error(token, ErrorCode::UnknownMessage, message_type);
            return;
        };
        if current_state.validate_incoming(kind).is_err() {
            self.send_error(token, ErrorCode::InvalidState, message_type);
            return;
        }
        match kind {
            MessageType::ClientHello => self.handle_hello(token, payload),
            MessageType::ListExecutions => self.handle_list_executions(token),
            MessageType::Attach => self.handle_attach(token, payload),
            MessageType::Detach => self.handle_detach(token, payload),
            MessageType::Input => self.handle_input(token, payload),
            MessageType::Resize => self.handle_resize(token, payload),
            MessageType::Resync => self.handle_resync(token, payload),
            MessageType::Goodbye => {
                self.release_connection_attachment(token);
                if let Some(state) = self.local_ipc.as_mut() {
                    let mut dummy = Vec::new();
                    state.server.close(token, &mut dummy);
                }
            }
            _ => self.send_error(token, ErrorCode::InvalidState, message_type),
        }
    }

    fn handle_hello(&mut self, token: u64, payload: &[u8]) {
        if framing::ClientHello::decode(payload).is_err() {
            self.send_error(token, ErrorCode::MalformedPayload, MessageType::ClientHello as u16);
            return;
        }
        let Some(state) = self.local_ipc.as_mut() else {
            return;
        };
        let hello = framing::ServerHello {
            runtime_id: 0,
            server_capabilities: 0b11,
            max_frame_payload: framing::MAX_FRAME_PAYLOAD,
            max_input_payload: framing::MAX_INPUT_BYTES,
        };
        let frame = framing::encode_frame(MessageType::ServerHello, &hello.encode());
        let _ = state.server.enqueue(token, frame, None);
        state.server.set_state(token, LocalIpcConnState::Ready);
    }

    fn handle_list_executions(&mut self, token: u64) {
        let summaries = self.list();
        let Some(state) = self.local_ipc.as_mut() else {
            return;
        };
        let entries = summaries
            .into_iter()
            .take(framing::MAX_EXECUTION_LIST_ENTRIES as usize)
            .map(|summary| ExecutionListEntry {
                execution_id: summary.id,
                lifecycle: match summary.lifecycle {
                    ExecutionLifecycle::Running => WireLifecycle::Running,
                    _ => WireLifecycle::Terminating,
                },
                has_controller: state.attachments.has_controller(summary.id),
                attachment_count: state.attachments.attachments_for_execution(summary.id) as u16,
            })
            .collect();
        let list = ExecutionList { entries };
        let frame = framing::encode_frame(MessageType::ExecutionList, &list.encode());
        let _ = state.server.enqueue(token, frame, None);
    }

    fn handle_attach(&mut self, token: u64, payload: &[u8]) {
        let Ok(attach) = WireAttach::decode(payload) else {
            self.send_error(token, ErrorCode::MalformedPayload, MessageType::Attach as u16);
            return;
        };
        if self.lookup(attach.execution_id).is_none() {
            self.send_error(token, ErrorCode::InvalidExecution, MessageType::Attach as u16);
            return;
        }
        let Some(state) = self.local_ipc.as_ref() else {
            return;
        };
        if attach.requested_role == Role::Controller && state.attachments.has_controller(attach.execution_id)
        {
            self.send_error(token, ErrorCode::ControllerBusy, MessageType::Attach as u16);
            return;
        }
        if state.attachments.len() >= crate::local_ipc::attachment::MAX_LIVE_ATTACHMENTS {
            self.send_error(token, ErrorCode::CapacityExceeded, MessageType::Attach as u16);
            return;
        }

        let Some(entry) = self.entries.get(&attach.execution_id) else {
            self.send_error(token, ErrorCode::InvalidExecution, MessageType::Attach as u16);
            return;
        };
        let rows = entry.execution.terminal().rows();
        let cols = entry.execution.terminal().cols();
        let attachment_id = AttachmentId::new();
        // Drain any damage already pending for this execution so the
        // per-tick `publish_projection_updates` sweep below does not
        // immediately re-publish (and send a redundant `GenerationWake`)
        // for the exact same content this attach's initial snapshot
        // already captured.
        self.take_execution_damage(attach.execution_id);
        let Some(entry) = self.entries.get(&attach.execution_id) else {
            self.send_error(token, ErrorCode::InvalidExecution, MessageType::Attach as u16);
            return;
        };

        match build_projection(attachment_id, attach.execution_id, rows, cols, entry.execution.terminal())
        {
            Ok((mut projection_entry, committed_generation)) => {
                let Some(reader_fd) = projection_entry.region.take_reader_fd() else {
                    self.send_error(token, ErrorCode::InternalFailure, MessageType::Attach as u16);
                    return;
                };
                let region_bytes = projection_entry.region.region_bytes() as u64;
                let capacity_rows = projection_entry.capacity_rows;
                let capacity_cols = projection_entry.capacity_cols;
                let projection_id = projection_entry.projection_id;
                projection_entry.token = token;

                let Some(state) = self.local_ipc.as_mut() else {
                    return;
                };
                state.attachments.insert_prevalidated(
                    attachment_id,
                    attach.execution_id,
                    attach.requested_role,
                    projection_id,
                );
                state.projections.insert(attachment_id, projection_entry);
                state
                    .connections
                    .entry(token)
                    .or_insert(ConnectionMeta { attachment: None })
                    .attachment = Some(attachment_id);

                let attached = WireAttached {
                    execution_id: attach.execution_id,
                    attachment_id,
                    projection_id,
                    granted_role: attach.requested_role,
                    committed_generation,
                    region_bytes,
                    capacity_rows,
                    capacity_cols,
                };
                let frame = framing::encode_frame(MessageType::Attached, &attached.encode());
                let _ = state.server.enqueue(token, frame, Some(reader_fd));
                state.server.set_state(token, LocalIpcConnState::Attached);
            }
            Err(()) => {
                self.send_error(token, ErrorCode::InternalFailure, MessageType::Attach as u16);
            }
        }
    }

    fn handle_detach(&mut self, token: u64, payload: &[u8]) {
        let Ok(detach) = framing::Detach::decode(payload) else {
            self.send_error(token, ErrorCode::MalformedPayload, MessageType::Detach as u16);
            return;
        };
        let Some(state) = self.local_ipc.as_mut() else {
            return;
        };
        if state.attachments.detach(detach.attachment_id).is_err() {
            self.send_error(token, ErrorCode::StaleIdentity, MessageType::Detach as u16);
            return;
        }
        state.projections.remove(&detach.attachment_id);
        if let Some(meta) = state.connections.get_mut(&token) {
            meta.attachment = None;
        }
        let response = framing::Detached {
            attachment_id: detach.attachment_id,
        };
        let frame = framing::encode_frame(MessageType::Detached, &response.encode());
        let _ = state.server.enqueue(token, frame, None);
        state.server.set_state(token, LocalIpcConnState::Ready);
    }

    fn handle_input(&mut self, token: u64, payload: &[u8]) {
        let Ok(input) = framing::InputRef::decode(payload) else {
            self.send_error(token, ErrorCode::MalformedPayload, MessageType::Input as u16);
            return;
        };
        let Some(state) = self.local_ipc.as_ref() else {
            return;
        };
        let execution_id = match state.attachments.authorize_mutation(input.attachment_id) {
            Ok(id) => id,
            Err(crate::local_ipc::attachment::AttachmentError::PermissionDenied) => {
                self.send_error(token, ErrorCode::PermissionDenied, MessageType::Input as u16);
                return;
            }
            Err(_) => {
                self.send_error(token, ErrorCode::StaleIdentity, MessageType::Input as u16);
                return;
            }
        };
        let bytes = input.bytes.to_vec();
        match self.input_ingress(execution_id) {
            Ok(ingress) => {
                if ingress.try_submit(bytes).is_err() {
                    self.send_error(token, ErrorCode::Backpressure, MessageType::Input as u16);
                }
            }
            Err(_) => {
                self.send_error(token, ErrorCode::InvalidExecution, MessageType::Input as u16);
            }
        }
    }

    fn handle_resize(&mut self, token: u64, payload: &[u8]) {
        let Ok(resize) = WireResize::decode(payload) else {
            self.send_error(token, ErrorCode::MalformedPayload, MessageType::Resize as u16);
            return;
        };
        let Some(state) = self.local_ipc.as_ref() else {
            return;
        };
        let execution_id = match state.attachments.authorize_mutation(resize.attachment_id) {
            Ok(id) => id,
            Err(crate::local_ipc::attachment::AttachmentError::PermissionDenied) => {
                self.send_error(token, ErrorCode::PermissionDenied, MessageType::Resize as u16);
                return;
            }
            Err(_) => {
                self.send_error(token, ErrorCode::StaleIdentity, MessageType::Resize as u16);
                return;
            }
        };
        if resize.rows == 0
            || resize.columns == 0
            || resize.rows > MAX_CAPACITY_ROWS
            || resize.columns > MAX_CAPACITY_COLS
        {
            self.send_error(token, ErrorCode::InvalidGeometry, MessageType::Resize as u16);
            return;
        }
        let Ok(size) = WindowSize::cells(resize.columns, resize.rows) else {
            self.send_error(token, ErrorCode::InvalidGeometry, MessageType::Resize as u16);
            return;
        };
        if self.resize(execution_id, size).is_err() {
            self.send_error(token, ErrorCode::InvalidExecution, MessageType::Resize as u16);
        }
    }

    fn handle_resync(&mut self, token: u64, payload: &[u8]) {
        let Ok(resync) = framing::Resync::decode(payload) else {
            self.send_error(token, ErrorCode::MalformedPayload, MessageType::Resync as u16);
            return;
        };
        let Some(state) = self.local_ipc.as_ref() else {
            return;
        };
        let Ok(execution_id) = state.attachments.execution_of(resync.attachment_id) else {
            self.send_error(token, ErrorCode::StaleIdentity, MessageType::Resync as u16);
            return;
        };
        // Draining any pending damage here (rather than only reading its
        // generation) prevents the per-tick `publish_projection_updates`
        // sweep from immediately re-publishing the same content this
        // explicit resync already captures.
        self.take_execution_damage(execution_id);
        let Some(entry) = self.entries.get(&execution_id) else {
            self.send_error(token, ErrorCode::InvalidExecution, MessageType::Resync as u16);
            return;
        };
        let damage_generation = entry.execution.terminal().damage_generation();
        let snapshot = producer::full_snapshot(entry.execution.terminal(), damage_generation);

        let Some(state) = self.local_ipc.as_mut() else {
            return;
        };
        let Some(projection) = state.projections.get_mut(&resync.attachment_id) else {
            self.send_error(token, ErrorCode::ProjectionUnavailable, MessageType::Resync as u16);
            return;
        };
        if let Ok(generation) = projection.writer.publish(&snapshot.as_snapshot_write()) {
            let wake = WireGenerationWake {
                attachment_id: resync.attachment_id,
                projection_id: projection.projection_id,
                committed_generation: generation,
            };
            let frame = framing::encode_frame(MessageType::GenerationWake, &wake.encode());
            let _ = state.server.enqueue(token, frame, None);
        }
    }

    /// Once per tick: for every execution with at least one live
    /// projection, consumes at most one damage generation and republishes
    /// every attached projection from the current canonical visible state.
    /// A capacity-exceeded publish (canonical resize grew beyond the
    /// current region) triggers `ProjectionReplaced` instead.
    fn publish_projection_updates(&mut self) {
        let Some(state) = self.local_ipc.as_ref() else {
            return;
        };
        let execution_ids: Vec<ExecutionId> = state
            .projections
            .values()
            .map(|projection| projection.execution_id)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        for execution_id in execution_ids {
            let Some(damage) = self.take_execution_damage(execution_id) else {
                continue;
            };
            let Some(entry) = self.entries.get(&execution_id) else {
                continue;
            };
            let snapshot = producer::full_snapshot(entry.execution.terminal(), damage.generation);
            let rows = snapshot.rows;
            let cols = snapshot.columns;

            let Some(state) = self.local_ipc.as_mut() else {
                return;
            };
            let attachment_ids: Vec<AttachmentId> = state
                .projections
                .iter()
                .filter(|(_, projection)| projection.execution_id == execution_id)
                .map(|(id, _)| *id)
                .collect();

            for attachment_id in attachment_ids {
                let needs_replacement = state
                    .projections
                    .get(&attachment_id)
                    .is_some_and(|projection| {
                        rows > projection.capacity_rows || cols > projection.capacity_cols
                    });
                if needs_replacement {
                    replace_projection(state, attachment_id, execution_id, rows, cols, &snapshot);
                    continue;
                }
                let Some(projection) = state.projections.get_mut(&attachment_id) else {
                    continue;
                };
                if let Ok(generation) = projection.writer.publish(&snapshot.as_snapshot_write()) {
                    let wake = WireGenerationWake {
                        attachment_id,
                        projection_id: projection.projection_id,
                        committed_generation: generation,
                    };
                    let frame = framing::encode_frame(MessageType::GenerationWake, &wake.encode());
                    let _ = state.server.enqueue(projection.token, frame, None);
                }
            }
        }
    }
}

fn align_up(value: usize, alignment: usize) -> usize {
    value.div_ceil(alignment) * alignment
}

fn slot_stride_for(capacity_rows: u16, capacity_cols: u16) -> u64 {
    use crate::projection::layout::{CELL_LEN, DAMAGE_LEN, SLOT_HEADER_LEN};
    let cell_bytes = capacity_rows as usize * capacity_cols as usize * CELL_LEN;
    let damage_bytes = capacity_rows as usize * DAMAGE_LEN;
    align_up(SLOT_HEADER_LEN + cell_bytes + damage_bytes, 64) as u64
}

fn build_projection(
    attachment_id: AttachmentId,
    execution_id: ExecutionId,
    rows: u16,
    cols: u16,
    terminal: &seyal_terminal::TerminalState,
) -> Result<(ProjectionEntry, u64), ()> {
    let capacity_rows = rows.clamp(1, MAX_CAPACITY_ROWS);
    let capacity_cols = cols.clamp(1, MAX_CAPACITY_COLS);
    let slot_stride = slot_stride_for(capacity_rows, capacity_cols);
    let region_bytes = REGION_HEADER_LEN as u64 + 2 * slot_stride;
    let projection_id = ProjectionId::new();
    let region_header = RegionHeader {
        region_bytes,
        execution_id: u128::from_le_bytes(execution_id.to_bytes()),
        attachment_id: u128::from_le_bytes(attachment_id.to_bytes()),
        projection_id: u128::from_le_bytes(projection_id.to_bytes()),
        slot_stride,
        slot0_offset: REGION_HEADER_LEN as u64,
        capacity_rows,
        capacity_cols,
    };
    let region = ProjectionRegion::create(&region_header).map_err(|_| ())?;
    let memory = region.writer_memory();
    let mut writer = Writer::new(memory, region_header).map_err(|_| ())?;
    let damage_generation = terminal.damage_generation();
    let snapshot = producer::full_snapshot(terminal, damage_generation);
    let generation = writer.publish(&snapshot.as_snapshot_write()).map_err(|_| ())?;
    Ok((
        ProjectionEntry {
            execution_id,
            token: 0,
            region,
            writer,
            projection_id,
            capacity_rows,
            capacity_cols,
        },
        generation,
    ))
}

fn replace_projection(
    state: &mut LocalIpcState,
    attachment_id: AttachmentId,
    execution_id: ExecutionId,
    rows: u16,
    cols: u16,
    snapshot: &producer::OwnedSnapshot,
) {
    let Some(existing) = state.projections.get(&attachment_id) else {
        return;
    };
    let token = existing.token;
    let capacity_rows = rows.clamp(1, MAX_CAPACITY_ROWS);
    let capacity_cols = cols.clamp(1, MAX_CAPACITY_COLS);
    let slot_stride = slot_stride_for(capacity_rows, capacity_cols);
    let region_bytes = REGION_HEADER_LEN as u64 + 2 * slot_stride;
    let projection_id = ProjectionId::new();
    let region_header = RegionHeader {
        region_bytes,
        execution_id: u128::from_le_bytes(execution_id.to_bytes()),
        attachment_id: u128::from_le_bytes(attachment_id.to_bytes()),
        projection_id: u128::from_le_bytes(projection_id.to_bytes()),
        slot_stride,
        slot0_offset: REGION_HEADER_LEN as u64,
        capacity_rows,
        capacity_cols,
    };
    let Ok(region) = ProjectionRegion::create(&region_header) else {
        let _ = state
            .attachments
            .replace_projection(attachment_id, projection_id);
        return;
    };
    let memory = region.writer_memory();
    let Ok(mut writer) = Writer::new(memory, region_header) else {
        return;
    };
    let Ok(committed_generation) = writer.publish(&snapshot.as_snapshot_write()) else {
        return;
    };
    let mut region = region;
    let Some(reader_fd) = region.take_reader_fd() else {
        return;
    };

    let _ = state
        .attachments
        .replace_projection(attachment_id, projection_id);
    state.projections.insert(
        attachment_id,
        ProjectionEntry {
            execution_id,
            token,
            region,
            writer,
            projection_id,
            capacity_rows,
            capacity_cols,
        },
    );

    let message = WireProjectionReplaced {
        execution_id,
        attachment_id,
        projection_id,
        committed_generation,
        region_bytes,
        capacity_rows,
        capacity_cols,
    };
    let frame = framing::encode_frame(MessageType::ProjectionReplaced, &message.encode());
    let _ = state.server.enqueue(token, frame, Some(reader_fd));
}
