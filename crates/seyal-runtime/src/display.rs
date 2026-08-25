//! Candidate-D presentation-neutral display transport.
//!
//! Terminal authority remains in `TerminalExecution`; this module only converts
//! owned projection-neutral snapshots/updates into immutable binary UDS frames
//! and provides a disposable client-cache decoder for protocol consumers.

use std::sync::Arc;

use seyal_exec::{
    ProjectionAttributes, ProjectionCell, ProjectionColor, TerminalProjectionSnapshot,
    TerminalProjectionUpdate,
};

use crate::local_ipc::framing::{self, FrameHeader, HEADER_LEN, MAX_FRAME_PAYLOAD, MessageType};

pub const DISPLAY_CHUNK_HEADER_LEN: usize = 40;
pub const DISPLAY_CELL_LEN: usize = 16;
pub const MAX_DISPLAY_ROWS: u16 = 256;
pub const MAX_DISPLAY_COLUMNS: u16 = 512;
pub const MAX_DISPLAY_CELLS: usize = 131_072;
pub const MAX_DISPLAY_BATCH_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayKind {
    Snapshot,
    Delta,
}

impl DisplayKind {
    fn message_type(self) -> MessageType {
        match self {
            Self::Snapshot => MessageType::DisplaySnapshot,
            Self::Delta => MessageType::DisplayDelta,
        }
    }

    fn from_message_type(message_type: MessageType) -> Result<Self, DisplayError> {
        match message_type {
            MessageType::DisplaySnapshot => Ok(Self::Snapshot),
            MessageType::DisplayDelta => Ok(Self::Delta),
            _ => Err(DisplayError::WrongMessageType),
        }
    }
}

