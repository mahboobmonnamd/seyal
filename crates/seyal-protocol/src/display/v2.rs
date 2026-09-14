//! Grapheme display v2 decode/validation (SPEC-011 §§11.2–11.9).

use super::{
    decode_bool, decode_color, validate_geometry, DecodedDisplayChunk, DisplayAttributes,
    DisplayCell, DisplayColor, DisplayError, DisplayKind, DISPLAY_CELL_LEN,
};
use crate::framing::MAX_FRAME_PAYLOAD;
use std::sync::Arc;

pub const DISPLAY_CHUNK_HEADER_V2_LEN: usize = 48;
pub const DISPLAY_SCHEMA_V2: u16 = 2;
pub const MAX_GRAPHEME_SIDECAR_BYTES: usize = 65_536;
pub const MAX_GRAPHEME_UTF8_BYTES: usize = 8_192;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum DisplayCellRole {
    Empty = 0,
    Lead = 1,
    Continuation = 2,
}

#[derive(Clone, Copy)]
struct PendingContinuation {
    row: u16,
    foreground: DisplayColor,
    background: DisplayColor,
    attributes: DisplayAttributes,
}

impl DisplayCellRole {
    fn from_bits(bits: u32) -> Result<Self, DisplayError> {
        match bits {
            0 => Ok(Self::Empty),
            1 => Ok(Self::Lead),
            2 => Ok(Self::Continuation),
            _ => Err(DisplayError::InvalidRole),
        }
    }
}

pub fn validate_v2_span(
    first_col: u16,
    row_count: u16,
    cell_count: usize,
    columns: u16,
) -> Result<(), DisplayError> {
    if cell_count == 0 || row_count == 0 {
        return Err(DisplayError::InvalidChunk);
    }
    let columns_usize = columns as usize;
    let whole_row = first_col == 0
        && cell_count
            == (row_count as usize)
                .checked_mul(columns_usize)
                .ok_or(DisplayError::Overflow)?;
    let partial_row = row_count == 1
        && (first_col as usize)
            .checked_add(cell_count)
            .ok_or(DisplayError::Overflow)?
            <= columns_usize;
    if whole_row || partial_row {
        Ok(())
    } else {
        Err(DisplayError::InvalidChunk)
    }
}

pub fn encode_color(color: DisplayColor) -> u32 {
    match color {
        DisplayColor::Default => 0,
        DisplayColor::Indexed(index) => (0b01u32 << 30) | index as u32,
        DisplayColor::Rgb { r, g, b } => {
            (0b10u32 << 30) | ((r as u32) << 16) | ((g as u32) << 8) | b as u32
        }
    }
}

/// Packs v2 `meta` u32 (attributes + role/width/sidecar).
pub fn encode_v2_cell_meta(
    role: DisplayCellRole,
    width: u8,
    sidecar: bool,
    grapheme_len: u16,
    attributes: DisplayAttributes,
) -> u32 {
    let mut meta = 0u32;
    if attributes.bold {
        meta |= 1;
    }
    if attributes.underline {
        meta |= 1 << 1;
    }
    if attributes.inverse {
        meta |= 1 << 2;
    }
    meta |= (role as u32) << 3;
    meta |= (width as u32 & 0b11) << 5;
    if sidecar {
        meta |= 1 << 7;
        let len_minus_1 = grapheme_len.saturating_sub(1) as u32;
        meta |= (len_minus_1 & 0x1fff) << 8;
    }
    meta
}

