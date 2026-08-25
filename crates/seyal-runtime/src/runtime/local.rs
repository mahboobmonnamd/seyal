use std::{collections::HashMap, path::PathBuf};

use seyal_exec::{ExecutionReactor, ReactorEventKind, RegistrationToken, WindowSize};

use crate::{
    AttachmentId, ExecutionId, RuntimeError,
    display::{self, EncodedDisplayBatch, MAX_DISPLAY_COLUMNS, MAX_DISPLAY_ROWS},
    local_ipc::{
        attachment::{AttachmentError, AttachmentRegistry, MAX_LIVE_ATTACHMENTS},
        connection::{
            ConnectionState as LocalIpcConnState, DeltaEnqueueResult, LocalIpcServer, ServerEvent,
        },
        discovery,
        framing::{
            self, Attach as WireAttach, Attached as WireAttached, ErrorCode, ExecutionList,
            ExecutionListEntry, Lifecycle as WireLifecycle, MessageType, Resize as WireResize, Role,
        },
    },
};

use super::{ExecutionLifecycle, Runtime};

pub(super) struct ConnectionMeta {
    attachment: Option<AttachmentId>,
    reactor_token: RegistrationToken,
}

#[derive(Clone, Copy)]
struct PublishedDisplay {
    generation: u64,
    rows: u16,
    columns: u16,
}

pub(super) struct LocalIpcState {
    pub(super) server: LocalIpcServer,
    pub(super) socket_path: PathBuf,
    pub(super) listener_reactor_token: RegistrationToken,
    attachments: AttachmentRegistry,
    connections: HashMap<u64, ConnectionMeta>,
    reactor_connections: HashMap<RegistrationToken, u64>,
    published: HashMap<ExecutionId, PublishedDisplay>,
}

impl LocalIpcState {
    pub(super) fn bind(
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
        let server = LocalIpcServer::bind(
            &socket_path,
            crate::local_ipc::connection::MAX_CONNECTIONS,
        )?;
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
            published: HashMap::new(),
        })
    }
}

impl Drop for LocalIpcState {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

impl Runtime {
    pub(super) fn service_local_reactor_event(
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
        }
        if let Some(execution_id) = execution_id {
            if let Some(entry) = self.entries.get_mut(&execution_id) {
                entry.attachments.remove(&attachment_id);
            }
            let no_viewers = self.local_ipc.as_ref().is_none_or(|state| {
                state.attachments.attachments_for_execution(execution_id) == 0
            });
            if no_viewers
                && let Some(state) = self.local_ipc.as_mut()
            {
                state.published.remove(&execution_id);
            }
        }
    }

    fn send_mandatory_frame(&mut self, token: u64, bytes: Vec<u8>) -> bool {
        let queued = self.local_ipc.as_mut().is_some_and(|state| {
            state.server.enqueue_mandatory(token, bytes).is_ok()
        });
        if !queued {
            self.close_local_connection(token);
            return false;
        }
        self.sync_local_writable(token)
    }

    fn send_after_display_frame(&mut self, token: u64, bytes: Vec<u8>) -> bool {
        let queued = self.local_ipc.as_mut().is_some_and(|state| {
            state.server.enqueue_after_display(token, bytes).is_ok()
        });
        if !queued {
            self.close_local_connection(token);
            return false;
        }
        self.sync_local_writable(token)
    }

