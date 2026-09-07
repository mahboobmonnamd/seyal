//! M001 Candidate-D scalar display decode (message types 12/13).

use super::{
    decode_bool, decode_color, validate_geometry, DecodedDisplayChunk, DisplayAttributes,
    DisplayCell, DisplayError, DisplayKind, DISPLAY_CELL_LEN, DISPLAY_CHUNK_HEADER_LEN,
};

pub fn decode_payload_v1(
    kind: DisplayKind,
    payload: &[u8],
) -> Result<DecodedDisplayChunk, DisplayError> {
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
        cells.push(decode_cell_v1(&payload[offset..offset + DISPLAY_CELL_LEN])?);
        offset += DISPLAY_CELL_LEN;
    }

    Ok(DecodedDisplayChunk {
        kind,
        schema: 1,
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
        first_col: 0,
        chunk_index,
        chunk_count,
        cells,
    })
}

fn decode_cell_v1(bytes: &[u8]) -> Result<DisplayCell, DisplayError> {
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
    let attributes = DisplayAttributes {
        bold: attr & 0b001 != 0,
        underline: attr & 0b010 != 0,
        inverse: attr & 0b100 != 0,
    };
    Ok(DisplayCell::lead_scalar(
        scalar, 1, foreground, background, attributes,
    ))
}
