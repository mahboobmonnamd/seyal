use std::{
    collections::VecDeque,
    io::{Read, Write},
    net::Shutdown,
    os::{fd::AsRawFd, unix::net::UnixStream},
    path::Path,
    time::Duration,
};

use seyal_render::{
    CellSource, CommittedDisplay, CursorState, PreparationResult, PreparedSurface,
    RenderAttributes, RenderCell, RenderColor, RowDamage,
};
use seyal_runtime::{
    AttachmentId, ExecutionId,
    display::{
        DISPLAY_CELL_LEN, DISPLAY_CHUNK_HEADER_LEN, DecodedDisplayChunk, DisplayAttributes,
        DisplayCache, DisplayCell, DisplayColor, DisplayError, DisplayKind,
        MAX_DISPLAY_BATCH_BYTES, MAX_DISPLAY_CELLS, decode_chunk, empty_cache,
    },
    local_ipc::{
        discovery::{control_socket_path, darwin_user_runtime_dir, ensure_verified_runtime_dir},
        framing::{
            Attach, Attached, CAP_BINARY_DISPLAY, CAP_CORRELATED_RESIZE, CAP_SEMANTIC_TERMINAL_KEY,
            ClientHello, ErrorCode, ErrorMessage, ExecutionList, FrameHeader, HEADER_LEN, InputRef,
            Lifecycle, LifecycleMessage, MAX_FRAME_PAYLOAD, MAX_INPUT_BYTES, MessageType,
            ResizeRequest, ResizeResult, ResizeResultCode, Resync, Role, ServerHello, TerminalKey,
            TerminalKeyKind, TerminalKeyModifiers, encode_frame,
        },
    },
    pass8::{BLOCK_STATE_MESSAGE_TYPE, BlockLifecycle, BlockState, CAP_BLOCK_METADATA},
};

use crate::block::{BlockCache, is_epoch_quarantined, quarantine_epoch};