    fn send_snapshot_batch(&mut self, token: u64, batch: EncodedDisplayBatch) -> bool {
        let queued = self.local_ipc.as_mut().is_some_and(|state| {
            state.server.enqueue_snapshot(token, batch).is_ok()
        });
        if !queued {
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
        let _ = self.send_mandatory_frame(token, frame);
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
                if payload.is_empty() {
                    self.close_local_connection(token);
                } else {
                    self.send_error(token, ErrorCode::MalformedPayload, message_type);
                }
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
            server_capabilities: framing::CAP_BINARY_DISPLAY | framing::CAP_OBSERVER,
            max_frame_payload: framing::MAX_FRAME_PAYLOAD,
            max_input_payload: framing::MAX_INPUT_BYTES,
        };
        let frame = framing::encode_frame(MessageType::ServerHello, &response.encode());
        if self.send_mandatory_frame(token, frame)
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
        let _ = self.send_mandatory_frame(token, frame);
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
            self.send_error(token, ErrorCode::InvalidExecution, MessageType::Attach as u16);
            return;
        };
        let Some(state) = self.local_ipc.as_ref() else {
            return;
        };
        if state.attachments.len() >= MAX_LIVE_ATTACHMENTS {
            self.send_error(token, ErrorCode::CapacityExceeded, MessageType::Attach as u16);
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

        let snapshot = entry.execution.projection_snapshot();
        let Ok(snapshot_batch) = display::encode_snapshot(&snapshot) else {
            self.send_error(token, ErrorCode::DisplayUnavailable, MessageType::Attach as u16);
            return;
        };
        let attachment_id = AttachmentId::new();
        let attached = WireAttached {
            execution_id: attach.execution_id,
            attachment_id,
            granted_role: attach.requested_role,
            current_generation: snapshot.source_damage_generation,
        };
        let attached_frame = framing::encode_frame(MessageType::Attached, &attached.encode());
        if !self.send_mandatory_frame(token, attached_frame)
            || !self.send_snapshot_batch(token, snapshot_batch)
        {
            return;
        }

        let first_viewer = state.attachments.attachments_for_execution(attach.execution_id) == 0;
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
                token,
            );
            if first_viewer {
                state.published.insert(
                    attach.execution_id,
                    PublishedDisplay {
                        generation: snapshot.source_damage_generation,
                        rows: snapshot.rows,
                        columns: snapshot.columns,
                    },
                );
            }
            if let Some(meta) = state.connections.get_mut(&token) {
                meta.attachment = Some(attachment_id);
            }
            state.server.set_state(token, LocalIpcConnState::Attached);
        }
        if let Some(entry) = self.entries.get_mut(&attach.execution_id) {
            entry.attachments.insert(attachment_id);
        }
    }

    fn handle_detach(&mut self, token: u64, payload: &[u8]) {
        let Ok(detach) = framing::Detach::decode(payload) else {
            self.send_error(token, ErrorCode::MalformedPayload, MessageType::Detach as u16);
            return;
        };
        let execution_id = match self.local_ipc.as_ref().map(|state| {
            state
                .attachments
                .execution_for_connection(token, detach.attachment_id)
        }) {
            Some(Ok(id)) => id,
            _ => {
                self.send_error(token, ErrorCode::StaleIdentity, MessageType::Detach as u16);
                return;
            }
        };
        let detached = self.local_ipc.as_mut().is_some_and(|state| {
            state
                .attachments
                .detach_for_connection(token, detach.attachment_id)
                .is_ok()
        });
        if !detached {
            self.send_error(token, ErrorCode::StaleIdentity, MessageType::Detach as u16);
            return;
        }
        if let Some(state) = self.local_ipc.as_mut() {
            if let Some(meta) = state.connections.get_mut(&token) {
                meta.attachment = None;
            }
            state.server.set_state(token, LocalIpcConnState::Ready);
            if state.attachments.attachments_for_execution(execution_id) == 0 {
                state.published.remove(&execution_id);
            }
        }
        if let Some(entry) = self.entries.get_mut(&execution_id) {
            entry.attachments.remove(&detach.attachment_id);
        }
        let response = framing::Detached {
            attachment_id: detach.attachment_id,
        };
        let frame = framing::encode_frame(MessageType::Detached, &response.encode());
        let _ = self.send_mandatory_frame(token, frame);
    }

    fn handle_input(&mut self, token: u64, payload: &[u8]) {
        let Ok(input) = framing::InputRef::decode(payload) else {
            self.send_error(token, ErrorCode::MalformedPayload, MessageType::Input as u16);
            return;
        };
        let execution_id = match self.local_ipc.as_ref().map(|state| {
            state
                .attachments
                .authorize_mutation(token, input.attachment_id)
        }) {
            Some(Ok(id)) => id,
            Some(Err(AttachmentError::PermissionDenied)) => {
                self.send_error(token, ErrorCode::PermissionDenied, MessageType::Input as u16);
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
            Err(_) => self.send_error(token, ErrorCode::InvalidExecution, MessageType::Input as u16),
        }
    }

    fn handle_resize(&mut self, token: u64, payload: &[u8]) {
        let Ok(resize) = WireResize::decode(payload) else {
            self.send_error(token, ErrorCode::MalformedPayload, MessageType::Resize as u16);
            return;
        };
        let execution_id = match self.local_ipc.as_ref().map(|state| {
            state
                .attachments
                .authorize_mutation(token, resize.attachment_id)
        }) {
            Some(Ok(id)) => id,
            Some(Err(AttachmentError::PermissionDenied)) => {
                self.send_error(token, ErrorCode::PermissionDenied, MessageType::Resize as u16);
                return;
            }
            _ => {
                self.send_error(token, ErrorCode::StaleIdentity, MessageType::Resize as u16);
                return;
            }
        };
        if resize.rows == 0
            || resize.columns == 0
            || resize.rows > MAX_DISPLAY_ROWS
            || resize.columns > MAX_DISPLAY_COLUMNS
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
        let execution_id = match self.local_ipc.as_ref().map(|state| {
            state
                .attachments
                .execution_for_connection(token, resync.attachment_id)
        }) {
            Some(Ok(id)) => id,
            _ => {
                self.send_error(token, ErrorCode::StaleIdentity, MessageType::Resync as u16);
                return;
            }
        };
        let Some(entry) = self.entries.get(&execution_id) else {
            self.send_error(token, ErrorCode::InvalidExecution, MessageType::Resync as u16);
            return;
        };
        let snapshot = entry.execution.projection_snapshot();
        match display::encode_snapshot(&snapshot) {
            Ok(batch) => {
                let _ = self.send_snapshot_batch(token, batch);
            }
            Err(_) => self.send_error(
                token,
                ErrorCode::DisplayUnavailable,
                MessageType::Resync as u16,
            ),
        }
    }

    pub(super) fn notify_local_ipc_execution_finalized(&mut self, execution_id: ExecutionId) {
        let notifications = {
            let Some(state) = self.local_ipc.as_mut() else {
                return;
            };
            let attachment_connections = state
                .attachments
                .attachments_with_connections_for_execution(execution_id);
            state.attachments.remove_all_for_execution(execution_id);
            state.published.remove(&execution_id);
            for (_, connection_token) in &attachment_connections {
                if let Some(meta) = state.connections.get_mut(connection_token) {
                    meta.attachment = None;
                }
            }
            attachment_connections
                .into_iter()
                .map(|(_, connection_token)| connection_token)
                .collect::<Vec<_>>()
        };
        for token in notifications {
            let message = framing::LifecycleMessage {
                execution_id,
                lifecycle: framing::Lifecycle::Finalized,
            };
            let frame = framing::encode_frame(MessageType::Lifecycle, &message.encode());
            if self.send_after_display_frame(token, frame)
                && let Some(state) = self.local_ipc.as_mut()
            {
                state.server.set_state(token, LocalIpcConnState::Ready);
            }
        }
    }

    pub(super) fn publish_display_updates(&mut self) {
        let execution_ids = self.local_ipc.as_ref().map_or_else(Vec::new, |state| {
            let mut ids = Vec::new();
            for summary in self.entries.keys().copied() {
                if state.attachments.attachments_for_execution(summary) > 0 {
                    ids.push(summary);
                }
            }
            ids
        });

        for execution_id in execution_ids {
            let update = self
                .entries
                .get_mut(&execution_id)
                .and_then(|entry| entry.execution.take_projection_update());
            let Some(update) = update else {
                continue;
            };
            let previous = self
                .local_ipc
                .as_ref()
                .and_then(|state| state.published.get(&execution_id).copied());
            if previous.is_some_and(|value| update.source_damage_generation <= value.generation) {
                continue;
            }

            let viewers = self.local_ipc.as_ref().map_or_else(Vec::new, |state| {
                state
                    .attachments
                    .attachments_with_connections_for_execution(execution_id)
            });
            if viewers.is_empty() {
                continue;
            }

            let dimensions_changed = previous.is_some_and(|value| {
                value.rows != update.rows || value.columns != update.columns
            });
            if previous.is_none() || dimensions_changed {
                if let Ok(snapshot) = display::encode_snapshot(&update) {
                    for (_, token) in viewers {
                        let _ = self.send_snapshot_batch(token, snapshot.clone());
                    }
                } else {
                    for (_, token) in viewers {
                        self.send_error(
                            token,
                            ErrorCode::DisplayUnavailable,
                            MessageType::DisplaySnapshot as u16,
                        );
                    }
                }
            } else {
                let base_generation = previous.unwrap().generation;
                match display::encode_delta(&update, base_generation) {
                    Ok(delta) => {
                        let mut snapshot: Option<EncodedDisplayBatch> = None;
                        for (_, token) in viewers {
                            let result = self.local_ipc.as_mut().and_then(|state| {
                                state.server.try_enqueue_delta(token, delta.clone()).ok()
                            });
                            match result {
                                Some(DeltaEnqueueResult::Queued | DeltaEnqueueResult::Skipped) => {
                                    self.sync_local_writable(token);
                                }
                                Some(DeltaEnqueueResult::NeedSnapshot) => {
                                    if snapshot.is_none() {
                                        snapshot = display::encode_snapshot(&update).ok();
                                    }
                                    if let Some(batch) = snapshot.clone() {
                                        let _ = self.send_snapshot_batch(token, batch);
                                    } else {
                                        self.send_error(
                                            token,
                                            ErrorCode::DisplayUnavailable,
                                            MessageType::DisplaySnapshot as u16,
                                        );
                                    }
                                }
                                None => self.close_local_connection(token),
                            }
                        }
                    }
                    Err(_) => {
                        for (_, token) in viewers {
                            self.send_error(
                                token,
                                ErrorCode::DisplayUnavailable,
                                MessageType::DisplayDelta as u16,
                            );
                        }
                    }
                }
            }

            if let Some(state) = self.local_ipc.as_mut() {
                state.published.insert(
                    execution_id,
                    PublishedDisplay {
                        generation: update.source_damage_generation,
                        rows: update.rows,
                        columns: update.columns,
                    },
                );
            }
        }
    }
}