pub fn decode_payload_v2(
    kind: DisplayKind,
    payload: &[u8],
) -> Result<DecodedDisplayChunk, DisplayError> {
    if payload.len() < DISPLAY_CHUNK_HEADER_V2_LEN {
        return Err(DisplayError::InvalidLength);
    }
    if payload.len() > MAX_FRAME_PAYLOAD as usize {
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
    let sidecar_len = u32::from_le_bytes(payload[40..44].try_into().unwrap()) as usize;
    let schema = u16::from_le_bytes(payload[44..46].try_into().unwrap());
    let first_col = u16::from_le_bytes(payload[46..48].try_into().unwrap());

    if schema != DISPLAY_SCHEMA_V2 {
        return Err(DisplayError::InvalidChunk);
    }
    validate_geometry(rows, columns, cursor_row, cursor_col)?;
    if kind == DisplayKind::Snapshot && base_generation != 0 {
        return Err(DisplayError::InvalidChunk);
    }
    if row_count == 0
        || chunk_count == 0
        || chunk_index >= chunk_count
        || first_row as u32 + row_count as u32 > rows as u32
        || first_col >= columns
        || cell_count == 0
    {
        return Err(DisplayError::InvalidChunk);
    }
    if sidecar_len > MAX_GRAPHEME_SIDECAR_BYTES {
        return Err(DisplayError::InvalidSidecar);
    }
    validate_v2_span(first_col, row_count, cell_count, columns)?;

    let cells_bytes = cell_count
        .checked_mul(DISPLAY_CELL_LEN)
        .ok_or(DisplayError::Overflow)?;
    let expected_len = DISPLAY_CHUNK_HEADER_V2_LEN
        .checked_add(cells_bytes)
        .and_then(|v| v.checked_add(sidecar_len))
        .ok_or(DisplayError::Overflow)?;
    if payload.len() != expected_len {
        return Err(DisplayError::InvalidLength);
    }

    let cells_start = DISPLAY_CHUNK_HEADER_V2_LEN;
    let sidecar_start = cells_start
        .checked_add(cells_bytes)
        .ok_or(DisplayError::Overflow)?;
    let sidecar = &payload[sidecar_start..sidecar_start + sidecar_len];

    let mut cells = Vec::with_capacity(cell_count);
    let mut expected_sidecar_off = 0usize;
    let mut pending_continuation = None;

    for index in 0..cell_count {
        let offset = cells_start + index * DISPLAY_CELL_LEN;
        let columns_usize = columns as usize;
        let (cell_row, cell_col) = if row_count == 1 {
            (
                first_row,
                first_col
                    .checked_add(index as u16)
                    .ok_or(DisplayError::Overflow)?,
            )
        } else {
            (
                first_row
                    .checked_add((index / columns_usize) as u16)
                    .ok_or(DisplayError::Overflow)?,
                (index % columns_usize) as u16,
            )
        };
        let cell = decode_cell_v2(
            &payload[offset..offset + DISPLAY_CELL_LEN],
            sidecar,
            &mut expected_sidecar_off,
            &mut pending_continuation,
            cell_row,
            cell_col,
            columns,
        )?;
        cells.push(cell);
    }
    if pending_continuation.is_some() || expected_sidecar_off != sidecar_len {
        return Err(DisplayError::InvalidSidecar);
    }

    Ok(DecodedDisplayChunk {
        kind,
        schema: DISPLAY_SCHEMA_V2,
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
        first_col,
        chunk_index,
        chunk_count,
        cells,
    })
}

fn decode_cell_v2(
    bytes: &[u8],
    sidecar: &[u8],
    expected_sidecar_off: &mut usize,
    pending_continuation: &mut Option<PendingContinuation>,
    cell_row: u16,
    cell_col: u16,
    columns: u16,
) -> Result<DisplayCell, DisplayError> {
    if bytes.len() != DISPLAY_CELL_LEN {
        return Err(DisplayError::InvalidCell);
    }
    let text_ref = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
    let foreground = decode_color(u32::from_le_bytes(bytes[4..8].try_into().unwrap()))?;
    let background = decode_color(u32::from_le_bytes(bytes[8..12].try_into().unwrap()))?;
    let meta = u32::from_le_bytes(bytes[12..16].try_into().unwrap());

    let attributes = DisplayAttributes {
        bold: meta & 1 != 0,
        underline: meta & (1 << 1) != 0,
        inverse: meta & (1 << 2) != 0,
    };
    let role = DisplayCellRole::from_bits((meta >> 3) & 0b11)?;
    let width = ((meta >> 5) & 0b11) as u8;
    if width == 3 {
        return Err(DisplayError::InvalidWidth);
    }
    let sidecar_flag = meta & (1 << 7) != 0;
    let len_minus_1 = (meta >> 8) & 0x1fff;
    let reserved = meta >> 21;
    if reserved != 0 {
        return Err(DisplayError::InvalidAttributes);
    }

    if let Some(expected) = pending_continuation {
        // Structural continuation rules stay fail-closed. Presentation
        // colors/attributes are not independent authority (SPEC-011 §5.3 /
        // §11.4): a reconnect snapshot may repeat the lead's style or the
        // terminal's default continuation style. Rejecting that mismatch as
        // InvalidCell drops the whole attach snapshot, so no frame reaches
        // Metal and the input bridge appears dead.
        if expected.row != cell_row {
            return Err(DisplayError::InvalidCell);
        }
        if role != DisplayCellRole::Continuation
            || width != 0
            || sidecar_flag
            || len_minus_1 != 0
            || text_ref != 0
        {
            return Err(DisplayError::InvalidCell);
        }
        let foreground = expected.foreground;
        let background = expected.background;
        let attributes = expected.attributes;
        *pending_continuation = None;
        return Ok(DisplayCell {
            scalar: ' ',
            role: DisplayCellRole::Continuation,
            width: 0,
            text: Arc::from([]),
            sidecar: false,
            foreground,
            background,
            attributes,
        });
    }

    match role {
        DisplayCellRole::Empty => {
            if text_ref != 0 || width != 0 || sidecar_flag || len_minus_1 != 0 {
                return Err(DisplayError::InvalidCell);
            }
            Ok(DisplayCell {
                scalar: ' ',
                role: DisplayCellRole::Empty,
                width: 0,
                text: Arc::from([]),
                sidecar: false,
                foreground,
                background,
                attributes,
            })
        }
        DisplayCellRole::Continuation => Err(DisplayError::InvalidCell),
        DisplayCellRole::Lead => {
            if width != 1 && width != 2 {
                return Err(DisplayError::InvalidWidth);
            }
            if width == 2 && cell_col + 1 >= columns {
                return Err(DisplayError::InvalidCell);
            }
            let text = if sidecar_flag {
                let len = (len_minus_1 as usize)
                    .checked_add(1)
                    .ok_or(DisplayError::Overflow)?;
                if len == 0 || len > MAX_GRAPHEME_UTF8_BYTES {
                    return Err(DisplayError::InvalidSidecar);
                }
                let start = text_ref as usize;
                if start != *expected_sidecar_off {
                    return Err(DisplayError::InvalidSidecar);
                }
                let end = start.checked_add(len).ok_or(DisplayError::Overflow)?;
                if end > sidecar.len() {
                    return Err(DisplayError::InvalidSidecar);
                }
                let bytes = &sidecar[start..end];
                if std::str::from_utf8(bytes).is_err() {
                    return Err(DisplayError::InvalidUnicode);
                }
                *expected_sidecar_off = end;
                let scalar = std::str::from_utf8(bytes)
                    .ok()
                    .and_then(|s| s.chars().next())
                    .ok_or(DisplayError::InvalidUnicode)?;
                (scalar, Arc::<[u8]>::from(bytes.to_vec()))
            } else {
                if len_minus_1 != 0 {
                    return Err(DisplayError::InvalidCell);
                }
                let scalar = char::from_u32(text_ref).ok_or(DisplayError::InvalidUnicode)?;
                let mut buf = [0u8; 4];
                let encoded = scalar.encode_utf8(&mut buf);
                (scalar, Arc::from(encoded.as_bytes().to_vec()))
            };
            if width == 2 {
                *pending_continuation = Some(PendingContinuation {
                    row: cell_row,
                    foreground,
                    background,
                    attributes,
                });
            }
            Ok(DisplayCell {
                scalar: text.0,
                role: DisplayCellRole::Lead,
                width,
                text: text.1,
                sidecar: sidecar_flag,
                foreground,
                background,
                attributes,
            })
        }
    }
}
