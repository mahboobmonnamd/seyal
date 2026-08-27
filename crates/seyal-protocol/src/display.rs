//! Candidate-D presentation-neutral display wire types, decoder and disposable client cache.
//!
//! This module owns protocol/value validation only. It owns no PTY, VT, canonical
//! terminal state, Runtime registry, renderer, or platform UI state.

use std::sync::Arc;

use crate::framing::{FrameHeader, HEADER_LEN, MessageType};

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
    pub fn message_type(self) -> MessageType {
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

    fn encoded_cell(scalar: char) -> [u8; DISPLAY_CELL_LEN] {
        let mut out = [0u8; DISPLAY_CELL_LEN];
        out[0..4].copy_from_slice(&(scalar as u32).to_le_bytes());
        out
    }

    #[test]
    fn one_row_snapshot_decodes_and_commits_atomically() {
        let mut payload = Vec::new();
        payload.extend_from_slice(&7u64.to_le_bytes());
        payload.extend_from_slice(&0u64.to_le_bytes());
        payload.extend_from_slice(&1u16.to_le_bytes());
        payload.extend_from_slice(&2u16.to_le_bytes());
        payload.extend_from_slice(&0u16.to_le_bytes());
        payload.extend_from_slice(&0u16.to_le_bytes());
        payload.extend_from_slice(&[1, 0, 0, 0]);
        payload.extend_from_slice(&0u16.to_le_bytes());
        payload.extend_from_slice(&1u16.to_le_bytes());
        payload.extend_from_slice(&0u16.to_le_bytes());
        payload.extend_from_slice(&1u16.to_le_bytes());
        payload.extend_from_slice(&2u32.to_le_bytes());
        payload.extend_from_slice(&encoded_cell('A'));
        payload.extend_from_slice(&encoded_cell('B'));
        let frame = crate::framing::encode_frame(MessageType::DisplaySnapshot, &payload);
        let chunk = decode_chunk(&frame).unwrap();
        let mut cache = empty_cache();
        cache.apply_chunks(&[chunk]).unwrap();
        assert_eq!(cache.generation, 7);
        assert_eq!((cache.rows, cache.columns), (1, 2));
        assert_eq!(cache.cells[0].scalar, 'A');
        assert_eq!(cache.cells[1].scalar, 'B');
    }
}
