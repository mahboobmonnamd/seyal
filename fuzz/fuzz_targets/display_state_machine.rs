#![no_main]

use libfuzzer_sys::fuzz_target;
use seyal_exec::{
    ProjectionAttributes, ProjectionCell, ProjectionColor, ProjectionDamage,
    TerminalProjectionSnapshot, TerminalProjectionUpdate,
};
use seyal_runtime::display::{
    DecodedDisplayChunk, DisplayCache, decode_chunk, empty_cache, encode_delta, encode_snapshot,
};

fn input_byte(data: &[u8], index: usize) -> u8 {
    data.get(index).copied().unwrap_or(0)
}

fn cell(seed: u8) -> ProjectionCell {
    ProjectionCell {
        scalar: char::from(b' ' + seed % 95),
        foreground: ProjectionColor::Default,
        background: ProjectionColor::Default,
        attributes: ProjectionAttributes {
            bold: seed & 0x01 != 0,
            underline: seed & 0x02 != 0,
            inverse: seed & 0x04 != 0,
        },
    }
}

fn cells(data: &[u8], offset: usize, count: usize) -> Vec<ProjectionCell> {
    (0..count)
        .map(|index| cell(input_byte(data, offset.wrapping_add(index))))
        .collect()
}

fn decode_batch(frames: &[std::sync::Arc<[u8]>]) -> Option<Vec<DecodedDisplayChunk>> {
    frames
        .iter()
        .map(|frame| decode_chunk(frame))
        .collect::<Result<Vec<_>, _>>()
        .ok()
}

fn assert_atomic_error(
    cache: &DisplayCache,
    before: &DisplayCache,
    result: Result<(), seyal_runtime::display::DisplayError>,
) {
    if result.is_err() {
        assert_eq!(
            cache, before,
            "rejected display batch partially mutated client cache"
        );
    }
}

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    let rows = 1 + (input_byte(data, 0) % 8) as u16;
    let columns = 1 + (input_byte(data, 1) % 32) as u16;
    let total_cells = rows as usize * columns as usize;
    let initial_generation = 1 + (input_byte(data, 2) % 8) as u64;
    let snapshot = TerminalProjectionSnapshot {
        rows,
        columns,
        cursor_row: (input_byte(data, 3) as u16) % rows,
        cursor_col: (input_byte(data, 4) as u16) % columns,
        cursor_visible: input_byte(data, 5) & 1 != 0,
        alternate_screen: input_byte(data, 5) & 2 != 0,
        source_damage_generation: initial_generation,
        damage: ProjectionDamage::full(rows),
        cells: cells(data, 6, total_cells),
    };

    let Ok(encoded_snapshot) = encode_snapshot(&snapshot) else {
        return;
    };
    let Some(snapshot_chunks) = decode_batch(&encoded_snapshot.frames) else {
        panic!("encoder produced a snapshot rejected by the production decoder");
    };
    let mut cache = empty_cache();
    cache
        .apply_chunks(&snapshot_chunks)
        .expect("encoder/decoder snapshot must apply to an empty cache");

    let mut cursor = 6usize.saturating_add(total_cells);
    let mut step = 0usize;
    while cursor < data.len() && step < 32 {
        let command = input_byte(data, cursor);
        cursor = cursor.saturating_add(1);

        let first_row = (input_byte(data, cursor) as u16) % rows;
        cursor = cursor.saturating_add(1);
        let available = rows - first_row;
        let row_count = 1 + (input_byte(data, cursor) as u16) % available;
        cursor = cursor.saturating_add(1);
        let last_row = first_row + row_count - 1;
        let update_cell_count = row_count as usize * columns as usize;
        let generation = cache.generation.saturating_add(1);
        let update = TerminalProjectionUpdate {
            rows,
            columns,
            cursor_row: (input_byte(data, cursor) as u16) % rows,
            cursor_col: (input_byte(data, cursor.saturating_add(1)) as u16) % columns,
            cursor_visible: command & 0x10 != 0,
            alternate_screen: command & 0x20 != 0,
            source_damage_generation: generation,
            damage: ProjectionDamage {
                full: first_row == 0 && row_count == rows,
                first_row,
                last_row,
            },
            cells: cells(data, cursor.saturating_add(2), update_cell_count),
        };
        cursor = cursor.saturating_add(2).saturating_add(update_cell_count);

        let base_generation = if command & 0x01 == 0 {
            cache.generation
        } else {
            cache.generation.saturating_add(7)
        };
        let Ok(encoded) = encode_delta(&update, base_generation) else {
            break;
        };
        let Some(mut chunks) = decode_batch(&encoded.frames) else {
            panic!("encoder produced a delta rejected by the production decoder");
        };

        match (command >> 1) & 0x03 {
            0 => {}
            1 => {
                if chunks.len() > 1 {
                    chunks.pop();
                } else if let Some(first) = chunks.first_mut() {
                    first.chunk_count = first.chunk_count.saturating_add(1);
                }
            }
            2 => chunks.reverse(),
            3 => {
                if let Some(first) = chunks.first_mut() {
                    first.base_generation = first.base_generation.saturating_add(1);
                }
            }
            _ => unreachable!(),
        }

        let before = cache.clone();
        let result = cache.apply_chunks(&chunks);
        assert_atomic_error(&cache, &before, result);
        step += 1;
    }
});
