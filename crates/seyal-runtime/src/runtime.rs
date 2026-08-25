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
    AttachmentId, CapabilityPolicy, ExecutionId, InputIngress, RuntimeError, RuntimeId,
    WorkspaceId,
    input::{AcceptedInput, ControlMessage},
    singleton::SingletonGuard,
};

#[cfg(target_os = "macos")]
use crate::{
    ProjectionId,
    local_ipc::{
        attachment::{AttachmentError, AttachmentRegistry, MAX_LIVE_ATTACHMENTS},
        connection::{ConnectionState as LocalIpcConnState, LocalIpcServer, ServerEvent},
        discovery,
        framing::{
            self, Attach as WireAttach, Attached as WireAttached, ErrorCode, ExecutionList,
            ExecutionListEntry, GenerationWake as WireGenerationWake, Lifecycle as WireLifecycle,
            MessageType, ProjectionReplaced as WireProjectionReplaced, Resize as WireResize, Role,
        },
    },
    projection::{
        layout::{
            MAX_CAPACITY_COLS, MAX_CAPACITY_ROWS, MAX_REGION_BYTES, REGION_HEADER_LEN, RegionHeader,
        },
        lifecycle::ProjectionRegion,
        producer,
        writer::Writer,
    },
};

const EVENT_CAPACITY: usize = 128;
const READ_BUFFER_SIZE: usize = 16 * 1024;
const CONTROL_DISPATCH_QUANTUM: usize = 64;
const ROLLBACK_REAP_TICK: Duration = Duration::from_millis(10);

#[cfg(target_os = "macos")]
const MAX_AGGREGATE_PROJECTION_BYTES: usize = 128 * 1024 * 1024;

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

#[cfg(target_os = "macos")]
struct ConnectionMeta {
    attachment: Option<AttachmentId>,
    reactor_token: RegistrationToken,
}

#[cfg(target_os = "macos")]
struct ProjectionEntry {
    execution_id: ExecutionId,
    connection_token: u64,
    writer: Writer,
    region: ProjectionRegion,
    projection_id: ProjectionId,
    capacity_rows: u16,
    capacity_cols: u16,
}

#[cfg(target_os = "macos")]
struct LocalIpcState {
    server: LocalIpcServer,
    socket_path: PathBuf,
    listener_reactor_token: RegistrationToken,
    attachments: AttachmentRegistry,
    connections: HashMap<u64, ConnectionMeta>,
    reactor_connections: HashMap<RegistrationToken, u64>,
    projections: HashMap<AttachmentId, ProjectionEntry>,
    aggregate_projection_bytes: usize,
}

#[cfg(target_os = "macos")]
impl LocalIpcState {
    fn bind(
        reactor: &mut ExecutionReactor,
        runtime_dir_override: Option<PathBuf>,
    ) -> Result<Self, RuntimeError> {
        let runtime_dir = match runtime_dir_override {
            Some(dir) => dir,
            None => discovery::darwin_user_runtime_dir().map_err(|_| {
                RuntimeError::Io(std::io::Error::other("local IPC discovery failed"))
            })?,
        };
        discovery::ensure_verified_runtime_dir(&runtime_dir).map_err(|_| {
            RuntimeError::Io(std::io::Error::other(
                "local IPC directory verification failed",
            ))
        })?;
        let socket_path = discovery::control_socket_path(&runtime_dir).map_err(|_| {
            RuntimeError::Io(std::io::Error::other("local IPC socket path invalid"))
        })?;
        discovery::remove_verified_stale_socket(&socket_path).map_err(|_| {
            RuntimeError::Io(std::io::Error::other(
                "local IPC stale socket validation failed",
            ))
        })?;
        let server =
            LocalIpcServer::bind(&socket_path, crate::local_ipc::connection::MAX_CONNECTIONS)?;
        let listener_reactor_token = match reactor.register_auxiliary(server.listener_fd()) {
            Ok(token) => token,
            Err(error) => {
                drop(server);
                let _ = std::fs::remove_file(&socket_path);
                return Err(error.into());
            }
        };
        Ok(Self {
            server,
            socket_path,
            listener_reactor_token,
            attachments: AttachmentRegistry::new(),
            connections: HashMap::new(),
            reactor_connections: HashMap::new(),
            projections: HashMap::new(),
            aggregate_projection_bytes: 0,
        })
    }
}

