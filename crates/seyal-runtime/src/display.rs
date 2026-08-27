//! Candidate-D display producer adapter owned by Runtime.
//!
//! Terminal authority remains in `TerminalExecution`. The versioned wire/value
//! schema, decoder and disposable client cache live in `seyal-protocol`; this
//! module only converts authoritative projection snapshots/updates into protocol
//! batches without introducing a second terminal state.

use std::sync::Arc;

#[cfg(feature = "benchmark-instrumentation")]
use std::sync::atomic::{AtomicU64, Ordering};

use seyal_exec::{
    ProjectionCell, ProjectionColor, TerminalProjectionSnapshot, TerminalProjectionUpdate,
};
#[cfg(test)]
use seyal_exec::ProjectionAttributes;
use seyal_protocol::framing::{self, MAX_FRAME_PAYLOAD};
#[cfg(test)]
use seyal_protocol::framing::HEADER_LEN;

pub use seyal_protocol::display::{
    DISPLAY_CELL_LEN, DISPLAY_CHUNK_HEADER_LEN, DecodedDisplayChunk, DisplayAttributes,
    DisplayCache, DisplayCell, DisplayColor, DisplayError, DisplayKind, EncodedDisplayBatch,
    MAX_DISPLAY_BATCH_BYTES, MAX_DISPLAY_CELLS, MAX_DISPLAY_COLUMNS, MAX_DISPLAY_ROWS,
    decode_chunk, empty_cache,
};

#[cfg(feature = "benchmark-instrumentation")]
static BENCH_SNAPSHOT_ENCODES: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "benchmark-instrumentation")]
static BENCH_DELTA_ENCODES: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "benchmark-instrumentation")]
static BENCH_SNAPSHOT_BYTES: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "benchmark-instrumentation")]
static BENCH_DELTA_BYTES: AtomicU64 = AtomicU64::new(0);

#[cfg(feature = "benchmark-instrumentation")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BenchmarkDisplayCounters {
    pub snapshot_encodes: u64,
    pub delta_encodes: u64,
    pub snapshot_bytes: u64,
    pub delta_bytes: u64,
}

#[cfg(feature = "benchmark-instrumentation")]
pub fn reset_benchmark_display_counters() {
    BENCH_SNAPSHOT_ENCODES.store(0, Ordering::Relaxed);
    BENCH_DELTA_ENCODES.store(0, Ordering::Relaxed);
    BENCH_SNAPSHOT_BYTES.store(0, Ordering::Relaxed);
    BENCH_DELTA_BYTES.store(0, Ordering::Relaxed);
}

#[cfg(feature = "benchmark-instrumentation")]
pub fn benchmark_display_counters() -> BenchmarkDisplayCounters {
    BenchmarkDisplayCounters {
        snapshot_encodes: BENCH_SNAPSHOT_ENCODES.load(Ordering::Relaxed),
        delta_encodes: BENCH_DELTA_ENCODES.load(Ordering::Relaxed),
        snapshot_bytes: BENCH_SNAPSHOT_BYTES.load(Ordering::Relaxed),
        delta_bytes: BENCH_DELTA_BYTES.load(Ordering::Relaxed),
    }
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
    let batch = encode_rows(
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
    )?;
    #[cfg(feature = "benchmark-instrumentation")]
    {
        BENCH_SNAPSHOT_ENCODES.fetch_add(1, Ordering::Relaxed);
        BENCH_SNAPSHOT_BYTES.fetch_add(batch.total_bytes as u64, Ordering::Relaxed);
    }
    Ok(batch)
}

pub fn encode_delta(
    update: &TerminalProjectionUpdate,
    base_generation: u64,
) -> Result<EncodedDisplayBatch, DisplayError> {
    validate_update(update)?;
    let first_row = update.damage.first_row;
    let row_count = update.damage.row_count();
    let batch = encode_rows(
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
    )?;
    #[cfg(feature = "benchmark-instrumentation")]
    {
        BENCH_DELTA_ENCODES.fetch_add(1, Ordering::Relaxed);
        BENCH_DELTA_BYTES.fetch_add(batch.total_bytes as u64, Ordering::Relaxed);
    }
    Ok(batch)
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