#[derive(Clone, Debug)]
pub struct EncodedDisplayBatch {
    pub kind: DisplayKind,
    pub generation: u64,
    pub base_generation: u64,
    pub rows: u16,
    pub columns: u16,
    pub frames: Vec<Arc<[u8]>>,
    pub total_bytes: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayError {
    InvalidGeometry,
    InvalidCursor,
    InvalidDamage,
    InvalidChunk,
    InvalidLength,
    InvalidCell,
    InvalidColor,
    InvalidAttributes,
    InvalidUnicode,
    WrongMessageType,
    GenerationMismatch,
    DimensionMismatch,
    IncompleteBatch,
    BatchTooLarge,
    Overflow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayColor {
    Default,
    Indexed(u8),
    Rgb { r: u8, g: u8, b: u8 },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DisplayAttributes {
    pub bold: bool,
    pub underline: bool,
    pub inverse: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisplayCell {
    pub scalar: char,
    pub foreground: DisplayColor,
    pub background: DisplayColor,
    pub attributes: DisplayAttributes,
}

impl From<ProjectionCell> for DisplayCell {
    fn from(cell: ProjectionCell) -> Self {
        Self {
            scalar: cell.scalar,
            foreground: convert_color(cell.foreground),
            background: convert_color(cell.background),
            attributes: convert_attributes(cell.attributes),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DisplayCache {
    pub generation: u64,
    pub rows: u16,
    pub columns: u16,
    pub cursor_row: u16,
    pub cursor_col: u16,
    pub cursor_visible: bool,
    pub alternate_screen: bool,
    pub cells: Vec<DisplayCell>,
}

#[derive(Clone, Debug)]
pub struct DecodedDisplayChunk {
    pub kind: DisplayKind,
    pub generation: u64,
    pub base_generation: u64,
    pub rows: u16,
    pub columns: u16,
    pub cursor_row: u16,
    pub cursor_col: u16,
    pub cursor_visible: bool,
    pub alternate_screen: bool,
    pub first_row: u16,
    pub row_count: u16,
    pub chunk_index: u16,
    pub chunk_count: u16,
    pub cells: Vec<DisplayCell>,
}

#[derive(Clone, Copy)]
struct DisplayMeta {
    generation: u64,
    rows: u16,
    columns: u16,
    cursor_row: u16,
    cursor_col: u16,
    cursor_visible: bool,
    alternate_screen: bool,
}

pub fn encode_snapshot(
    snapshot: &TerminalProjectionSnapshot,
) -> Result<EncodedDisplayBatch, DisplayError> {
    validate_snapshot(snapshot)?;
    encode_rows(
        DisplayMeta {
            generation: snapshot.source_damage_generation,
            rows: snapshot.rows,
            columns: snapshot.columns,
            cursor_row: snapshot.cursor_row,
            cursor_col: snapshot.cursor_col,
            cursor_visible: snapshot.cursor_visible,
            alternate_screen: snapshot.alternate_screen,
        },
        DisplayKind::Snapshot,
        0,
        0,
        snapshot.rows,
        &snapshot.cells,
    )
}

pub fn encode_delta(
    update: &TerminalProjectionUpdate,
    base_generation: u64,
) -> Result<EncodedDisplayBatch, DisplayError> {
    validate_update(update)?;
    let first_row = update.damage.first_row;
    let row_count = update.damage.row_count();
    encode_rows(
        DisplayMeta {
            generation: update.source_damage_generation,
            rows: update.rows,
            columns: update.columns,
            cursor_row: update.cursor_row,
            cursor_col: update.cursor_col,
            cursor_visible: update.cursor_visible,
            alternate_screen: update.alternate_screen,
        },
        DisplayKind::Delta,
        base_generation,
        first_row,
        row_count,
        &update.cells,
    )
}

fn encode_rows(
    meta: DisplayMeta,
    kind: DisplayKind,
    base_generation: u64,
    first_row: u16,
    row_count: u16,
    cells: &[ProjectionCell],
) -> Result<EncodedDisplayBatch, DisplayError> {
    validate_geometry(meta.rows, meta.columns, meta.cursor_row, meta.cursor_col)?;
    if row_count == 0
        || first_row >= meta.rows
        || first_row as u32 + row_count as u32 > meta.rows as u32
    {
        return Err(DisplayError::InvalidDamage);
    }
    let expected_cells = (row_count as usize)
        .checked_mul(meta.columns as usize)
        .ok_or(DisplayError::Overflow)?;
    if cells.len() != expected_cells {
        return Err(DisplayError::InvalidDamage);
    }

    let bytes_per_row = (meta.columns as usize)
        .checked_mul(DISPLAY_CELL_LEN)
        .ok_or(DisplayError::Overflow)?;
    let available = (MAX_FRAME_PAYLOAD as usize)
        .checked_sub(DISPLAY_CHUNK_HEADER_LEN)
        .ok_or(DisplayError::Overflow)?;
    let rows_per_chunk = available / bytes_per_row;
    if rows_per_chunk == 0 {
        return Err(DisplayError::InvalidGeometry);
    }
    let chunk_count = (row_count as usize).div_ceil(rows_per_chunk);
    let chunk_count_u16 = u16::try_from(chunk_count).map_err(|_| DisplayError::Overflow)?;
    let mut frames = Vec::with_capacity(chunk_count);
    let mut total_bytes = 0usize;
    let mut emitted_rows = 0usize;

    for chunk_index in 0..chunk_count {
        let chunk_rows = rows_per_chunk.min(row_count as usize - emitted_rows);
        let chunk_first_row = first_row as usize + emitted_rows;
        let cell_count = chunk_rows
            .checked_mul(meta.columns as usize)
            .ok_or(DisplayError::Overflow)?;
        let payload_len = DISPLAY_CHUNK_HEADER_LEN
            .checked_add(
                cell_count
                    .checked_mul(DISPLAY_CELL_LEN)
                    .ok_or(DisplayError::Overflow)?,
            )
            .ok_or(DisplayError::Overflow)?;
        if payload_len > MAX_FRAME_PAYLOAD as usize {
            return Err(DisplayError::InvalidLength);
        }

        let mut payload = Vec::with_capacity(payload_len);
        payload.extend_from_slice(&meta.generation.to_le_bytes());
        payload.extend_from_slice(&base_generation.to_le_bytes());
        payload.extend_from_slice(&meta.rows.to_le_bytes());
        payload.extend_from_slice(&meta.columns.to_le_bytes());
        payload.extend_from_slice(&meta.cursor_row.to_le_bytes());
        payload.extend_from_slice(&meta.cursor_col.to_le_bytes());
        payload.push(meta.cursor_visible as u8);
        payload.push(meta.alternate_screen as u8);
        payload.push(0);
        payload.push(0);
        payload.extend_from_slice(&(chunk_first_row as u16).to_le_bytes());
        payload.extend_from_slice(&(chunk_rows as u16).to_le_bytes());
        payload.extend_from_slice(&(chunk_index as u16).to_le_bytes());
        payload.extend_from_slice(&chunk_count_u16.to_le_bytes());
        payload.extend_from_slice(&(cell_count as u32).to_le_bytes());
        debug_assert_eq!(payload.len(), DISPLAY_CHUNK_HEADER_LEN);

        let source_first = emitted_rows
            .checked_mul(meta.columns as usize)
            .ok_or(DisplayError::Overflow)?;
        let source_last = source_first
            .checked_add(cell_count)
            .ok_or(DisplayError::Overflow)?;
        for cell in &cells[source_first..source_last] {
            encode_projection_cell(*cell, &mut payload);
        }
        debug_assert_eq!(payload.len(), payload_len);

        let frame = framing::encode_frame(kind.message_type(), &payload);
        total_bytes = total_bytes
            .checked_add(frame.len())
            .ok_or(DisplayError::Overflow)?;
        if total_bytes > MAX_DISPLAY_BATCH_BYTES {
            return Err(DisplayError::BatchTooLarge);
        }
        frames.push(Arc::<[u8]>::from(frame));
        emitted_rows += chunk_rows;
    }

    Ok(EncodedDisplayBatch {
        kind,
        generation: meta.generation,
        base_generation,
        rows: meta.rows,
        columns: meta.columns,
        frames,
        total_bytes,
    })
}

pub fn decode_chunk(frame: &[u8]) -> Result<DecodedDisplayChunk, DisplayError> {
    if frame.len() < HEADER_LEN {
        return Err(DisplayError::InvalidLength);
    }
    let header =
        FrameHeader::decode(&frame[..HEADER_LEN]).map_err(|_| DisplayError::InvalidLength)?;
    let expected = HEADER_LEN
        .checked_add(header.payload_len as usize)
        .ok_or(DisplayError::Overflow)?;
    if frame.len() != expected {
        return Err(DisplayError::InvalidLength);
    }
    let message_type =
        MessageType::from_u16(header.message_type).ok_or(DisplayError::WrongMessageType)?;
    let kind = DisplayKind::from_message_type(message_type)?;
    decode_payload(kind, &frame[HEADER_LEN..])
}

fn decode_payload(kind: DisplayKind, payload: &[u8]) -> Result<DecodedDisplayChunk, DisplayError> {
    if payload.len() < DISPLAY_CHUNK_HEADER_LEN {
        return Err(DisplayError::InvalidLength);
    }
    let generation = u64::from_le_bytes(payload[0..8].try_into().unwrap());
    let base_generation = u64::from_le_bytes(payload[8..16].try_into().unwrap());
    let rows = u16::from_le_bytes(payload[16..18].try_into().unwrap());
    let columns = u16::from_le_bytes(payload[18..20].try_into().unwrap());
    let cursor_row = u16::from_le_bytes(payload[20..22].try_into().unwrap());
    let cursor_col = u16::from_le_bytes(payload[22..24].try_into().unwrap());
    let cursor_visible = decode_bool(payload[24])?;
    let alternate_screen = decode_bool(payload[25])?;
    if payload[26] != 0 || payload[27] != 0 {
        return Err(DisplayError::InvalidChunk);
    }
    let first_row = u16::from_le_bytes(payload[28..30].try_into().unwrap());
    let row_count = u16::from_le_bytes(payload[30..32].try_into().unwrap());
    let chunk_index = u16::from_le_bytes(payload[32..34].try_into().unwrap());
    let chunk_count = u16::from_le_bytes(payload[34..36].try_into().unwrap());
    let cell_count = u32::from_le_bytes(payload[36..40].try_into().unwrap()) as usize;

    validate_geometry(rows, columns, cursor_row, cursor_col)?;
    if kind == DisplayKind::Snapshot && base_generation != 0 {
        return Err(DisplayError::InvalidChunk);
    }
    if row_count == 0
        || chunk_count == 0
        || chunk_index >= chunk_count
        || first_row as u32 + row_count as u32 > rows as u32
    {
        return Err(DisplayError::InvalidChunk);
    }
    let expected_cells = (row_count as usize)
        .checked_mul(columns as usize)
        .ok_or(DisplayError::Overflow)?;
    if cell_count != expected_cells {
        return Err(DisplayError::InvalidChunk);
    }
    let expected_len = DISPLAY_CHUNK_HEADER_LEN
        .checked_add(
            cell_count
                .checked_mul(DISPLAY_CELL_LEN)
                .ok_or(DisplayError::Overflow)?,
        )
        .ok_or(DisplayError::Overflow)?;
    if payload.len() != expected_len {
        return Err(DisplayError::InvalidLength);
    }

    let mut cells = Vec::with_capacity(cell_count);
    let mut offset = DISPLAY_CHUNK_HEADER_LEN;
    for _ in 0..cell_count {
        cells.push(decode_cell(&payload[offset..offset + DISPLAY_CELL_LEN])?);
        offset += DISPLAY_CELL_LEN;
    }

    Ok(DecodedDisplayChunk {
        kind,
        generation,
        base_generation,
        rows,
        columns,
        cursor_row,
        cursor_col,
        cursor_visible,
        alternate_screen,
        first_row,
        row_count,
        chunk_index,
        chunk_count,
        cells,
    })
}

impl DisplayCache {
    pub fn apply_batch(&mut self, batch: &EncodedDisplayBatch) -> Result<(), DisplayError> {
        let chunks = batch
            .frames
            .iter()
            .map(|frame| decode_chunk(frame))
            .collect::<Result<Vec<_>, _>>()?;
        self.apply_chunks(&chunks)
    }

    pub fn apply_chunks(&mut self, chunks: &[DecodedDisplayChunk]) -> Result<(), DisplayError> {
        let first = chunks.first().ok_or(DisplayError::IncompleteBatch)?;
        if chunks.len() != first.chunk_count as usize {
            return Err(DisplayError::IncompleteBatch);
        }
        for (index, chunk) in chunks.iter().enumerate() {
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
                || chunk.chunk_index as usize != index
            {
                return Err(DisplayError::InvalidChunk);
            }
        }

        let mut expected_row = if first.kind == DisplayKind::Snapshot {
            0
        } else {
            first.first_row
        };
        for chunk in chunks {
            if chunk.first_row != expected_row {
                return Err(DisplayError::InvalidChunk);
            }
            expected_row = expected_row
                .checked_add(chunk.row_count)
                .ok_or(DisplayError::Overflow)?;
        }
        if first.kind == DisplayKind::Snapshot && expected_row != first.rows {
            return Err(DisplayError::IncompleteBatch);
        }

        match first.kind {
            DisplayKind::Snapshot => {
                let expected_cells = (first.rows as usize)
                    .checked_mul(first.columns as usize)
                    .ok_or(DisplayError::Overflow)?;
                let mut cells = Vec::with_capacity(expected_cells);
                for chunk in chunks {
                    cells.extend_from_slice(&chunk.cells);
                }
                if cells.len() != expected_cells {
                    return Err(DisplayError::IncompleteBatch);
                }
                self.cells = cells;
                self.rows = first.rows;
                self.columns = first.columns;
            }
            DisplayKind::Delta => {
                if self.generation != first.base_generation {
                    return Err(DisplayError::GenerationMismatch);
                }
                if self.rows != first.rows || self.columns != first.columns {
                    return Err(DisplayError::DimensionMismatch);
                }
                for chunk in chunks {
                    let first_cell = (chunk.first_row as usize)
                        .checked_mul(self.columns as usize)
                        .ok_or(DisplayError::Overflow)?;
                    let last_cell = first_cell
                        .checked_add(chunk.cells.len())
                        .ok_or(DisplayError::Overflow)?;
                    if last_cell > self.cells.len() {
                        return Err(DisplayError::InvalidChunk);
                    }
                    self.cells[first_cell..last_cell].copy_from_slice(&chunk.cells);
                }
            }
        }

        self.generation = first.generation;
        self.cursor_row = first.cursor_row;
        self.cursor_col = first.cursor_col;
        self.cursor_visible = first.cursor_visible;
        self.alternate_screen = first.alternate_screen;
        Ok(())
    }
}

pub fn empty_cache() -> DisplayCache {
    DisplayCache {
        generation: 0,
        rows: 0,
        columns: 0,
        cursor_row: 0,
        cursor_col: 0,
        cursor_visible: false,
        alternate_screen: false,
        cells: Vec::new(),
    }
}

fn validate_snapshot(snapshot: &TerminalProjectionSnapshot) -> Result<(), DisplayError> {
    validate_geometry(
        snapshot.rows,
        snapshot.columns,
        snapshot.cursor_row,
        snapshot.cursor_col,
    )?;
    let expected = (snapshot.rows as usize)
        .checked_mul(snapshot.columns as usize)
        .ok_or(DisplayError::Overflow)?;
    if expected > MAX_DISPLAY_CELLS || snapshot.cells.len() != expected {
        return Err(DisplayError::InvalidGeometry);
    }
    Ok(())
}

fn validate_update(update: &TerminalProjectionUpdate) -> Result<(), DisplayError> {
    validate_geometry(
        update.rows,
        update.columns,
        update.cursor_row,
        update.cursor_col,
    )?;
    if update.damage.first_row > update.damage.last_row || update.damage.last_row >= update.rows {
        return Err(DisplayError::InvalidDamage);
    }
    if update.damage.full
        && (update.damage.first_row != 0 || update.damage.last_row != update.rows.saturating_sub(1))
    {
        return Err(DisplayError::InvalidDamage);
    }
    let expected = (update.damage.row_count() as usize)
        .checked_mul(update.columns as usize)
        .ok_or(DisplayError::Overflow)?;
    if expected > MAX_DISPLAY_CELLS || update.cells.len() != expected {
        return Err(DisplayError::InvalidDamage);
    }
    Ok(())
}

fn validate_geometry(
    rows: u16,
    columns: u16,
    cursor_row: u16,
    cursor_col: u16,
) -> Result<(), DisplayError> {
    if rows == 0
        || columns == 0
        || rows > MAX_DISPLAY_ROWS
        || columns > MAX_DISPLAY_COLUMNS
        || cursor_row >= rows
        || cursor_col >= columns
    {
        return Err(DisplayError::InvalidGeometry);
    }
    let cells = (rows as usize)
        .checked_mul(columns as usize)
        .ok_or(DisplayError::Overflow)?;
    if cells > MAX_DISPLAY_CELLS {
        return Err(DisplayError::InvalidGeometry);
    }
    Ok(())
}

fn convert_color(color: ProjectionColor) -> DisplayColor {
    match color {
        ProjectionColor::Default => DisplayColor::Default,
        ProjectionColor::Indexed(index) => DisplayColor::Indexed(index),
        ProjectionColor::Rgb { r, g, b } => DisplayColor::Rgb { r, g, b },
    }
}

fn convert_attributes(attributes: ProjectionAttributes) -> DisplayAttributes {
    DisplayAttributes {
        bold: attributes.bold,
        underline: attributes.underline,
        inverse: attributes.inverse,
    }
}

fn encode_projection_cell(cell: ProjectionCell, out: &mut Vec<u8>) {
    out.extend_from_slice(&(cell.scalar as u32).to_le_bytes());
    out.extend_from_slice(&encode_color(cell.foreground).to_le_bytes());
    out.extend_from_slice(&encode_color(cell.background).to_le_bytes());
    let attributes = (cell.attributes.bold as u16)
        | ((cell.attributes.underline as u16) << 1)
        | ((cell.attributes.inverse as u16) << 2);
    out.extend_from_slice(&attributes.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
}

fn encode_color(color: ProjectionColor) -> u32 {
    match color {
        ProjectionColor::Default => 0,
        ProjectionColor::Indexed(index) => (0b01u32 << 30) | index as u32,
        ProjectionColor::Rgb { r, g, b } => {
            (0b10u32 << 30) | ((r as u32) << 16) | ((g as u32) << 8) | b as u32
        }
    }
}

fn decode_cell(bytes: &[u8]) -> Result<DisplayCell, DisplayError> {
    if bytes.len() != DISPLAY_CELL_LEN {
        return Err(DisplayError::InvalidCell);
    }
    let scalar = char::from_u32(u32::from_le_bytes(bytes[0..4].try_into().unwrap()))
        .ok_or(DisplayError::InvalidUnicode)?;
    let foreground = decode_color(u32::from_le_bytes(bytes[4..8].try_into().unwrap()))?;
    let background = decode_color(u32::from_le_bytes(bytes[8..12].try_into().unwrap()))?;
    let attr = u16::from_le_bytes(bytes[12..14].try_into().unwrap());
    if attr & !0b111 != 0 || bytes[14..16] != [0, 0] {
        return Err(DisplayError::InvalidAttributes);
    }
    Ok(DisplayCell {
        scalar,
        foreground,
        background,
        attributes: DisplayAttributes {
            bold: attr & 0b001 != 0,
            underline: attr & 0b010 != 0,
            inverse: attr & 0b100 != 0,
        },
    })
}

fn decode_color(value: u32) -> Result<DisplayColor, DisplayError> {
    let kind = value >> 30;
    let payload = value & 0x3fff_ffff;
    match kind {
        0b00 if payload == 0 => Ok(DisplayColor::Default),
        0b01 if payload <= 0xff => Ok(DisplayColor::Indexed(payload as u8)),
        0b10 if payload <= 0x00ff_ffff => Ok(DisplayColor::Rgb {
            r: ((payload >> 16) & 0xff) as u8,
            g: ((payload >> 8) & 0xff) as u8,
            b: (payload & 0xff) as u8,
        }),
        _ => Err(DisplayError::InvalidColor),
    }
}

fn decode_bool(value: u8) -> Result<bool, DisplayError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(DisplayError::InvalidChunk),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use seyal_exec::{ProjectionDamage, TerminalProjectionUpdate};

    fn cell(index: usize) -> ProjectionCell {
        ProjectionCell {
            scalar: char::from_u32(b'a' as u32 + (index % 26) as u32).unwrap(),
            foreground: ProjectionColor::Default,
            background: ProjectionColor::Default,
            attributes: ProjectionAttributes::default(),
        }
    }

    fn sample_snapshot(rows: u16, columns: u16, generation: u64) -> TerminalProjectionSnapshot {
        TerminalProjectionSnapshot {
            rows,
            columns,
            cursor_row: 0,
            cursor_col: 0,
            cursor_visible: true,
            alternate_screen: false,
            source_damage_generation: generation,
            damage: ProjectionDamage::full(rows),
            cells: (0..rows as usize * columns as usize).map(cell).collect(),
        }
    }

    fn sample_update(
        rows: u16,
        columns: u16,
        generation: u64,
        damage: ProjectionDamage,
    ) -> TerminalProjectionUpdate {
        TerminalProjectionUpdate {
            rows,
            columns,
            cursor_row: 0,
            cursor_col: 0,
            cursor_visible: true,
            alternate_screen: false,
            source_damage_generation: generation,
            damage,
            cells: (0..damage.row_count() as usize * columns as usize)
                .map(cell)
                .collect(),
        }
    }

    #[test]
    fn snapshot_round_trip_rebuilds_disposable_cache() {
        let snapshot = sample_snapshot(24, 80, 7);
        let batch = encode_snapshot(&snapshot).unwrap();
        let mut cache = empty_cache();
        cache.apply_batch(&batch).unwrap();
        assert_eq!(cache.generation, 7);
        assert_eq!((cache.rows, cache.columns), (24, 80));
        assert_eq!(cache.cells.len(), 24 * 80);
    }

    #[test]
    fn large_snapshot_is_chunked_under_frame_limit() {
        let snapshot = sample_snapshot(256, 512, 8);
        let batch = encode_snapshot(&snapshot).unwrap();
        assert!(batch.frames.len() > 1);
        assert!(
            batch
                .frames
                .iter()
                .all(|frame| frame.len() <= HEADER_LEN + MAX_FRAME_PAYLOAD as usize)
        );
        let mut cache = empty_cache();
        cache.apply_batch(&batch).unwrap();
        assert_eq!(cache.cells.len(), 131_072);
    }

    #[test]
    fn delta_carries_only_projection_update_cells() {
        let initial = sample_snapshot(24, 80, 10);
        let mut cache = empty_cache();
        cache
            .apply_batch(&encode_snapshot(&initial).unwrap())
            .unwrap();

        let damage = ProjectionDamage {
            full: false,
            first_row: 10,
            last_row: 11,
        };
        let mut update = sample_update(24, 80, 11, damage);
        update.cells[0].scalar = 'Z';
        assert_eq!(update.cells.len(), 160);
        let delta = encode_delta(&update, 10).unwrap();
        let decoded = decode_chunk(&delta.frames[0]).unwrap();
        assert_eq!((decoded.first_row, decoded.row_count), (10, 2));
        cache.apply_batch(&delta).unwrap();
        assert_eq!(cache.generation, 11);
        assert_eq!(cache.cells[10 * 80].scalar, 'Z');
    }

    #[test]
    fn delta_generation_gap_is_rejected_without_partial_commit() {
        let snapshot = sample_snapshot(24, 80, 3);
        let mut cache = empty_cache();
        cache
            .apply_batch(&encode_snapshot(&snapshot).unwrap())
            .unwrap();
        let update = sample_update(
            24,
            80,
            5,
            ProjectionDamage {
                full: false,
                first_row: 0,
                last_row: 0,
            },
        );
        let delta = encode_delta(&update, 4).unwrap();
        let before = cache.clone();
        assert_eq!(
            cache.apply_batch(&delta),
            Err(DisplayError::GenerationMismatch)
        );
        assert_eq!(cache, before);
    }

    #[test]
    fn incomplete_chunked_snapshot_does_not_mutate_cache() {
        let snapshot = sample_snapshot(256, 512, 9);
        let batch = encode_snapshot(&snapshot).unwrap();
        let chunks = batch
            .frames
            .iter()
            .map(|frame| decode_chunk(frame))
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let mut cache = empty_cache();
        let before = cache.clone();
        assert_eq!(
            cache.apply_chunks(&chunks[..chunks.len() - 1]),
            Err(DisplayError::IncompleteBatch)
        );
        assert_eq!(cache, before);
    }

    #[test]
    fn incomplete_multi_chunk_delta_does_not_partially_mutate_cache() {
        let snapshot = sample_snapshot(256, 512, 20);
        let mut cache = empty_cache();
        cache
            .apply_batch(&encode_snapshot(&snapshot).unwrap())
            .unwrap();
        let mut update = sample_update(256, 512, 21, ProjectionDamage::full(256));
        update.cells[0].scalar = 'Z';
        let batch = encode_delta(&update, 20).unwrap();
        assert!(batch.frames.len() > 1);
        let chunks = batch
            .frames
            .iter()
            .map(|frame| decode_chunk(frame))
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let before = cache.clone();
        assert_eq!(
            cache.apply_chunks(&chunks[..chunks.len() - 1]),
            Err(DisplayError::IncompleteBatch)
        );
        assert_eq!(cache, before);
        cache.apply_chunks(&chunks).unwrap();
        assert_eq!(cache.cells[0].scalar, 'Z');
    }
}