#[cfg(target_os = "macos")]
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
    #[cfg(target_os = "macos")]
    local_ipc: Option<LocalIpcState>,
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
            } => Some(LocalIpcState::bind(
                &mut reactor,
                runtime_dir_override.clone(),
            )?),
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
            #[cfg(target_os = "macos")]
            local_ipc,
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
        self.publish_projection_updates();
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
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
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
        // Final PTY bytes may have mutated canonical TerminalState during the
        // same drain turn that observes EOF. Publish that pending canonical
        // damage while TerminalExecution still exists; only then may teardown
        // remove the execution and its projection resources.
        #[cfg(target_os = "macos")]
        self.publish_projection_updates();

        let Some(mut entry) = self.entries.remove(&id) else {
            return Ok(());
        };
        entry.ingress_active.store(false, Ordering::Release);
        entry.pending_input.clear();
        self.by_token.remove(&entry.token);
        self.reactor.deregister(entry.token)?;
        drop(entry);
        #[cfg(target_os = "macos")]
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
        [requested, lifecycle, rollback].into_iter().flatten().min()
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

#[cfg(target_os = "macos")]
impl Runtime {
    fn service_local_reactor_event(
        &mut self,
        reactor_token: RegistrationToken,
        kind: ReactorEventKind,
        hangup: bool,
    ) -> Result<(), RuntimeError> {
        let listener = self
            .local_ipc
            .as_ref()
            .is_some_and(|state| state.listener_reactor_token == reactor_token);
        if listener {
            self.accept_local_connections()?;
            return Ok(());
        }

        let connection_token = self
            .local_ipc
            .as_ref()
            .and_then(|state| state.reactor_connections.get(&reactor_token).copied());
        let Some(connection_token) = connection_token else {
            return Ok(());
        };

        let events = {
            let Some(state) = self.local_ipc.as_mut() else {
                return Ok(());
            };
            match kind {
                ReactorEventKind::AuxiliaryReadable => {
                    state.server.service_read(connection_token, hangup)
                }
                ReactorEventKind::AuxiliaryWritable => state.server.service_write(connection_token),
                _ => Vec::new(),
            }
        };
        self.handle_local_server_events(events);
        if self.local_connection_exists(connection_token) {
            self.sync_local_writable(connection_token);
        }
        Ok(())
    }

    fn accept_local_connections(&mut self) -> Result<(), RuntimeError> {
        let events = {
            let Some(state) = self.local_ipc.as_mut() else {
                return Ok(());
            };
            state.server.accept_ready()?
        };
        for event in events {
            match event {
                ServerEvent::Connected { token } => {
                    let fd = self
                        .local_ipc
                        .as_ref()
                        .and_then(|state| state.server.connection_fd(token));
                    let Some(fd) = fd else {
                        continue;
                    };
                    match self.reactor.register_auxiliary(fd) {
                        Ok(reactor_token) => {
                            if let Some(state) = self.local_ipc.as_mut() {
                                state.connections.insert(
                                    token,
                                    ConnectionMeta {
                                        attachment: None,
                                        reactor_token,
                                    },
                                );
                                state.reactor_connections.insert(reactor_token, token);
                            }
                        }
                        Err(_) => {
                            if let Some(state) = self.local_ipc.as_mut() {
                                state.server.close(token);
                            }
                        }
                    }
                }
                ServerEvent::PeerRejected => {}
                other => self.handle_local_server_events(vec![other]),
            }
        }
        Ok(())
    }

    fn handle_local_server_events(&mut self, events: Vec<ServerEvent>) {
        for event in events {
            match event {
                ServerEvent::Connected { .. } | ServerEvent::PeerRejected => {}
                ServerEvent::FramingError { token } | ServerEvent::Disconnected { token } => {
                    self.close_local_connection(token);
                }
                ServerEvent::Frame {
                    token,
                    message_type,
                    payload,
                } => self.dispatch_local_ipc_frame(token, message_type, &payload),
            }
        }
    }

