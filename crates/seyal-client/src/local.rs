use std::{
    io::{Read, Write},
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
            Attach, Attached, CAP_BINARY_DISPLAY, ClientHello, ErrorMessage, ExecutionList,
            FrameHeader, HEADER_LEN, Lifecycle, MAX_FRAME_PAYLOAD, MessageType, Resync, Role,
            ServerHello, encode_frame,
        },
    },
};

const STARTUP_TIMEOUT: Duration = Duration::from_secs(2);
const READ_CHUNK_BYTES: usize = 64 * 1024;
const MAX_BUFFERED_BYTES: usize = (MAX_FRAME_PAYLOAD as usize + HEADER_LEN) * 2;
const MAX_FRAMES_PER_POLL: usize = 64;
const MAX_BYTES_PER_POLL: usize = 4 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientError {
    RuntimeDiscovery,
    Io,
    Protocol,
    UnsupportedDisplayCapability,
    NoRunningExecution,
    InvalidAttachment,
    Display,
    Prepare,
    Server(u16),
    Disconnected,
    Capacity,
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

#[derive(Debug)]
struct PendingControlWrite {
    bytes: Vec<u8>,
    offset: usize,
}

impl PendingControlWrite {
    fn new(bytes: Vec<u8>) -> Self {
        Self { bytes, offset: 0 }
    }

    fn remaining(&self) -> &[u8] {
        &self.bytes[self.offset..]
    }
}

pub struct LocalDisplayClient {
    stream: UnixStream,
    buffered: Vec<u8>,
    read_offset: usize,
    pending_batch: PendingDisplayBatch,
    pending_control: Option<PendingControlWrite>,
    attachment_id: AttachmentId,
    cache: DisplayCache,
    prepared: PreparedSurface,
    last_preparation: PreparationResult,
}

impl LocalDisplayClient {
    /// Connect to the verified per-user Runtime and attach as an observer to the
    /// first running execution. This is the M001 single-surface resolution seam;
    /// later workspace selection can call `connect_execution` with an explicit
    /// stable `ExecutionId` without changing renderer ownership.
    pub fn connect_first_running() -> Result<Self, ClientError> {
        let runtime_dir = darwin_user_runtime_dir().map_err(|_| ClientError::RuntimeDiscovery)?;
        ensure_verified_runtime_dir(&runtime_dir).map_err(|_| ClientError::RuntimeDiscovery)?;
        let socket_path =
            control_socket_path(&runtime_dir).map_err(|_| ClientError::RuntimeDiscovery)?;

        let mut stream = connect_stream(&socket_path)?;
        hello(&mut stream)?;
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

        Self::finish_attach(stream, execution_id, Role::Observer)
    }

    pub fn connect_execution(
        socket_path: &Path,
        execution_id: ExecutionId,
        role: Role,
    ) -> Result<Self, ClientError> {
        let mut stream = connect_stream(socket_path)?;
        hello(&mut stream)?;
        Self::finish_attach(stream, execution_id, role)
    }

    fn finish_attach(
        mut stream: UnixStream,
        execution_id: ExecutionId,
        role: Role,
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
        if cache.generation != attached.current_generation {
            return Err(ClientError::Protocol);
        }

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
            pending_control: None,
            attachment_id: attached.attachment_id,
            cache,
            prepared,
            last_preparation: result,
        })
    }

    pub fn socket_fd(&self) -> i32 {
        self.stream.as_raw_fd()
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
        self.pending_control.is_some()
    }

    /// Complete at most one nonblocking control write attempt. The AppKit bridge
    /// calls this only after the socket becomes writable, so temporary pressure
    /// cannot turn a generation-gap resync into a disconnect or a busy retry.
    pub fn flush_control_write(&mut self) -> Result<(), ClientError> {
        let stream = &mut self.stream;
        flush_control_with(&mut self.pending_control, |bytes| stream.write(bytes))
    }

    /// Drain a bounded amount of already-ready socket work, atomically commit
    /// complete Candidate-D batches, union their damage, then prepare only the
    /// latest committed state once. Incomplete multi-chunk updates never escape
    /// to the renderer.
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
                let message_type =
                    MessageType::from_u16(header.message_type).ok_or(ClientError::Protocol)?;

                match message_type {
                    MessageType::DisplaySnapshot | MessageType::DisplayDelta => {
                        let chunk = decode_chunk(frame).map_err(|_| ClientError::Display)?;
                        if self.accept_display_chunk(chunk, &mut damage, &mut full_invalidation)? {
                            committed_any = true;
                        }
                    }
                    MessageType::Error => {
                        let payload = &frame[HEADER_LEN..frame_end - frame_start];
                        let error =
                            ErrorMessage::decode(payload).map_err(|_| ClientError::Protocol)?;
                        return Err(ClientError::Server(error.error_code));
                    }
                    MessageType::Lifecycle => {
                        // The final display update precedes lifecycle finalization.
                        // Preserve the last committed pixels; later lifecycle UI is
                        // outside Pass 6.
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
                Ok(0) => return Err(ClientError::Disconnected),
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
        Ok(true)
    }

    fn request_resync(&mut self) -> Result<(), ClientError> {
        if self.pending_control.is_none() {
            self.pending_control = Some(PendingControlWrite::new(encode_frame(
                MessageType::Resync,
                &Resync {
                    attachment_id: self.attachment_id,
                }
                .encode(),
            )));
        }
        self.flush_control_write()
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

fn flush_control_with(
    pending: &mut Option<PendingControlWrite>,
    mut write_once: impl FnMut(&[u8]) -> std::io::Result<usize>,
) -> Result<(), ClientError> {
    let Some(control) = pending.as_mut() else {
        return Ok(());
    };
    let remaining = control.remaining();
    if remaining.is_empty() {
        *pending = None;
        return Ok(());
    }

    match write_once(remaining) {
        Ok(0) => Err(ClientError::Io),
        Ok(count) if count <= remaining.len() => {
            control.offset = control
                .offset
                .checked_add(count)
                .ok_or(ClientError::Capacity)?;
            if control.offset == control.bytes.len() {
                *pending = None;
            }
            Ok(())
        }
        Ok(_) => Err(ClientError::Io),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(()),
        Err(_) => Err(ClientError::Io),
    }
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

fn hello(stream: &mut UnixStream) -> Result<(), ClientError> {
    // SPEC-004 reserves ClientHello capability bits in M001. The client
    // advertises zero and verifies the server's binary-display capability in
    // ServerHello before proceeding.
    send_control(
        stream,
        MessageType::ClientHello,
        &ClientHello {
            client_capabilities: 0,
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
    Ok(())
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
    fn pending_control_write_survives_partial_write_and_would_block() {
        let mut pending = Some(PendingControlWrite::new(vec![1, 2, 3, 4]));

        flush_control_with(&mut pending, |_| Ok(2)).unwrap();
        assert_eq!(pending.as_ref().unwrap().remaining(), &[3, 4]);

        flush_control_with(&mut pending, |_| {
            Err(std::io::Error::from(std::io::ErrorKind::WouldBlock))
        })
        .unwrap();
        assert_eq!(pending.as_ref().unwrap().remaining(), &[3, 4]);

        flush_control_with(&mut pending, |bytes| Ok(bytes.len())).unwrap();
        assert!(pending.is_none());
    }
}