const STARTUP_TIMEOUT: Duration = Duration::from_secs(2);
const READ_CHUNK_BYTES: usize = 64 * 1024;
const MAX_BUFFERED_BYTES: usize = (MAX_FRAME_PAYLOAD as usize + HEADER_LEN) * 2;
const MAX_FRAMES_PER_POLL: usize = 64;
const MAX_BYTES_PER_POLL: usize = 4 * 1024 * 1024;
const MAX_OUTBOUND_WIRE_BYTES: usize = 262_144;
const MAX_UNRESOLVED_RESIZES: usize = 1_024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientError {
    RuntimeDiscovery,
    Io,
    Protocol,
    UnsupportedDisplayCapability,
    UnsupportedInteractiveCapability,
    NoRunningExecution,
    InvalidAttachment,
    Display,
    Prepare,
    Server(u16),
    Disconnected,
    Capacity,
    ClientBackpressure,
    CommitTooLarge,
    LostController,
    ResizeProtocolFailure,
    InvalidGeometry,
    BlockMetadataConflict,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputAdmissionFailure {
    ClientBackpressure,
    CommitTooLarge,
    LostController,
    Disconnected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResizeFailure {
    ClientBackpressure,
    Apply(ErrorCode),
    Protocol,
    Disconnected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GridGeometry {
    pub rows: u16,
    pub columns: u16,
}

pub fn derive_grid_geometry(
    viewport_width: f64,
    viewport_height: f64,
    horizontal_insets: f64,
    vertical_insets: f64,
    cell_width: f64,
    cell_height: f64,
) -> Option<GridGeometry> {
    let operands = [
        viewport_width,
        viewport_height,
        horizontal_insets,
        vertical_insets,
        cell_width,
        cell_height,
    ];
    if operands.iter().any(|value| !value.is_finite())
        || viewport_width < 0.0
        || viewport_height < 0.0
        || horizontal_insets < 0.0
        || vertical_insets < 0.0
        || cell_width <= 0.0
        || cell_height <= 0.0
    {
        return None;
    }

    let usable_width = viewport_width - horizontal_insets;
    let usable_height = viewport_height - vertical_insets;
    if !usable_width.is_finite()
        || !usable_height.is_finite()
        || usable_width <= 0.0
        || usable_height <= 0.0
    {
        return None;
    }

    let column_ratio = usable_width / cell_width;
    let row_ratio = usable_height / cell_height;
    if !column_ratio.is_finite()
        || !row_ratio.is_finite()
        || column_ratio <= 0.0
        || row_ratio <= 0.0
    {
        return None;
    }

    let columns = column_ratio.floor().clamp(1.0, 512.0) as u16;
    let rows = row_ratio.floor().clamp(1.0, 256.0) as u16;
    Some(GridGeometry { rows, columns })
}

fn valid_terminal_key_request(kind: TerminalKeyKind, scalar: u32) -> bool {
    match kind {
        TerminalKeyKind::ControlAscii => matches!(scalar, 0x20 | 0x3f | 0x40 | 0x41..=0x5f),
        _ => scalar == 0,
    }
}

#[derive(Debug, Default)]
struct PendingDisplayBatch {
    chunks: Vec<DecodedDisplayChunk>,
    cells: usize,
    rows: usize,
    wire_bytes: usize,
}

impl PendingDisplayBatch {
    fn push(&mut self, chunk: DecodedDisplayChunk) -> Result<bool, ClientError> {
        let expected_count = usize::from(chunk.chunk_count);
        if expected_count == 0 || expected_count > usize::from(chunk.rows) {
            return Err(ClientError::Capacity);
        }

        if self.chunks.is_empty() {
            if chunk.chunk_index != 0 {
                return Err(ClientError::Protocol);
            }
            if chunk.kind == DisplayKind::Snapshot && chunk.first_row != 0 {
                return Err(ClientError::Protocol);
            }
            self.chunks.reserve(expected_count);
        } else {
            let first = self.chunks.first().ok_or(ClientError::Protocol)?;
            let previous = self.chunks.last().ok_or(ClientError::Protocol)?;
            if chunk.kind != first.kind
                || chunk.generation != first.generation
                || chunk.base_generation != first.base_generation
                || chunk.rows != first.rows
                || chunk.columns != first.columns
                || chunk.cursor_row != first.cursor_row
                || chunk.cursor_col != first.cursor_col
                || chunk.cursor_visible != first.cursor_visible
                || chunk.alternate_screen != first.alternate_screen
                || chunk.chunk_count != first.chunk_count
                || usize::from(chunk.chunk_index) != self.chunks.len()
            {
                return Err(ClientError::Protocol);
            }
            let expected_first_row = previous
                .first_row
                .checked_add(previous.row_count)
                .ok_or(ClientError::Capacity)?;
            if chunk.first_row != expected_first_row {
                return Err(ClientError::Protocol);
            }
        }

        if self.chunks.len() >= expected_count {
            return Err(ClientError::Protocol);
        }

        let next_rows = self
            .rows
            .checked_add(usize::from(chunk.row_count))
            .ok_or(ClientError::Capacity)?;
        if next_rows > usize::from(chunk.rows) {
            return Err(ClientError::Capacity);
        }

        let geometry_cells = usize::from(chunk.rows)
            .checked_mul(usize::from(chunk.columns))
            .ok_or(ClientError::Capacity)?;
        let next_cells = self
            .cells
            .checked_add(chunk.cells.len())
            .ok_or(ClientError::Capacity)?;
        if next_cells > geometry_cells || next_cells > MAX_DISPLAY_CELLS {
            return Err(ClientError::Capacity);
        }

        let chunk_wire_bytes = HEADER_LEN
            .checked_add(DISPLAY_CHUNK_HEADER_LEN)
            .and_then(|value| {
                chunk
                    .cells
                    .len()
                    .checked_mul(DISPLAY_CELL_LEN)
                    .and_then(|cell_bytes| value.checked_add(cell_bytes))
            })
            .ok_or(ClientError::Capacity)?;
        let next_wire_bytes = self
            .wire_bytes
            .checked_add(chunk_wire_bytes)
            .ok_or(ClientError::Capacity)?;
        if next_wire_bytes > MAX_DISPLAY_BATCH_BYTES {
            return Err(ClientError::Capacity);
        }

        self.rows = next_rows;
        self.cells = next_cells;
        self.wire_bytes = next_wire_bytes;
        self.chunks.push(chunk);
        Ok(self.chunks.len() == expected_count)
    }

    fn chunks(&self) -> &[DecodedDisplayChunk] {
        &self.chunks
    }

    fn clear(&mut self) {
        self.chunks.clear();
        self.cells = 0;
        self.rows = 0;
        self.wire_bytes = 0;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OutboundKind {
    Input,
    TerminalKey,
    Resize {
        request_id: u64,
        geometry: GridGeometry,
    },
    Resync,
}

#[derive(Debug)]
struct PendingControlWrite {
    bytes: Vec<u8>,
    offset: usize,
    kind: OutboundKind,
}

impl PendingControlWrite {
    fn new(bytes: Vec<u8>, kind: OutboundKind) -> Self {
        Self {
            bytes,
            offset: 0,
            kind,
        }
    }

    fn remaining(&self) -> &[u8] {
        &self.bytes[self.offset..]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ResizePhase {
    QueuedNotStarted,
    Writing,
    SentWaitingResult,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ResizeRecord {
    request_id: u64,
    geometry: GridGeometry,
    phase: ResizePhase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AppliedFence {
    request_id: u64,
    geometry: GridGeometry,
    applied_generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RetrySuppression {
    geometry: GridGeometry,
}

fn newest_pending_geometry(
    unresolved: &VecDeque<ResizeRecord>,
    applied_fence: Option<AppliedFence>,
) -> Option<GridGeometry> {
    let unresolved_latest = unresolved
        .iter()
        .max_by_key(|record| record.request_id)
        .map(|record| (record.request_id, record.geometry));
    let applied_latest = applied_fence.map(|fence| (fence.request_id, fence.geometry));
    match (unresolved_latest, applied_latest) {
        (Some(unresolved), Some(applied)) => Some(if unresolved.0 > applied.0 {
            unresolved.1
        } else {
            applied.1
        }),
        (Some(unresolved), None) => Some(unresolved.1),
        (None, Some(applied)) => Some(applied.1),
        (None, None) => None,
    }
}

fn resize_needs_mutation(
    desired: GridGeometry,
    committed: GridGeometry,
    newest_pending: Option<GridGeometry>,
) -> bool {
    if newest_pending == Some(desired) {
        return false;
    }
    !(newest_pending.is_none() && committed == desired)
}

pub struct LocalDisplayClient {
    stream: UnixStream,
    buffered: Vec<u8>,
    read_offset: usize,
    pending_batch: PendingDisplayBatch,
    outbound: VecDeque<PendingControlWrite>,
    outbound_wire_bytes: usize,
    runtime_id: u128,
    execution_id: ExecutionId,
    attachment_id: AttachmentId,
    role: Role,
    block_metadata_negotiated: bool,
    block_cache: BlockCache,
    cache: DisplayCache,
    prepared: PreparedSurface,
    last_preparation: PreparationResult,
    next_resize_request_id: u64,
    desired_geometry: Option<GridGeometry>,
    committed_geometry: GridGeometry,
    unresolved_resizes: VecDeque<ResizeRecord>,
    applied_awaiting_projection: Option<AppliedFence>,
    retry_suppression: Option<RetrySuppression>,
    resync_needed: bool,
    input_failure: Option<InputAdmissionFailure>,
    resize_failure: Option<ResizeFailure>,
}

impl LocalDisplayClient {
    /// Connect to the verified per-user Runtime and attach as Controller to the
    /// first running execution. Pass 7 makes the permanent native surface the
    /// interactive production terminal; an existing controller is surfaced as
    /// an explicit attach error rather than silently degrading to Observer.
    pub fn connect_first_running() -> Result<Self, ClientError> {
        let runtime_dir = darwin_user_runtime_dir().map_err(|_| ClientError::RuntimeDiscovery)?;
        ensure_verified_runtime_dir(&runtime_dir).map_err(|_| ClientError::RuntimeDiscovery)?;
        let socket_path =
            control_socket_path(&runtime_dir).map_err(|_| ClientError::RuntimeDiscovery)?;

        let mut stream = connect_stream(&socket_path)?;
        let mut server_hello = hello(&mut stream, true, true)?;
        send_control(&mut stream, MessageType::ListExecutions, &[])?;
        let (kind, payload) = read_blocking_frame(&mut stream)?;
        if kind != MessageType::ExecutionList {
            return Err(ClientError::Protocol);
        }
        let list = ExecutionList::decode(&payload).map_err(|_| ClientError::Protocol)?;
        let execution_id = list
            .entries
            .iter()
            .find(|entry| entry.lifecycle == Lifecycle::Running)
            .map(|entry| entry.execution_id)
            .ok_or(ClientError::NoRunningExecution)?;

        if is_epoch_quarantined(server_hello.runtime_id, execution_id) {
            drop(stream);
            stream = connect_stream(&socket_path)?;
            server_hello = hello(&mut stream, true, false)?;
        }
        let block_metadata_negotiated =
            server_hello.server_capabilities & CAP_BLOCK_METADATA != 0
                && !is_epoch_quarantined(server_hello.runtime_id, execution_id);
        Self::finish_attach(
            stream,
            execution_id,
            Role::Controller,
            server_hello.runtime_id,
            block_metadata_negotiated,
        )
    }

    pub fn connect_execution(
        socket_path: &Path,
        execution_id: ExecutionId,
        role: Role,
    ) -> Result<Self, ClientError> {
        let mut stream = connect_stream(socket_path)?;
        let mut server_hello = hello(&mut stream, role == Role::Controller, true)?;
        if is_epoch_quarantined(server_hello.runtime_id, execution_id) {
            drop(stream);
            stream = connect_stream(socket_path)?;
            server_hello = hello(&mut stream, role == Role::Controller, false)?;
        }
        let block_metadata_negotiated =
            server_hello.server_capabilities & CAP_BLOCK_METADATA != 0
                && !is_epoch_quarantined(server_hello.runtime_id, execution_id);
        Self::finish_attach(
            stream,
            execution_id,
            role,
            server_hello.runtime_id,
            block_metadata_negotiated,
        )
    }

    fn finish_attach(
        mut stream: UnixStream,
        execution_id: ExecutionId,
        role: Role,
        runtime_id: u128,
        block_metadata_negotiated: bool,
    ) -> Result<Self, ClientError> {
        send_control(
            &mut stream,
            MessageType::Attach,
            &Attach {
                execution_id,
                requested_role: role,
            }
            .encode(),
        )?;
        let (kind, payload) = read_blocking_frame(&mut stream)?;
        if kind == MessageType::Error {
            let error = ErrorMessage::decode(&payload).map_err(|_| ClientError::Protocol)?;
            return Err(ClientError::Server(error.error_code));
        }
        if kind != MessageType::Attached {
            return Err(ClientError::Protocol);
        }
        let attached = Attached::decode(&payload).map_err(|_| ClientError::Protocol)?;
        if attached.execution_id != execution_id || attached.granted_role != role {
            return Err(ClientError::InvalidAttachment);
        }

        let first_frame = read_blocking_raw_frame(&mut stream)?;
        let first = decode_chunk(&first_frame).map_err(|_| ClientError::Display)?;
        if first.kind != DisplayKind::Snapshot || first.chunk_index != 0 {
            return Err(ClientError::Protocol);
        }
        let chunk_count = first.chunk_count;
        let mut batch = PendingDisplayBatch::default();
        let mut complete = batch.push(first)?;
        for _ in 1..chunk_count {
            let frame = read_blocking_raw_frame(&mut stream)?;
            complete = batch.push(decode_chunk(&frame).map_err(|_| ClientError::Display)?)?;
        }
        if !complete {
            return Err(ClientError::Protocol);
        }

        let mut cache = empty_cache();
        cache
            .apply_chunks(batch.chunks())
            .map_err(|_| ClientError::Display)?;
        if cache.generation != attached.current_generation || cache.rows == 0 || cache.columns == 0
        {
            return Err(ClientError::Protocol);
        }

        let committed_geometry = GridGeometry {
            rows: cache.rows,
            columns: cache.columns,
        };
        let mut prepared = PreparedSurface::default();
        let result = prepare_cache(&mut prepared, &cache, RowDamage::full(cache.rows), true)?;

        stream.set_read_timeout(None).map_err(|_| ClientError::Io)?;
        stream
            .set_write_timeout(None)
            .map_err(|_| ClientError::Io)?;
        stream.set_nonblocking(true).map_err(|_| ClientError::Io)?;

        batch.clear();
        Ok(Self {
            stream,
            buffered: Vec::with_capacity(READ_CHUNK_BYTES),
            read_offset: 0,
            pending_batch: batch,
            outbound: VecDeque::new(),
            outbound_wire_bytes: 0,
            runtime_id,
            execution_id,
            attachment_id: attached.attachment_id,
            role,
            block_metadata_negotiated,
            block_cache: BlockCache::default(),
            cache,
            prepared,
            last_preparation: result,
            next_resize_request_id: 1,
            desired_geometry: None,
            committed_geometry,
            unresolved_resizes: VecDeque::new(),
            applied_awaiting_projection: None,
            retry_suppression: None,
            resync_needed: false,
            input_failure: None,
            resize_failure: None,
        })
    }

    pub fn socket_fd(&self) -> i32 {
        self.stream.as_raw_fd()
    }

    pub fn execution_id(&self) -> ExecutionId {
        self.execution_id
    }

    pub fn block_state(&self) -> Option<BlockState> {
        self.block_cache.visible()
    }

    pub fn cache(&self) -> &DisplayCache {
        &self.cache
    }

    pub fn prepared_surface(&self) -> &PreparedSurface {
        &self.prepared
    }

    pub fn last_preparation(&self) -> PreparationResult {
        self.last_preparation
    }

    pub fn wants_write(&self) -> bool {
        !self.outbound.is_empty()
    }

    pub fn input_failure(&self) -> Option<InputAdmissionFailure> {
        self.input_failure
    }

    pub fn resize_failure(&self) -> Option<ResizeFailure> {
        self.resize_failure
    }

    pub fn outbound_wire_bytes(&self) -> usize {
        self.outbound_wire_bytes
    }

    pub fn submit_committed_text(&mut self, text: &str) -> Result<(), ClientError> {
        self.require_controller()?;
        let bytes = text.as_bytes();
        if bytes.is_empty() {
            return Ok(());
        }
        if bytes.len() > MAX_INPUT_BYTES as usize {
            self.input_failure = Some(InputAdmissionFailure::CommitTooLarge);
            return Err(ClientError::CommitTooLarge);
        }
        let payload = InputRef {
            attachment_id: self.attachment_id,
            bytes,
        }
        .encode();
        let frame = encode_frame(MessageType::Input, &payload);
        if let Err(error) = self.admit_frame(frame, OutboundKind::Input) {
            self.input_failure = Some(InputAdmissionFailure::ClientBackpressure);
            return Err(error);
        }
        self.input_failure = None;
        self.flush_control_write()
    }

    pub fn submit_terminal_key(
        &mut self,
        kind: TerminalKeyKind,
        scalar: u32,
    ) -> Result<(), ClientError> {
        self.require_controller()?;
        if !valid_terminal_key_request(kind, scalar) {
            return Err(ClientError::Protocol);
        }
        let modifiers = if kind == TerminalKeyKind::ControlAscii {
            TerminalKeyModifiers::CONTROL
        } else {
            TerminalKeyModifiers::NONE
        };
        let payload = TerminalKey {
            attachment_id: self.attachment_id,
            kind,
            modifiers,
            scalar,
        }
        .encode();
        let frame = encode_frame(MessageType::TerminalKey, &payload);
        if let Err(error) = self.admit_frame(frame, OutboundKind::TerminalKey) {
            self.input_failure = Some(InputAdmissionFailure::ClientBackpressure);
            return Err(error);
        }
        self.input_failure = None;
        self.flush_control_write()
    }

    pub fn set_desired_geometry(&mut self, geometry: GridGeometry) -> Result<(), ClientError> {
        self.set_desired_geometry_for_layout(geometry, false)
    }

    pub fn set_desired_geometry_for_layout(
        &mut self,
        geometry: GridGeometry,
        meaningful_layout_epoch: bool,
    ) -> Result<(), ClientError> {
        if geometry.rows == 0
            || geometry.columns == 0
            || geometry.rows > 256
            || geometry.columns > 512
        {
            return Err(ClientError::InvalidGeometry);
        }
        self.require_controller()?;
        let geometry_changed = self.desired_geometry != Some(geometry);
        self.desired_geometry = Some(geometry);
        if geometry_changed
            || (meaningful_layout_epoch
                && self
                    .retry_suppression
                    .is_some_and(|suppressed| suppressed.geometry == geometry))
        {
            self.retry_suppression = None;
            self.resize_failure = None;
        }
        self.reconcile_resize()?;
        self.flush_control_write()
    }

    pub fn retry_resize(&mut self) -> Result<(), ClientError> {
        self.retry_suppression = None;
        self.resize_failure = None;
        self.reconcile_resize()?;
        self.flush_control_write()
    }

    fn require_controller(&mut self) -> Result<(), ClientError> {
        if self.role != Role::Controller {
            self.input_failure = Some(InputAdmissionFailure::LostController);
            return Err(ClientError::LostController);
        }
        Ok(())
    }

    fn admit_frame(&mut self, bytes: Vec<u8>, kind: OutboundKind) -> Result<(), ClientError> {
        #[cfg(feature = "benchmark-instrumentation")]
        let benchmark_input = matches!(kind, OutboundKind::Input | OutboundKind::TerminalKey);
        let next = self
            .outbound_wire_bytes
            .checked_add(bytes.len())
            .ok_or(ClientError::Capacity)?;
        if next > MAX_OUTBOUND_WIRE_BYTES {
            return Err(ClientError::ClientBackpressure);
        }
        self.outbound_wire_bytes = next;
        self.outbound
            .push_back(PendingControlWrite::new(bytes, kind));
        #[cfg(feature = "benchmark-instrumentation")]
        {
            crate::pass7_benchmark::observe_pass7_client_queue(self.outbound_wire_bytes);
            if benchmark_input {
                crate::pass7_benchmark::mark_pass7_client_admission(self.outbound_wire_bytes);
            }
        }
        Ok(())
    }

    /// Complete at most one nonblocking write attempt. Accepted wire bytes are
    /// decremented only as the socket actually accepts them; a partial frame is
    /// immutable and remains at the front of the FIFO until complete.
    pub fn flush_control_write(&mut self) -> Result<(), ClientError> {
        let (written, resize_phase, completed) = {
            let Some(front) = self.outbound.front_mut() else {
                return Ok(());
            };
            let remaining_len = front.remaining().len();
            if remaining_len == 0 {
                return Err(ClientError::Protocol);
            }
            match self.stream.write(front.remaining()) {
                Ok(0) => return Err(ClientError::Io),
                Ok(count) if count <= remaining_len => {
                    front.offset = front
                        .offset
                        .checked_add(count)
                        .ok_or(ClientError::Capacity)?;
                    let phase = match front.kind {
                        OutboundKind::Resize { request_id, .. } => Some((
                            request_id,
                            if count == remaining_len {
                                ResizePhase::SentWaitingResult
                            } else {
                                ResizePhase::Writing
                            },
                        )),
                        _ => None,
                    };
                    (count, phase, front.offset == front.bytes.len())
                }
                Ok(_) => return Err(ClientError::Io),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
                Err(_) => {
                    self.input_failure = Some(InputAdmissionFailure::Disconnected);
                    self.resize_failure = Some(ResizeFailure::Disconnected);
                    return Err(ClientError::Io);
                }
            }
        };

        self.outbound_wire_bytes = self.outbound_wire_bytes.saturating_sub(written);
        if let Some((request_id, phase)) = resize_phase {
            self.set_resize_phase(request_id, phase)?;
        }
        if completed {
            #[cfg(feature = "benchmark-instrumentation")]
            let completed_input = self.outbound.front().is_some_and(|pending| {
                matches!(
                    pending.kind,
                    OutboundKind::Input | OutboundKind::TerminalKey
                )
            });
            self.outbound.pop_front();
            #[cfg(feature = "benchmark-instrumentation")]
            if completed_input {
                crate::pass7_benchmark::mark_pass7_client_socket_complete(self.outbound_wire_bytes);
            }
        }
        if self.input_failure == Some(InputAdmissionFailure::ClientBackpressure) {
            self.input_failure = None;
        }
        self.try_queue_resync()?;
        self.reconcile_resize()?;
        Ok(())
    }

    pub fn poll_prepare(&mut self) -> Result<Option<PreparationResult>, ClientError> {
        let mut committed_any = false;
        let mut damage = RowDamage::none();
        let mut full_invalidation = false;
        let mut parsed_frames = 0usize;
        let mut bytes_read = 0usize;

        loop {
            while parsed_frames < MAX_FRAMES_PER_POLL {
                let Some(frame_end) = self.complete_frame_end()? else {
                    break;
                };
                let frame_start = self.read_offset;
                let frame = &self.buffered[frame_start..frame_end];
                let header = FrameHeader::decode(frame).map_err(|_| ClientError::Protocol)?;

                if header.message_type == BLOCK_STATE_MESSAGE_TYPE {
                    if !self.block_metadata_negotiated {
                        return Err(self.quarantine_block_metadata());
                    }
                    let incoming = BlockState::decode(&frame[HEADER_LEN..])
                        .map_err(|_| self.quarantine_block_metadata())?;
                    if self.block_cache.apply(self.execution_id, incoming).is_err() {
                        return Err(self.quarantine_block_metadata());
                    }
                    self.read_offset = frame_end;
                    parsed_frames += 1;
                    continue;
                }

                let message_type =
                    MessageType::from_u16(header.message_type).ok_or(ClientError::Protocol)?;
                match message_type {
                    MessageType::DisplaySnapshot | MessageType::DisplayDelta => {
                        let chunk = decode_chunk(frame).map_err(|_| ClientError::Display)?;
                        if self.accept_display_chunk(chunk, &mut damage, &mut full_invalidation)? {
                            committed_any = true;
                        }
                    }
                    MessageType::ResizeResult => {
                        let payload = &frame[HEADER_LEN..];
                        let result = ResizeResult::decode(payload).map_err(|_| {
                            self.resize_failure = Some(ResizeFailure::Protocol);
                            ClientError::ResizeProtocolFailure
                        })?;
                        self.accept_resize_result(result)?;
                    }
                    MessageType::Error => {
                        let payload = &frame[HEADER_LEN..];
                        let error =
                            ErrorMessage::decode(payload).map_err(|_| ClientError::Protocol)?;
                        if let Some(failure) = classify_server_error(error)? {
                            self.input_failure = Some(failure);
                        }
                    }
                    MessageType::Lifecycle => {
                        let lifecycle = LifecycleMessage::decode(&frame[HEADER_LEN..])
                            .map_err(|_| ClientError::Protocol)?;
                        if lifecycle.execution_id != self.execution_id {
                            return Err(ClientError::Protocol);
                        }
                        if lifecycle.lifecycle == Lifecycle::Finalized
                            && self.block_metadata_negotiated
                            && self.block_cache.visible().is_some_and(|block| {
                                block.state == BlockLifecycle::Current
                            })
                        {
                            return Err(self.quarantine_block_metadata());
                        }
                    }
                    _ => return Err(ClientError::Protocol),
                }
                self.read_offset = frame_end;
                parsed_frames += 1;
            }

            if parsed_frames >= MAX_FRAMES_PER_POLL || bytes_read >= MAX_BYTES_PER_POLL {
                break;
            }
            if self.complete_frame_end()?.is_some() {
                continue;
            }

            let mut chunk = [0u8; READ_CHUNK_BYTES];
            match self.stream.read(&mut chunk) {
                Ok(0) => {
                    self.input_failure = Some(InputAdmissionFailure::Disconnected);
                    self.resize_failure = Some(ResizeFailure::Disconnected);
                    return Err(ClientError::Disconnected);
                }
                Ok(count) => {
                    let live_bytes = self.buffered.len().saturating_sub(self.read_offset);
                    if live_bytes
                        .checked_add(count)
                        .is_none_or(|total| total > MAX_BUFFERED_BYTES)
                    {
                        return Err(ClientError::Capacity);
                    }
                    if self.read_offset != 0 {
                        self.compact_buffer();
                    }
                    self.buffered.extend_from_slice(&chunk[..count]);
                    bytes_read = bytes_read.saturating_add(count);
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(_) => return Err(ClientError::Io),
            }
        }

        self.compact_buffer();
        if !committed_any {
            return Ok(None);
        }

        let result = prepare_cache(&mut self.prepared, &self.cache, damage, full_invalidation)?;
        self.last_preparation = result;
        Ok(Some(result))
    }

    fn quarantine_block_metadata(&mut self) -> ClientError {
        self.block_cache.quarantine();
        quarantine_epoch(self.runtime_id, self.execution_id);
        let _ = self.stream.shutdown(Shutdown::Both);
        ClientError::BlockMetadataConflict
    }

    fn complete_frame_end(&self) -> Result<Option<usize>, ClientError> {
        let available = self.buffered.len().saturating_sub(self.read_offset);
        if available < HEADER_LEN {
            return Ok(None);
        }
        let header_end = self
            .read_offset
            .checked_add(HEADER_LEN)
            .ok_or(ClientError::Capacity)?;
        let header = FrameHeader::decode(&self.buffered[self.read_offset..header_end])
            .map_err(|_| ClientError::Protocol)?;
        let total = HEADER_LEN
            .checked_add(header.payload_len as usize)
            .ok_or(ClientError::Capacity)?;
        if available < total {
            return Ok(None);
        }
        self.read_offset
            .checked_add(total)
            .map(Some)
            .ok_or(ClientError::Capacity)
    }

    fn accept_display_chunk(
        &mut self,
        chunk: DecodedDisplayChunk,
        damage: &mut RowDamage,
        full_invalidation: &mut bool,
    ) -> Result<bool, ClientError> {
        if !self.pending_batch.push(chunk)? {
            return Ok(false);
        }

        let first_kind = self
            .pending_batch
            .chunks()
            .first()
            .map(|first| first.kind)
            .ok_or(ClientError::Protocol)?;
        match self.cache.apply_chunks(self.pending_batch.chunks()) {
            Ok(()) => {}
            Err(DisplayError::GenerationMismatch | DisplayError::DimensionMismatch) => {
                self.pending_batch.clear();
                self.request_resync()?;
                return Ok(false);
            }
            Err(_) => return Err(ClientError::Display),
        }

        match first_kind {
            DisplayKind::Snapshot => {
                *damage = RowDamage::full(self.cache.rows);
                *full_invalidation = true;
            }
            DisplayKind::Delta => {
                for committed in self.pending_batch.chunks() {
                    damage.union(
                        RowDamage::from_range(committed.first_row, committed.row_count)
                            .map_err(|_| ClientError::Prepare)?,
                    );
                }
            }
        }
        self.pending_batch.clear();
        self.observe_projection()?;
        Ok(true)
    }

    fn observe_projection(&mut self) -> Result<(), ClientError> {
        let geometry = GridGeometry {
            rows: self.cache.rows,
            columns: self.cache.columns,
        };
        self.committed_geometry = geometry;
        if let Some(fence) = self.applied_awaiting_projection
            && self.cache.generation >= fence.applied_generation
        {
            if self.cache.generation == fence.applied_generation && geometry != fence.geometry {
                self.resize_failure = Some(ResizeFailure::Protocol);
                return Err(ClientError::ResizeProtocolFailure);
            }
            self.applied_awaiting_projection = None;
        }
        if self.desired_geometry == Some(geometry) && self.applied_awaiting_projection.is_none() {
            self.resize_failure = None;
        }
        self.reconcile_resize()
    }

    fn accept_resize_result(&mut self, result: ResizeResult) -> Result<(), ClientError> {
        if result.attachment_id != self.attachment_id {
            self.resize_failure = Some(ResizeFailure::Protocol);
            return Err(ClientError::ResizeProtocolFailure);
        }
        let Some(index) = self
            .unresolved_resizes
            .iter()
            .position(|record| record.request_id == result.request_id)
        else {
            self.resize_failure = Some(ResizeFailure::Protocol);
            return Err(ClientError::ResizeProtocolFailure);
        };
        let record = self
            .unresolved_resizes
            .remove(index)
            .ok_or(ClientError::ResizeProtocolFailure)?;

        match result.result_code {
            ResizeResultCode::Applied => {
                let replace_fence = self
                    .applied_awaiting_projection
                    .is_none_or(|fence| result.request_id > fence.request_id);
                if replace_fence {
                    self.applied_awaiting_projection = Some(AppliedFence {
                        request_id: result.request_id,
                        geometry: record.geometry,
                        applied_generation: result.applied_generation,
                    });
                }
                self.retry_suppression = None;
                self.resize_failure = None;
            }
            ResizeResultCode::Error(error) => {
                self.resize_failure = Some(ResizeFailure::Apply(error));
                let newer_same_desired = self.unresolved_resizes.iter().any(|candidate| {
                    candidate.request_id > record.request_id
                        && Some(candidate.geometry) == self.desired_geometry
                });
                if !newer_same_desired && Some(record.geometry) == self.desired_geometry {
                    self.retry_suppression = Some(RetrySuppression {
                        geometry: record.geometry,
                    });
                }
                if matches!(
                    error,
                    ErrorCode::InvalidState
                        | ErrorCode::InvalidExecution
                        | ErrorCode::InvalidAttachment
                        | ErrorCode::StaleIdentity
                        | ErrorCode::PermissionDenied
                        | ErrorCode::ControllerBusy
                        | ErrorCode::UnsupportedVersion
                        | ErrorCode::UnknownMessage
                        | ErrorCode::MalformedPayload
                ) {
                    self.role = Role::Observer;
                    self.input_failure = Some(InputAdmissionFailure::LostController);
                }
            }
        }
        self.reconcile_resize()
    }

    fn set_resize_phase(&mut self, request_id: u64, phase: ResizePhase) -> Result<(), ClientError> {
        let record = self
            .unresolved_resizes
            .iter_mut()
            .find(|record| record.request_id == request_id)
            .ok_or(ClientError::ResizeProtocolFailure)?;
        record.phase = phase;
        Ok(())
    }

    fn reconcile_resize(&mut self) -> Result<(), ClientError> {
        let Some(desired) = self.desired_geometry else {
            return Ok(());
        };
        if self.role != Role::Controller {
            return Ok(());
        }
        if self
            .retry_suppression
            .is_some_and(|suppressed| suppressed.geometry == desired)
        {
            return Ok(());
        }

        let newest_pending =
            newest_pending_geometry(&self.unresolved_resizes, self.applied_awaiting_projection);
        if !resize_needs_mutation(desired, self.committed_geometry, newest_pending) {
            return Ok(());
        }

        if let Some(last) = self.outbound.back_mut()
            && last.offset == 0
            && let OutboundKind::Resize {
                request_id,
                geometry,
            } = &mut last.kind
        {
            let Some(record) = self
                .unresolved_resizes
                .iter_mut()
                .find(|record| record.request_id == *request_id)
            else {
                return Err(ClientError::ResizeProtocolFailure);
            };
            if record.phase != ResizePhase::QueuedNotStarted {
                return Err(ClientError::ResizeProtocolFailure);
            }
            let payload = ResizeRequest {
                attachment_id: self.attachment_id,
                request_id: *request_id,
                rows: desired.rows,
                columns: desired.columns,
            }
            .encode();
            let replacement = encode_frame(MessageType::ResizeRequest, &payload);
            if replacement.len() != last.bytes.len() {
                return Err(ClientError::Protocol);
            }
            last.bytes = replacement;
            *geometry = desired;
            record.geometry = desired;
            return Ok(());
        }

        if self.unresolved_resizes.len() >= MAX_UNRESOLVED_RESIZES {
            self.resize_failure = Some(ResizeFailure::ClientBackpressure);
            return Err(ClientError::ClientBackpressure);
        }

        let request_id = self.next_resize_request_id;
        if request_id == 0 {
            self.resize_failure = Some(ResizeFailure::Protocol);
            return Err(ClientError::ResizeProtocolFailure);
        }
        let payload = ResizeRequest {
            attachment_id: self.attachment_id,
            request_id,
            rows: desired.rows,
            columns: desired.columns,
        }
        .encode();
        let frame = encode_frame(MessageType::ResizeRequest, &payload);
        if let Err(error) = self.admit_frame(
            frame,
            OutboundKind::Resize {
                request_id,
                geometry: desired,
            },
        ) {
            if error == ClientError::ClientBackpressure {
                self.resize_failure = Some(ResizeFailure::ClientBackpressure);
            }
            return Err(error);
        }
        self.unresolved_resizes.push_back(ResizeRecord {
            request_id,
            geometry: desired,
            phase: ResizePhase::QueuedNotStarted,
        });
        self.next_resize_request_id = request_id.checked_add(1).unwrap_or(0);
        if self.resize_failure == Some(ResizeFailure::ClientBackpressure) {
            self.resize_failure = None;
        }
        Ok(())
    }

    fn request_resync(&mut self) -> Result<(), ClientError> {
        self.resync_needed = true;
        self.try_queue_resync()
    }

    fn try_queue_resync(&mut self) -> Result<(), ClientError> {
        if !self.resync_needed
            || self
                .outbound
                .iter()
                .any(|pending| pending.kind == OutboundKind::Resync)
        {
            return Ok(());
        }
        let frame = encode_frame(
            MessageType::Resync,
            &Resync {
                attachment_id: self.attachment_id,
            }
            .encode(),
        );
        match self.admit_frame(frame, OutboundKind::Resync) {
            Ok(()) => {
                self.resync_needed = false;
                self.flush_control_write()
            }
            Err(ClientError::ClientBackpressure) => Ok(()),
            Err(error) => Err(error),
        }
    }

    fn compact_buffer(&mut self) {
        if self.read_offset == 0 {
            return;
        }
        if self.read_offset >= self.buffered.len() {
            self.buffered.clear();
        } else {
            self.buffered.drain(..self.read_offset);
        }
        self.read_offset = 0;
    }
}

fn classify_server_error(
    error: ErrorMessage,
) -> Result<Option<InputAdmissionFailure>, ClientError> {
    if error.error_code == ErrorCode::Backpressure as u16
        && (error.offending_message_type == MessageType::Input as u16
            || error.offending_message_type == MessageType::TerminalKey as u16)
    {
        return Ok(Some(InputAdmissionFailure::ClientBackpressure));
    }
    Err(ClientError::Server(error.error_code))
}

struct RuntimeCells<'a>(&'a [DisplayCell]);

impl CellSource for RuntimeCells<'_> {
    fn len(&self) -> usize {
        self.0.len()
    }

    fn cell(&self, index: usize) -> Option<RenderCell> {
        self.0.get(index).copied().map(runtime_cell_to_render)
    }
}

fn prepare_cache(
    prepared: &mut PreparedSurface,
    cache: &DisplayCache,
    damage: RowDamage,
    full_invalidation: bool,
) -> Result<PreparationResult, ClientError> {
    let source = RuntimeCells(&cache.cells);
    prepared
        .prepare(
            CommittedDisplay {
                generation: cache.generation,
                rows: cache.rows,
                columns: cache.columns,
                cursor: CursorState::new(cache.cursor_row, cache.cursor_col, cache.cursor_visible),
                alternate_screen: cache.alternate_screen,
                cells: &source,
            },
            damage,
            full_invalidation,
        )
        .map_err(|_| ClientError::Prepare)
}

fn runtime_cell_to_render(cell: DisplayCell) -> RenderCell {
    RenderCell {
        scalar: cell.scalar,
        foreground: runtime_color_to_render(cell.foreground),
        background: runtime_color_to_render(cell.background),
        attributes: runtime_attributes_to_render(cell.attributes),
    }
}

fn runtime_color_to_render(color: DisplayColor) -> RenderColor {
    match color {
        DisplayColor::Default => RenderColor::Default,
        DisplayColor::Indexed(index) => RenderColor::Indexed(index),
        DisplayColor::Rgb { r, g, b } => RenderColor::Rgb { r, g, b },
    }
}

fn runtime_attributes_to_render(attributes: DisplayAttributes) -> RenderAttributes {
    RenderAttributes {
        bold: attributes.bold,
        underline: attributes.underline,
        inverse: attributes.inverse,
    }
}

fn connect_stream(path: &Path) -> Result<UnixStream, ClientError> {
    let stream = UnixStream::connect(path).map_err(|_| ClientError::Io)?;
    stream
        .set_read_timeout(Some(STARTUP_TIMEOUT))
        .map_err(|_| ClientError::Io)?;
    stream
        .set_write_timeout(Some(STARTUP_TIMEOUT))
        .map_err(|_| ClientError::Io)?;
    Ok(stream)
}

fn hello(
    stream: &mut UnixStream,
    interactive: bool,
    request_block_metadata: bool,
) -> Result<ServerHello, ClientError> {
    send_control(
        stream,
        MessageType::ClientHello,
        &ClientHello {
            client_capabilities: if request_block_metadata {
                CAP_BLOCK_METADATA
            } else {
                0
            },
        }
        .encode(),
    )?;
    let (kind, payload) = read_blocking_frame(stream)?;
    if kind == MessageType::Error {
        let error = ErrorMessage::decode(&payload).map_err(|_| ClientError::Protocol)?;
        return Err(ClientError::Server(error.error_code));
    }
    if kind != MessageType::ServerHello {
        return Err(ClientError::Protocol);
    }
    let hello = ServerHello::decode(&payload).map_err(|_| ClientError::Protocol)?;
    if hello.server_capabilities & CAP_BINARY_DISPLAY == 0 {
        return Err(ClientError::UnsupportedDisplayCapability);
    }
    if interactive
        && (hello.server_capabilities & CAP_SEMANTIC_TERMINAL_KEY == 0
            || hello.server_capabilities & CAP_CORRELATED_RESIZE == 0)
    {
        return Err(ClientError::UnsupportedInteractiveCapability);
    }
    Ok(hello)
}

fn send_control(
    stream: &mut UnixStream,
    message_type: MessageType,
    payload: &[u8],
) -> Result<(), ClientError> {
    stream
        .write_all(&encode_frame(message_type, payload))
        .map_err(|_| ClientError::Io)
}

fn read_blocking_frame(stream: &mut UnixStream) -> Result<(MessageType, Vec<u8>), ClientError> {
    let frame = read_blocking_raw_frame(stream)?;
    let header = FrameHeader::decode(&frame[..HEADER_LEN]).map_err(|_| ClientError::Protocol)?;
    let message_type = MessageType::from_u16(header.message_type).ok_or(ClientError::Protocol)?;
    Ok((message_type, frame[HEADER_LEN..].to_vec()))
}

fn read_blocking_raw_frame(stream: &mut UnixStream) -> Result<Vec<u8>, ClientError> {
    let mut header_bytes = [0u8; HEADER_LEN];
    stream
        .read_exact(&mut header_bytes)
        .map_err(|_| ClientError::Io)?;
    let header = FrameHeader::decode(&header_bytes).map_err(|_| ClientError::Protocol)?;
    let mut frame = Vec::with_capacity(HEADER_LEN + header.payload_len as usize);
    frame.extend_from_slice(&header_bytes);
    frame.resize(HEADER_LEN + header.payload_len as usize, 0);
    stream
        .read_exact(&mut frame[HEADER_LEN..])
        .map_err(|_| ClientError::Io)?;
    Ok(frame)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geometry(rows: u16, columns: u16) -> GridGeometry {
        GridGeometry { rows, columns }
    }

    fn display_cell() -> DisplayCell {
        DisplayCell {
            scalar: 'x',
            foreground: DisplayColor::Default,
            background: DisplayColor::Default,
            attributes: DisplayAttributes::default(),
        }
    }

    fn decoded_chunk(
        chunk_index: u16,
        chunk_count: u16,
        first_row: u16,
        row_count: u16,
    ) -> DecodedDisplayChunk {
        let columns = 1;
        DecodedDisplayChunk {
            kind: DisplayKind::Delta,
            generation: 2,
            base_generation: 1,
            rows: 4,
            columns,
            cursor_row: 0,
            cursor_col: 0,
            cursor_visible: true,
            alternate_screen: false,
            first_row,
            row_count,
            chunk_index,
            chunk_count,
            cells: vec![display_cell(); usize::from(row_count) * usize::from(columns)],
        }
    }

    #[test]
    fn runtime_cell_adapter_preserves_scalar_style_and_color_without_copying_cache() {
        let cells = [DisplayCell {
            scalar: 'Q',
            foreground: DisplayColor::Indexed(5),
            background: DisplayColor::Rgb { r: 1, g: 2, b: 3 },
            attributes: DisplayAttributes {
                bold: true,
                underline: true,
                inverse: false,
            },
        }];
        let source = RuntimeCells(&cells);
        let converted = source.cell(0).unwrap();
        assert_eq!(converted.scalar, 'Q');
        assert_eq!(converted.foreground, RenderColor::Indexed(5));
        assert_eq!(converted.background, RenderColor::Rgb { r: 1, g: 2, b: 3 });
        assert!(converted.attributes.bold);
        assert!(converted.attributes.underline);
    }

    #[test]
    fn pending_display_batch_rejects_impossible_chunk_count_before_allocation_growth() {
        let mut batch = PendingDisplayBatch::default();
        let mut chunk = decoded_chunk(0, 5, 0, 1);
        chunk.rows = 4;

        assert_eq!(batch.push(chunk), Err(ClientError::Capacity));
        assert!(batch.chunks().is_empty());
    }

    #[test]
    fn pending_display_batch_rejects_replayed_or_noncontiguous_rows() {
        let mut batch = PendingDisplayBatch::default();
        assert!(!batch.push(decoded_chunk(0, 2, 0, 1)).unwrap());

        let replayed = decoded_chunk(1, 2, 0, 1);
        assert_eq!(batch.push(replayed), Err(ClientError::Protocol));
        assert_eq!(batch.chunks().len(), 1);
    }

    #[test]
    fn finite_geometry_validation_rejects_each_nonfinite_operand_and_clamps() {
        let base = [800.0, 600.0, 20.0, 20.0, 10.0, 20.0];
        for index in 0..base.len() {
            for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                let mut values = base;
                values[index] = invalid;
                assert_eq!(
                    derive_grid_geometry(
                        values[0], values[1], values[2], values[3], values[4], values[5]
                    ),
                    None
                );
            }
        }

        for invalid in [
            [-1.0, 600.0, 20.0, 20.0, 10.0, 20.0],
            [800.0, -1.0, 20.0, 20.0, 10.0, 20.0],
            [800.0, 600.0, -1.0, 20.0, 10.0, 20.0],
            [800.0, 600.0, 20.0, -1.0, 10.0, 20.0],
            [800.0, 600.0, 20.0, 20.0, 0.0, 20.0],
            [800.0, 600.0, 20.0, 20.0, -1.0, 20.0],
            [800.0, 600.0, 20.0, 20.0, 10.0, 0.0],
            [800.0, 600.0, 20.0, 20.0, 10.0, -1.0],
        ] {
            assert_eq!(
                derive_grid_geometry(
                    invalid[0], invalid[1], invalid[2], invalid[3], invalid[4], invalid[5]
                ),
                None
            );
        }

        let smallest_positive = f64::from_bits(1);
        assert_eq!(
            derive_grid_geometry(1.0, 1.0, 0.0, 0.0, smallest_positive, 1.0),
            None
        );
        assert_eq!(
            derive_grid_geometry(1.0, 1.0, 0.0, 0.0, 1.0, smallest_positive),
            None
        );
        assert_eq!(
            derive_grid_geometry(0.1, 0.1, 0.0, 0.0, 10.0, 20.0),
            Some(GridGeometry {
                rows: 1,
                columns: 1
            })
        );
        assert_eq!(
            derive_grid_geometry(1.0e12, 1.0e12, 0.0, 0.0, 1.0, 1.0),
            Some(GridGeometry {
                rows: 256,
                columns: 512
            })
        );
    }

    #[test]
    fn newest_pending_resize_is_highest_request_id_across_result_and_unresolved_state() {
        let mut unresolved = VecDeque::from([ResizeRecord {
            request_id: 1,
            geometry: geometry(24, 80),
            phase: ResizePhase::SentWaitingResult,
        }]);
        let fence = AppliedFence {
            request_id: 2,
            geometry: geometry(30, 100),
            applied_generation: 9,
        };
        assert_eq!(
            newest_pending_geometry(&unresolved, Some(fence)),
            Some(geometry(30, 100))
        );
        unresolved.push_back(ResizeRecord {
            request_id: 3,
            geometry: geometry(40, 120),
            phase: ResizePhase::QueuedNotStarted,
        });
        assert_eq!(
            newest_pending_geometry(&unresolved, Some(fence)),
            Some(geometry(40, 120))
        );
    }

    #[test]
    fn resize_mutation_is_suppressed_for_committed_or_newest_pending_geometry() {
        let current = geometry(24, 80);
        let desired = geometry(30, 100);
        assert!(!resize_needs_mutation(current, current, None));
        assert!(!resize_needs_mutation(desired, current, Some(desired)));
        assert!(resize_needs_mutation(desired, current, None));
    }
}