    fn local_connection_exists(&self, token: u64) -> bool {
        self.local_ipc.as_ref().is_some_and(|state| {
            state.connections.contains_key(&token) && state.server.contains(token)
        })
    }

    fn sync_local_writable(&mut self, token: u64) -> bool {
        let values = self.local_ipc.as_ref().and_then(|state| {
            let meta = state.connections.get(&token)?;
            Some((meta.reactor_token, state.server.wants_write(token)))
        });
        let Some((reactor_token, wants_write)) = values else {
            return false;
        };
        if self
            .reactor
            .set_writable(reactor_token, wants_write)
            .is_err()
        {
            self.close_local_connection(token);
            return false;
        }
        true
    }

    fn close_local_connection(&mut self, token: u64) {
        let (reactor_token, attachment) = {
            let Some(state) = self.local_ipc.as_mut() else {
                return;
            };
            state.server.close(token);
            let Some(meta) = state.connections.remove(&token) else {
                return;
            };
            state.reactor_connections.remove(&meta.reactor_token);
            (meta.reactor_token, meta.attachment)
        };

        if let Some(attachment_id) = attachment {
            self.release_local_attachment(attachment_id);
        }
        let _ = self.reactor.deregister(reactor_token);
    }

    fn release_local_attachment(&mut self, attachment_id: AttachmentId) {
        let execution_id = self
            .local_ipc
            .as_ref()
            .and_then(|state| state.attachments.execution_of(attachment_id).ok());
        if let Some(state) = self.local_ipc.as_mut() {
            let _ = state.attachments.detach(attachment_id);
            if let Some(projection) = state.projections.remove(&attachment_id) {
                state.aggregate_projection_bytes = state
                    .aggregate_projection_bytes
                    .saturating_sub(projection.region.region_bytes());
            }
        }
        if let Some(execution_id) = execution_id
            && let Some(entry) = self.entries.get_mut(&execution_id)
        {
            let removed = entry.attachments.remove(&attachment_id);
            debug_assert!(removed);
        }
    }

    fn send_mandatory_frame(
        &mut self,
        token: u64,
        bytes: Vec<u8>,
        fd: Option<std::os::fd::OwnedFd>,
    ) -> bool {
        let sent = self
            .local_ipc
            .as_mut()
            .is_some_and(|state| state.server.enqueue_mandatory(token, bytes, fd).is_ok());
        if !sent {
            self.close_local_connection(token);
            return false;
        }
        self.sync_local_writable(token)
    }

    fn send_wake_frame(&mut self, token: u64, bytes: Vec<u8>) -> bool {
        let sent = self
            .local_ipc
            .as_mut()
            .is_some_and(|state| state.server.enqueue_wake(token, bytes).is_ok());
        if !sent {
            self.close_local_connection(token);
            return false;
        }
        self.sync_local_writable(token)
    }

