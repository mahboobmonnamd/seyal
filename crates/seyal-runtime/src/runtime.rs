use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::PathBuf,
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

const EVENT_CAPACITY: usize = 128;
const READ_BUFFER_SIZE: usize = 16 * 1024;
const CONTROL_DISPATCH_QUANTUM: usize = 64;
const ROLLBACK_REAP_TICK: Duration = Duration::from_millis(10);

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
}

impl Runtime {
    pub fn new(config: RuntimeConfig) -> Result<Self, RuntimeError> {
        let singleton = SingletonGuard::acquire(&config.singleton_path)?;
        let reactor = ExecutionReactor::new()?;
        let (control_tx, control_rx) = sync_channel(config.control_queue_capacity);
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
        })
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

        if let Some(exit) = execution.try_wait()? {
            self.reactor.deregister(token)?;
            return Err(RuntimeError::ChildExitedBeforePublication(exit));
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