    fn send_error(&mut self, token: u64, code: ErrorCode, offending: u16) {
        let message = framing::ErrorMessage {
            error_code: code as u16,
            offending_message_type: offending,
            detail_code: 0,
        };
        let frame = framing::encode_frame(MessageType::Error, &message.encode());
        let _ = self.send_mandatory_frame(token, frame, None);
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
            MessageType::ListExecutions => self.handle_list_executions(token, payload),
            MessageType::Attach => self.handle_attach(token, payload),
            MessageType::Detach => self.handle_detach(token, payload),
            MessageType::Input => self.handle_input(token, payload),
            MessageType::Resize => self.handle_resize(token, payload),
            MessageType::Resync => self.handle_resync(token, payload),
            MessageType::Goodbye => {
                if !payload.is_empty() {
                    self.send_error(token, ErrorCode::MalformedPayload, message_type);
                    return;
                }
                self.close_local_connection(token);
            }
            _ => self.send_error(token, ErrorCode::InvalidState, message_type),
        }
    }

    fn handle_hello(&mut self, token: u64, payload: &[u8]) {
        let Ok(hello) = framing::ClientHello::decode(payload) else {
            self.send_error(
                token,
                ErrorCode::MalformedPayload,
                MessageType::ClientHello as u16,
            );
            return;
        };
        if hello.client_capabilities != 0 {
            self.send_error(
                token,
                ErrorCode::MalformedPayload,
                MessageType::ClientHello as u16,
            );
            return;
        }
        let response = framing::ServerHello {
            runtime_id: u128::from_le_bytes(self.id.to_bytes()),
            server_capabilities: 0b11,
            max_frame_payload: framing::MAX_FRAME_PAYLOAD,
            max_input_payload: framing::MAX_INPUT_BYTES,
        };
        let frame = framing::encode_frame(MessageType::ServerHello, &response.encode());
        if self.send_mandatory_frame(token, frame, None)
            && let Some(state) = self.local_ipc.as_mut()
        {
            state.server.set_state(token, LocalIpcConnState::Ready);
        }
    }

    fn handle_list_executions(&mut self, token: u64, payload: &[u8]) {
        if !payload.is_empty() {
            self.send_error(
                token,
                ErrorCode::MalformedPayload,
                MessageType::ListExecutions as u16,
            );
            return;
        }
        let summaries = self.list();
        let Some(state) = self.local_ipc.as_ref() else {
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
                attachment_count: summary.attachment_count.min(u16::MAX as usize) as u16,
            })
            .collect();
        let frame = framing::encode_frame(
            MessageType::ExecutionList,
            &ExecutionList { entries }.encode(),
        );
        let _ = self.send_mandatory_frame(token, frame, None);
    }

    fn handle_attach(&mut self, token: u64, payload: &[u8]) {
        let Ok(attach) = WireAttach::decode(payload) else {
            self.send_error(
                token,
                ErrorCode::MalformedPayload,
                MessageType::Attach as u16,
            );
            return;
        };
        let Some(entry) = self.entries.get(&attach.execution_id) else {
            self.send_error(
                token,
                ErrorCode::InvalidExecution,
                MessageType::Attach as u16,
            );
            return;
        };
        let Some(state) = self.local_ipc.as_ref() else {
            return;
        };
        if state.attachments.len() >= MAX_LIVE_ATTACHMENTS {
            self.send_error(
                token,
                ErrorCode::CapacityExceeded,
                MessageType::Attach as u16,
            );
            return;
        }
        if attach.requested_role == Role::Controller
            && state.attachments.has_controller(attach.execution_id)
        {
            self.send_error(token, ErrorCode::ControllerBusy, MessageType::Attach as u16);
            return;
        }
        if state
            .connections
            .get(&token)
            .and_then(|meta| meta.attachment)
            .is_some()
        {
            self.send_error(token, ErrorCode::InvalidState, MessageType::Attach as u16);
            return;
        }

        let snapshot = producer::from_execution(entry.execution.projection_snapshot());
        let attachment_id = AttachmentId::new();
        let Ok((mut projection, committed_generation)) =
            build_projection(attachment_id, attach.execution_id, token, &snapshot)
        else {
            self.send_error(
                token,
                ErrorCode::ProjectionUnavailable,
                MessageType::Attach as u16,
            );
            return;
        };
        let new_bytes = projection.region.region_bytes();
        if !self.projection_budget_allows(0, new_bytes) {
            self.send_error(
                token,
                ErrorCode::CapacityExceeded,
                MessageType::Attach as u16,
            );
            return;
        }
        let Some(reader_fd) = projection.region.take_reader_fd() else {
            self.send_error(
                token,
                ErrorCode::InternalFailure,
                MessageType::Attach as u16,
            );
            return;
        };
        let attached = WireAttached {
            execution_id: attach.execution_id,
            attachment_id,
            projection_id: projection.projection_id,
            granted_role: attach.requested_role,
            committed_generation,
            region_bytes: new_bytes as u64,
            capacity_rows: projection.capacity_rows,
            capacity_cols: projection.capacity_cols,
        };
        let frame = framing::encode_frame(MessageType::Attached, &attached.encode());
        if !self.send_mandatory_frame(token, frame, Some(reader_fd)) {
            return;
        }

        {
            let Some(state) = self.local_ipc.as_mut() else {
                return;
            };
            if !state.connections.contains_key(&token) {
                return;
            }
            state.attachments.insert_prevalidated(
                attachment_id,
                attach.execution_id,
                attach.requested_role,
                projection.projection_id,
                token,
            );
            state.aggregate_projection_bytes += new_bytes;
            state.projections.insert(attachment_id, projection);
            if let Some(meta) = state.connections.get_mut(&token) {
                meta.attachment = Some(attachment_id);
            }
            state.server.set_state(token, LocalIpcConnState::Attached);
        }

        let Some(entry) = self.entries.get_mut(&attach.execution_id) else {
            self.release_local_attachment(attachment_id);
            self.close_local_connection(token);
            return;
        };
        let inserted = entry.attachments.insert(attachment_id);
        debug_assert!(inserted);
    }

    fn handle_detach(&mut self, token: u64, payload: &[u8]) {
        let Ok(detach) = framing::Detach::decode(payload) else {
            self.send_error(
                token,
                ErrorCode::MalformedPayload,
                MessageType::Detach as u16,
            );
            return;
        };
        let execution_id = match self.local_ipc.as_ref().map(|state| {
            state
                .attachments
                .execution_for_connection(token, detach.attachment_id)
        }) {
            Some(Ok(id)) => id,
            Some(Err(AttachmentError::WrongConnection)) => {
                self.send_error(token, ErrorCode::StaleIdentity, MessageType::Detach as u16);
                return;
            }
            _ => {
                self.send_error(token, ErrorCode::StaleIdentity, MessageType::Detach as u16);
                return;
            }
        };
        let result = self.local_ipc.as_mut().map(|state| {
            state
                .attachments
                .detach_for_connection(token, detach.attachment_id)
        });
        match result {
            Some(Ok(())) => {}
            Some(Err(AttachmentError::WrongConnection)) => {
                self.send_error(token, ErrorCode::StaleIdentity, MessageType::Detach as u16);
                return;
            }
            _ => {
                self.send_error(token, ErrorCode::StaleIdentity, MessageType::Detach as u16);
                return;
            }
        }
        if let Some(state) = self.local_ipc.as_mut() {
            if let Some(projection) = state.projections.remove(&detach.attachment_id) {
                state.aggregate_projection_bytes = state
                    .aggregate_projection_bytes
                    .saturating_sub(projection.region.region_bytes());
            }
            if let Some(meta) = state.connections.get_mut(&token) {
                meta.attachment = None;
            }
            state.server.set_state(token, LocalIpcConnState::Ready);
        }
        if let Some(entry) = self.entries.get_mut(&execution_id) {
            let removed = entry.attachments.remove(&detach.attachment_id);
            debug_assert!(removed);
        }
        let response = framing::Detached {
            attachment_id: detach.attachment_id,
        };
        let frame = framing::encode_frame(MessageType::Detached, &response.encode());
        let _ = self.send_mandatory_frame(token, frame, None);
    }

    fn handle_input(&mut self, token: u64, payload: &[u8]) {
        let Ok(input) = framing::InputRef::decode(payload) else {
            self.send_error(
                token,
                ErrorCode::MalformedPayload,
                MessageType::Input as u16,
            );
            return;
        };
        let authorization = self.local_ipc.as_ref().map(|state| {
            state
                .attachments
                .authorize_mutation(token, input.attachment_id)
        });
        let execution_id = match authorization {
            Some(Ok(id)) => id,
            Some(Err(AttachmentError::PermissionDenied)) => {
                self.send_error(
                    token,
                    ErrorCode::PermissionDenied,
                    MessageType::Input as u16,
                );
                return;
            }
            _ => {
                self.send_error(token, ErrorCode::StaleIdentity, MessageType::Input as u16);
                return;
            }
        };
        match self.input_ingress(execution_id) {
            Ok(ingress) => {
                if ingress.try_submit(input.bytes.to_vec()).is_err() {
                    self.send_error(token, ErrorCode::Backpressure, MessageType::Input as u16);
                }
            }
            Err(_) => self.send_error(
                token,
                ErrorCode::InvalidExecution,
                MessageType::Input as u16,
            ),
        }
    }

    fn handle_resize(&mut self, token: u64, payload: &[u8]) {
        let Ok(resize) = WireResize::decode(payload) else {
            self.send_error(
                token,
                ErrorCode::MalformedPayload,
                MessageType::Resize as u16,
            );
            return;
        };
        let authorization = self.local_ipc.as_ref().map(|state| {
            state
                .attachments
                .authorize_mutation(token, resize.attachment_id)
        });
        let execution_id = match authorization {
            Some(Ok(id)) => id,
            Some(Err(AttachmentError::PermissionDenied)) => {
                self.send_error(
                    token,
                    ErrorCode::PermissionDenied,
                    MessageType::Resize as u16,
                );
                return;
            }
            _ => {
                self.send_error(token, ErrorCode::StaleIdentity, MessageType::Resize as u16);
                return;
            }
        };
        if resize.rows == 0
            || resize.columns == 0
            || resize.rows > MAX_CAPACITY_ROWS
            || resize.columns > MAX_CAPACITY_COLS
        {
            self.send_error(
                token,
                ErrorCode::InvalidGeometry,
                MessageType::Resize as u16,
            );
            return;
        }
        let Ok(size) = WindowSize::cells(resize.columns, resize.rows) else {
            self.send_error(
                token,
                ErrorCode::InvalidGeometry,
                MessageType::Resize as u16,
            );
            return;
        };
        if self.resize(execution_id, size).is_err() {
            self.send_error(
                token,
                ErrorCode::InvalidExecution,
                MessageType::Resize as u16,
            );
        }
    }

    fn handle_resync(&mut self, token: u64, payload: &[u8]) {
        let Ok(resync) = framing::Resync::decode(payload) else {
            self.send_error(
                token,
                ErrorCode::MalformedPayload,
                MessageType::Resync as u16,
            );
            return;
        };
        let execution = self.local_ipc.as_ref().map(|state| {
            state
                .attachments
                .execution_for_connection(token, resync.attachment_id)
        });
        let execution_id = match execution {
            Some(Ok(id)) => id,
            _ => {
                self.send_error(token, ErrorCode::StaleIdentity, MessageType::Resync as u16);
                return;
            }
        };
        let Some(entry) = self.entries.get(&execution_id) else {
            self.send_error(
                token,
                ErrorCode::InvalidExecution,
                MessageType::Resync as u16,
            );
            return;
        };
        let snapshot = producer::from_execution(entry.execution.projection_snapshot());

        let can_publish = self.local_ipc.as_ref().is_some_and(|state| {
            state
                .projections
                .get(&resync.attachment_id)
                .is_some_and(|projection| {
                    snapshot.rows <= projection.capacity_rows
                        && snapshot.columns <= projection.capacity_cols
                })
        });
        if can_publish {
            let published = {
                let state = self.local_ipc.as_mut().unwrap();
                let projection = state.projections.get_mut(&resync.attachment_id).unwrap();
                projection
                    .writer
                    .publish(&snapshot.as_snapshot_write())
                    .ok()
                    .map(|generation| (generation, projection.projection_id))
            };
            if let Some((generation, projection_id)) = published {
                let wake = WireGenerationWake {
                    attachment_id: resync.attachment_id,
                    projection_id,
                    committed_generation: generation,
                };
                let frame = framing::encode_frame(MessageType::GenerationWake, &wake.encode());
                let _ = self.send_wake_frame(token, frame);
                return;
            }
        }

        if !self.install_projection_replacement(
            resync.attachment_id,
            execution_id,
            token,
            &snapshot,
        ) && self.local_connection_exists(token)
        {
            self.mark_projection_unavailable(resync.attachment_id);
            self.send_error(
                token,
                ErrorCode::ProjectionUnavailable,
                MessageType::Resync as u16,
            );
        }
    }

    fn notify_local_ipc_execution_finalized(&mut self, execution_id: ExecutionId) {
        let notifications = {
            let Some(state) = self.local_ipc.as_mut() else {
                return;
            };
            let attachment_connections = state
                .attachments
                .attachments_with_connections_for_execution(execution_id);
            let _ = state.attachments.remove_all_for_execution(execution_id);
            let mut notifications = Vec::with_capacity(attachment_connections.len());
            for (attachment_id, connection_token) in attachment_connections {
                if let Some(projection) = state.projections.remove(&attachment_id) {
                    state.aggregate_projection_bytes = state
                        .aggregate_projection_bytes
                        .saturating_sub(projection.region.region_bytes());
                }
                notifications.push(connection_token);
                if let Some(meta) = state.connections.get_mut(&connection_token) {
                    meta.attachment = None;
                }
            }
            notifications
        };
        for token in notifications {
            let message = framing::LifecycleMessage {
                execution_id,
                lifecycle: framing::Lifecycle::Finalized,
            };
            let frame = framing::encode_frame(MessageType::Lifecycle, &message.encode());
            if self.send_mandatory_frame(token, frame, None)
                && let Some(state) = self.local_ipc.as_mut()
            {
                state.server.set_state(token, LocalIpcConnState::Ready);
            }
        }
    }

    fn publish_projection_updates(&mut self) {
        let mut execution_ids = [None; MAX_LIVE_ATTACHMENTS];
        let mut execution_count = 0usize;
        if let Some(state) = self.local_ipc.as_ref() {
            for projection in state.projections.values() {
                if !execution_ids[..execution_count].contains(&Some(projection.execution_id))
                    && execution_count < execution_ids.len()
                {
                    execution_ids[execution_count] = Some(projection.execution_id);
                    execution_count += 1;
                }
            }
        }

        for execution_id in execution_ids[..execution_count].iter().flatten().copied() {
            let update = self
                .entries
                .get_mut(&execution_id)
                .and_then(|entry| entry.execution.take_projection_update());
            let Some(update) = update else {
                continue;
            };
            let snapshot = producer::from_execution(update);
            let mut attachment_ids = [None; MAX_LIVE_ATTACHMENTS];
            let mut attachment_count = 0usize;
            if let Some(state) = self.local_ipc.as_ref() {
                for (&attachment_id, projection) in &state.projections {
                    if projection.execution_id == execution_id
                        && attachment_count < attachment_ids.len()
                    {
                        attachment_ids[attachment_count] = Some(attachment_id);
                        attachment_count += 1;
                    }
                }
            }

            for attachment_id in attachment_ids[..attachment_count].iter().flatten().copied() {
                let projection_meta = self.local_ipc.as_ref().and_then(|state| {
                    state.projections.get(&attachment_id).map(|projection| {
                        (
                            projection.connection_token,
                            projection.capacity_rows,
                            projection.capacity_cols,
                            projection.projection_id,
                        )
                    })
                });
                let Some((token, capacity_rows, capacity_cols, projection_id)) = projection_meta
                else {
                    continue;
                };
                if snapshot.rows > capacity_rows || snapshot.columns > capacity_cols {
                    if !self.install_projection_replacement(
                        attachment_id,
                        execution_id,
                        token,
                        &snapshot,
                    ) {
                        self.mark_projection_unavailable(attachment_id);
                    }
                    continue;
                }
                let generation = self.local_ipc.as_mut().and_then(|state| {
                    state
                        .projections
                        .get_mut(&attachment_id)
                        .and_then(|projection| {
                            projection
                                .writer
                                .publish(&snapshot.as_snapshot_write())
                                .ok()
                        })
                });
                if let Some(generation) = generation {
                    let wake = WireGenerationWake {
                        attachment_id,
                        projection_id,
                        committed_generation: generation,
                    };
                    let frame = framing::encode_frame(MessageType::GenerationWake, &wake.encode());
                    let _ = self.send_wake_frame(token, frame);
                }
            }
        }
    }

    fn projection_budget_allows(&self, replaced_bytes: usize, new_bytes: usize) -> bool {
        let current = self
            .local_ipc
            .as_ref()
            .map_or(0, |state| state.aggregate_projection_bytes);
        current
            .saturating_sub(replaced_bytes)
            .checked_add(new_bytes)
            .is_some_and(|total| total <= MAX_AGGREGATE_PROJECTION_BYTES)
    }

    fn install_projection_replacement(
        &mut self,
        attachment_id: AttachmentId,
        execution_id: ExecutionId,
        token: u64,
        snapshot: &producer::OwnedSnapshot,
    ) -> bool {
        let old_bytes = self
            .local_ipc
            .as_ref()
            .and_then(|state| state.projections.get(&attachment_id))
            .map_or(0, |projection| projection.region.region_bytes());
        let Ok((mut projection, committed_generation)) =
            build_projection(attachment_id, execution_id, token, snapshot)
        else {
            return false;
        };
        let new_bytes = projection.region.region_bytes();
        if !self.projection_budget_allows(old_bytes, new_bytes) {
            return false;
        }
        let Some(reader_fd) = projection.region.take_reader_fd() else {
            return false;
        };
        let message = WireProjectionReplaced {
            execution_id,
            attachment_id,
            projection_id: projection.projection_id,
            committed_generation,
            region_bytes: new_bytes as u64,
            capacity_rows: projection.capacity_rows,
            capacity_cols: projection.capacity_cols,
        };
        let frame = framing::encode_frame(MessageType::ProjectionReplaced, &message.encode());
        if !self.send_mandatory_frame(token, frame, Some(reader_fd)) {
            return false;
        }
        let Some(state) = self.local_ipc.as_mut() else {
            return false;
        };
        if state
            .attachments
            .replace_projection(attachment_id, projection.projection_id)
            .is_err()
        {
            return false;
        }
        let replaced = state.projections.insert(attachment_id, projection);
        if let Some(old) = replaced {
            state.aggregate_projection_bytes = state
                .aggregate_projection_bytes
                .saturating_sub(old.region.region_bytes());
        }
        state.aggregate_projection_bytes += new_bytes;
        true
    }

    fn mark_projection_unavailable(&mut self, attachment_id: AttachmentId) {
        let Some(state) = self.local_ipc.as_mut() else {
            return;
        };
        if let Some(old) = state.projections.remove(&attachment_id) {
            state.aggregate_projection_bytes = state
                .aggregate_projection_bytes
                .saturating_sub(old.region.region_bytes());
        }
    }
}

#[cfg(target_os = "macos")]
fn align_up(value: usize, alignment: usize) -> Option<usize> {
    value
        .checked_add(alignment - 1)
        .map(|v| v / alignment * alignment)
}

#[cfg(target_os = "macos")]
fn projection_geometry(rows: u16, cols: u16) -> Result<(u16, u16, u64, u64), ()> {
    use crate::projection::layout::{CELL_LEN, DAMAGE_LEN, SLOT_HEADER_LEN};
    if rows == 0 || cols == 0 || rows > MAX_CAPACITY_ROWS || cols > MAX_CAPACITY_COLS {
        return Err(());
    }
    let cell_bytes = (rows as usize)
        .checked_mul(cols as usize)
        .and_then(|count| count.checked_mul(CELL_LEN))
        .ok_or(())?;
    let damage_bytes = (rows as usize).checked_mul(DAMAGE_LEN).ok_or(())?;
    let stride = SLOT_HEADER_LEN
        .checked_add(cell_bytes)
        .and_then(|value| value.checked_add(damage_bytes))
        .and_then(|value| align_up(value, 64))
        .ok_or(())?;
    let region_bytes = REGION_HEADER_LEN
        .checked_add(stride.checked_mul(2).ok_or(())?)
        .ok_or(())?;
    if region_bytes as u64 > MAX_REGION_BYTES {
        return Err(());
    }
    Ok((rows, cols, stride as u64, region_bytes as u64))
}

#[cfg(target_os = "macos")]
fn build_projection(
    attachment_id: AttachmentId,
    execution_id: ExecutionId,
    connection_token: u64,
    snapshot: &producer::OwnedSnapshot,
) -> Result<(ProjectionEntry, u64), ()> {
    let (capacity_rows, capacity_cols, slot_stride, region_bytes) =
        projection_geometry(snapshot.rows, snapshot.columns)?;
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
    let generation = writer
        .publish(&snapshot.as_snapshot_write())
        .map_err(|_| ())?;
    Ok((
        ProjectionEntry {
            execution_id,
            connection_token,
            writer,
            region,
            projection_id,
            capacity_rows,
            capacity_cols,
        },
        generation,
    ))
}
