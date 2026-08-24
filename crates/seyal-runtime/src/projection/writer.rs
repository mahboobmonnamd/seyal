//! SPEC-004 section 10 generation publication and race safety.
//!
//! The Runtime is the sole writer. This module implements the exact
//! writer/reader protocol from the specification: an odd/even atomic slot
//! sequence marks a slot as being-written versus finalized, and an atomic
//! region publication word names the currently committed `(generation,
//! slot)` pair. Readers must reject any slot observed mid-write or whose
//! sequence changes across the read.
//!
//! All raw-pointer/atomic access to shared memory is confined to this
//! module. [`RegionMemory`] is deliberately backend-agnostic (a real
//! `mmap`-backed region in production, a plain heap buffer in tests) so the
//! race protocol itself can be exercised without any platform IPC.

use std::sync::atomic::{AtomicU64, Ordering};

use crate::projection::layout::{
    CELL_LEN, CellRecord, DAMAGE_LEN, DamageRecord, LayoutError, ModeFlags, PUBLICATION_WORD_OFFSET,
    RegionHeader, SLOT_HEADER_LEN, SLOT_SEQUENCE_OFFSET, SlotHeader,
};

/// A raw, backend-agnostic view over projection region memory.
///
/// # Safety invariants (upheld by every constructor of this type)
///
/// - `ptr` is valid for reads and writes of `len` bytes for the entire
///   lifetime during which any copy of this handle is used.
/// - `ptr` is aligned to at least 8 bytes, so every 8-byte-aligned offset
///   used by this module (the region publication word and each slot
///   sequence word) can be soundly reinterpreted as an `AtomicU64`.
/// - Only the functions in this module dereference `ptr`; no normal Rust
///   reference to the pointee is ever created while a `RegionMemory` value
///   for it exists, because a concurrent writer may mutate the bytes at any
///   time (that is the entire point of the race-safe protocol below).
#[derive(Clone, Copy)]
pub struct RegionMemory {
    ptr: *mut u8,
    len: usize,
}

// SAFETY: `RegionMemory` is a thin, `Copy` handle over memory that is only
// ever touched through the bounds-checked, offset-validated accessors in
// this module (plain byte copies or `AtomicU64` operations). It carries no
// aliasing assumption beyond "the pointee is at least `len` bytes and
// 8-byte aligned for the lifetime of use", which is required of every
// constructor regardless of which thread calls it.
unsafe impl Send for RegionMemory {}
// SAFETY: see the `Send` impl; all shared access goes through atomics or
// through callers that already serialize non-atomic writes (the writer is
// the only non-atomic writer and never overlaps with a reader in time for
// the same bytes it is currently mutating, by construction of the protocol).
unsafe impl Sync for RegionMemory {}

impl RegionMemory {
    /// # Safety
    ///
    /// The caller must uphold the invariants documented on [`RegionMemory`]:
    /// `ptr` valid and 8-byte aligned for `len` bytes for as long as any
    /// value derived from this handle is used.
    pub unsafe fn new(ptr: *mut u8, len: usize) -> Self {
        debug_assert_eq!(ptr as usize % 8, 0, "region memory must be 8-byte aligned");
        Self { ptr, len }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Reads a snapshot of the raw bytes in `range` into a caller-owned
    /// copy. Used only for plain (non-atomic) header/payload bytes that the
    /// writer alone mutates outside the atomic words.
    pub fn read_bytes(&self, range: std::ops::Range<usize>) -> Result<Vec<u8>, WriterError> {
        if range.end > self.len {
            return Err(WriterError::OutOfBounds);
        }
        // SAFETY: `range.end <= self.len` was just checked, and `self.ptr`
        // is valid for `self.len` bytes per the type invariant. This reads
        // plain bytes (header/payload regions), never the atomic words,
        // which are always accessed exclusively through `atomic_word_at`.
        let slice = unsafe { std::slice::from_raw_parts(self.ptr, self.len) };
        Ok(slice[range].to_vec())
    }

    fn write_bytes(&self, offset: usize, bytes: &[u8]) -> Result<(), WriterError> {
        let end = offset
            .checked_add(bytes.len())
            .ok_or(WriterError::OutOfBounds)?;
        if end > self.len {
            return Err(WriterError::OutOfBounds);
        }
        // SAFETY: bounds checked above; only the sole Runtime writer calls
        // this, and only for the non-committed slot / not-yet-published
        // header bytes, never concurrently with a reader observing the same
        // bytes as finalized (enforced by the odd/even sequence protocol).
        let slice = unsafe { std::slice::from_raw_parts_mut(self.ptr, self.len) };
        slice[offset..end].copy_from_slice(bytes);
        Ok(())
    }

    /// Atomically loads the 8-byte word at `offset` with `ordering`.
    ///
    /// # Panics
    /// Panics (via `debug_assert`) in debug builds if `offset` is not
    /// 8-byte aligned or would read past the end of the region; both are
    /// programmer errors in this module's own offset arithmetic, never
    /// attacker-controlled input.
    fn atomic_load(&self, offset: usize, ordering: Ordering) -> u64 {
        debug_assert_eq!(offset % 8, 0);
        debug_assert!(offset + 8 <= self.len);
        // SAFETY: offset is 8-byte aligned and in-bounds (checked above),
        // and the type invariant guarantees `ptr` is valid/aligned for the
        // full region. Every read/write of this exact word anywhere in this
        // module goes through `AtomicU64`, so there is no data race with
        // plain reads.
        let word: &AtomicU64 = unsafe { AtomicU64::from_ptr(self.ptr.add(offset).cast()) };
        word.load(ordering)
    }

    fn atomic_store(&self, offset: usize, value: u64, ordering: Ordering) {
        debug_assert_eq!(offset % 8, 0);
        debug_assert!(offset + 8 <= self.len);
        // SAFETY: see `atomic_load`.
        let word: &AtomicU64 = unsafe { AtomicU64::from_ptr(self.ptr.add(offset).cast()) };
        word.store(value, ordering);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriterError {
    OutOfBounds,
    InvalidInput(LayoutError),
    GenerationExhausted,
}

impl From<LayoutError> for WriterError {
    fn from(value: LayoutError) -> Self {
        Self::InvalidInput(value)
    }
}

/// Snapshot payload the Runtime wants to publish as the next generation.
pub struct SnapshotWrite<'a> {
    pub rows: u16,
    pub columns: u16,
    pub cursor_row: u16,
    pub cursor_col: u16,
    pub cursor_visible: bool,
    pub cursor_style: u8,
    pub mode_flags: ModeFlags,
    pub cells: &'a [CellRecord],
    pub damages: &'a [DamageRecord],
    pub full_snapshot: bool,
    pub source_damage_generation: u64,
}

/// The sole Runtime-owned writer for one projection region.
pub struct Writer {
    memory: RegionMemory,
    region: RegionHeader,
    next_generation: u64,
    committed_slot: Option<u8>,
}

const MAX_GENERATION: u64 = (1u64 << 63) - 1;

impl Writer {
    /// Takes ownership of a freshly created, already-header-initialized
    /// region and resets both slot sequences to the unpublished/finalized
    /// state (`0`, even) so a reader observing generation `0` correctly
    /// treats the region as not yet readable.
    pub fn new(memory: RegionMemory, region: RegionHeader) -> Result<Self, WriterError> {
        let slot0 = region.slot_offset(0)?;
        let slot1 = region.slot_offset(1)?;
        memory.atomic_store(slot0 as usize + SLOT_SEQUENCE_OFFSET, 0, Ordering::Release);
        memory.atomic_store(slot1 as usize + SLOT_SEQUENCE_OFFSET, 0, Ordering::Release);
        memory.atomic_store(PUBLICATION_WORD_OFFSET, 0, Ordering::Release);
        Ok(Self {
            memory,
            region,
            next_generation: 1,
            committed_slot: None,
        })
    }

    pub fn committed_generation(&self) -> u64 {
        self.next_generation.saturating_sub(1)
    }

    /// Publishes `snapshot` as the next generation, returning its number.
    ///
    /// Implements SPEC-004 section 10.1 exactly: the non-committed slot is
    /// marked odd/writing, fully written, then marked even/finalized before
    /// the region publication word is advanced. No reader acknowledgement
    /// is required or waited for.
    pub fn publish(&mut self, snapshot: &SnapshotWrite<'_>) -> Result<u64, WriterError> {
        validate_snapshot(&self.region, snapshot)?;
        if self.next_generation > MAX_GENERATION {
            return Err(WriterError::GenerationExhausted);
        }

        let slot = match self.committed_slot {
            None => 0u8,
            Some(0) => 1u8,
            Some(_) => 0u8,
        };
        let generation = self.next_generation;
        let slot_offset = self.region.slot_offset(slot)? as usize;

        // Step 1/2: choose the non-committed slot, mark it odd (writing).
        self.memory.atomic_store(
            slot_offset + SLOT_SEQUENCE_OFFSET,
            2 * generation + 1,
            Ordering::Release,
        );

        // Step 3/4/5: write the complete slot header, cells and damage.
        let cells_offset = SLOT_HEADER_LEN as u32;
        let cells_len = snapshot.cells.len() * CELL_LEN;
        let damages_offset = cells_offset as usize + cells_len;
        let header = SlotHeader {
            generation,
            payload_bytes: (cells_len + snapshot.damages.len() * DAMAGE_LEN) as u32,
            rows: snapshot.rows,
            columns: snapshot.columns,
            cursor_row: snapshot.cursor_row,
            cursor_col: snapshot.cursor_col,
            cursor_visible: snapshot.cursor_visible,
            cursor_style: snapshot.cursor_style,
            mode_flags: snapshot.mode_flags,
            cell_count: snapshot.cells.len() as u32,
            damage_count: snapshot.damages.len() as u16,
            full_snapshot: snapshot.full_snapshot,
            cells_offset,
            damages_offset: damages_offset as u32,
            source_damage_generation: snapshot.source_damage_generation,
        };
        let mut header_bytes = [0u8; SLOT_HEADER_LEN];
        header.encode(&mut header_bytes)?;
        self.memory.write_bytes(slot_offset, &header_bytes)?;

        let mut cell_bytes = vec![0u8; cells_len];
        for (index, cell) in snapshot.cells.iter().enumerate() {
            cell.encode(&mut cell_bytes[index * CELL_LEN..(index + 1) * CELL_LEN])?;
        }
        self.memory
            .write_bytes(slot_offset + cells_offset as usize, &cell_bytes)?;

        let mut damage_bytes = vec![0u8; snapshot.damages.len() * DAMAGE_LEN];
        for (index, damage) in snapshot.damages.iter().enumerate() {
            damage.encode(&mut damage_bytes[index * DAMAGE_LEN..(index + 1) * DAMAGE_LEN])?;
        }
        self.memory
            .write_bytes(slot_offset + damages_offset, &damage_bytes)?;

        // Step 6: mark the slot even/finalized.
        self.memory.atomic_store(
            slot_offset + SLOT_SEQUENCE_OFFSET,
            2 * generation,
            Ordering::Release,
        );
        // Step 7: publish the committed (generation, slot) pair.
        self.memory.atomic_store(
            PUBLICATION_WORD_OFFSET,
            (generation << 1) | slot as u64,
            Ordering::Release,
        );

        self.committed_slot = Some(slot);
        self.next_generation += 1;
        Ok(generation)
    }
}

fn validate_snapshot(region: &RegionHeader, snapshot: &SnapshotWrite<'_>) -> Result<(), WriterError> {
    if snapshot.rows > region.capacity_rows || snapshot.columns > region.capacity_cols {
        return Err(WriterError::InvalidInput(LayoutError::InvalidRowsColumns));
    }
    let expected_cells = snapshot.rows as usize * snapshot.columns as usize;
    if snapshot.cells.len() != expected_cells {
        return Err(WriterError::InvalidInput(LayoutError::InvalidCellCount));
    }
    if snapshot.damages.len() > snapshot.rows as usize {
        return Err(WriterError::InvalidInput(LayoutError::InvalidDamageCount));
    }
    if !snapshot.full_snapshot {
        return Err(WriterError::InvalidInput(LayoutError::InvalidSnapshotFlags));
    }
    let cells_len = expected_cells * CELL_LEN;
    let damages_len = snapshot.damages.len() * DAMAGE_LEN;
    let slot_payload_end = SLOT_HEADER_LEN + cells_len + damages_len;
    if slot_payload_end > region.slot_stride as usize {
        return Err(WriterError::InvalidInput(LayoutError::InvalidOffsets));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReaderError {
    NoReadableGeneration,
    TornReadExceededRetries,
    Layout(LayoutError),
    OutOfBounds,
}

impl From<LayoutError> for ReaderError {
    fn from(value: LayoutError) -> Self {
        Self::Layout(value)
    }
}

impl From<WriterError> for ReaderError {
    fn from(value: WriterError) -> Self {
        match value {
            WriterError::OutOfBounds => Self::OutOfBounds,
            WriterError::InvalidInput(layout) => Self::Layout(layout),
            WriterError::GenerationExhausted => Self::OutOfBounds,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotRead {
    pub generation: u64,
    pub header: SlotHeader,
    pub cells: Vec<CellRecord>,
    pub damages: Vec<DamageRecord>,
}

const MAX_READ_RETRIES: u32 = 64;

/// Implements SPEC-004 section 10.2 exactly: a reader never renders a slot
/// observed with an odd or changed sequence, and retries (bounded) instead
/// of blocking or requiring writer cooperation.
pub fn read_latest(memory: &RegionMemory, region: &RegionHeader) -> Result<SnapshotRead, ReaderError> {
    for _ in 0..MAX_READ_RETRIES {
        // Step 1: acquire-load region.publication.
        let publication = memory.atomic_load(PUBLICATION_WORD_OFFSET, Ordering::Acquire);
        let generation = publication >> 1;
        let slot = (publication & 1) as u8;
        // Step 2: reject generation 0 (no readable snapshot yet).
        if generation == 0 {
            return Err(ReaderError::NoReadableGeneration);
        }
        let slot_offset = region.slot_offset(slot)? as usize;

        // Step 3: acquire-load slot[s].sequence.
        let sequence_before =
            memory.atomic_load(slot_offset + SLOT_SEQUENCE_OFFSET, Ordering::Acquire);
        // Step 4: reject/retry if odd (being written) or mismatched.
        if !sequence_before.is_multiple_of(2) || sequence_before != 2 * generation {
            continue;
        }

        // Step 5: validate fixed header fields, offsets and counts, then
        // read the slot bytes for rendering.
        let header_bytes = memory.read_bytes(slot_offset..slot_offset + SLOT_HEADER_LEN)?;
        let header = SlotHeader::decode(&header_bytes, region.capacity_rows, region.capacity_cols)?;
        let (cells_range, damages_range) = header.cell_and_damage_ranges(region.slot_stride)?;
        let cell_bytes =
            memory.read_bytes(slot_offset + cells_range.start..slot_offset + cells_range.end)?;
        let damage_bytes = memory
            .read_bytes(slot_offset + damages_range.start..slot_offset + damages_range.end)?;

        let mut cells = Vec::with_capacity(header.cell_count as usize);
        let (cell_chunks, _) = cell_bytes.as_chunks::<CELL_LEN>();
        for chunk in cell_chunks {
            cells.push(CellRecord::decode(chunk)?);
        }
        let mut damages = Vec::with_capacity(header.damage_count as usize);
        let (damage_chunks, _) = damage_bytes.as_chunks::<DAMAGE_LEN>();
        for chunk in damage_chunks {
            damages.push(DamageRecord::decode(chunk, header.rows)?);
        }

        // Step 7/8: re-check the sequence and publication identify the same
        // generation/slot; otherwise this read raced a rollover and must be
        // discarded/retried.
        let sequence_after =
            memory.atomic_load(slot_offset + SLOT_SEQUENCE_OFFSET, Ordering::Acquire);
        let publication_after = memory.atomic_load(PUBLICATION_WORD_OFFSET, Ordering::Acquire);
        if sequence_after == sequence_before && publication_after == publication {
            return Ok(SnapshotRead {
                generation,
                header,
                cells,
                damages,
            });
        }
    }
    Err(ReaderError::TornReadExceededRetries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projection::layout::{REGION_HEADER_LEN, WireAttributes, WireColor};

    /// Allocates an 8-byte-aligned buffer of at least `bytes` bytes backed
    /// by a `Vec<u64>`, matching the alignment guarantee real `mmap`
    /// regions always provide.
    fn aligned_region(bytes: usize) -> (Box<[u64]>, RegionMemory) {
        let words = bytes.div_ceil(8);
        let mut storage = vec![0u64; words].into_boxed_slice();
        let ptr = storage.as_mut_ptr().cast::<u8>();
        // SAFETY: `storage` outlives `memory` for the scope of every test
        // below (it is returned alongside and kept alive by the caller),
        // is 8-byte aligned (`Vec<u64>`), and is exactly `words * 8 >=
        // bytes` long.
        let memory = unsafe { RegionMemory::new(ptr, words * 8) };
        (storage, memory)
    }

    fn small_region_header(capacity_rows: u16, capacity_cols: u16, slot_stride: u64) -> RegionHeader {
        RegionHeader {
            region_bytes: REGION_HEADER_LEN as u64 + 2 * slot_stride,
            execution_id: 1,
            attachment_id: 2,
            projection_id: 3,
            slot_stride,
            slot0_offset: REGION_HEADER_LEN as u64,
            capacity_rows,
            capacity_cols,
        }
    }

    fn blank_cells(rows: u16, columns: u16) -> Vec<CellRecord> {
        vec![
            CellRecord {
                scalar: ' ',
                foreground: WireColor::Default,
                background: WireColor::Default,
                attributes: WireAttributes::default(),
            };
            rows as usize * columns as usize
        ]
    }

    #[test]
    fn reader_rejects_generation_zero_before_first_publish() {
        let region = small_region_header(4, 8, 4096);
        let (_storage, memory) = aligned_region(region.region_bytes as usize);
        let mut header_bytes = [0u8; REGION_HEADER_LEN];
        region.encode(&mut header_bytes).unwrap();
        memory.write_bytes(0, &header_bytes).unwrap();
        let _writer = Writer::new(memory, region).unwrap();

        assert_eq!(
            read_latest(&memory, &region),
            Err(ReaderError::NoReadableGeneration)
        );
    }

    #[test]
    fn writer_publish_then_reader_read_round_trips_a_full_snapshot() {
        let region = small_region_header(2, 3, 4096);
        let (_storage, memory) = aligned_region(region.region_bytes as usize);
        let mut header_bytes = [0u8; REGION_HEADER_LEN];
        region.encode(&mut header_bytes).unwrap();
        memory.write_bytes(0, &header_bytes).unwrap();
        let mut writer = Writer::new(memory, region).unwrap();

        let cells = blank_cells(2, 3);
        let damages = vec![DamageRecord {
            first_row: 0,
            last_row: 1,
            full: true,
        }];
        let snapshot = SnapshotWrite {
            rows: 2,
            columns: 3,
            cursor_row: 0,
            cursor_col: 0,
            cursor_visible: true,
            cursor_style: 0,
            mode_flags: ModeFlags::default(),
            cells: &cells,
            damages: &damages,
            full_snapshot: true,
            source_damage_generation: 1,
        };
        let generation = writer.publish(&snapshot).unwrap();
        assert_eq!(generation, 1);

        let read = read_latest(&memory, &region).unwrap();
        assert_eq!(read.generation, 1);
        assert_eq!(read.header.rows, 2);
        assert_eq!(read.header.columns, 3);
        assert_eq!(read.cells, cells);
        assert_eq!(read.damages, damages);
    }

    #[test]
    fn writer_alternates_slots_and_generations_are_monotonic() {
        let region = small_region_header(1, 1, 4096);
        let (_storage, memory) = aligned_region(region.region_bytes as usize);
        let mut header_bytes = [0u8; REGION_HEADER_LEN];
        region.encode(&mut header_bytes).unwrap();
        memory.write_bytes(0, &header_bytes).unwrap();
        let mut writer = Writer::new(memory, region).unwrap();

        let cells = blank_cells(1, 1);
        let mut generations = Vec::new();
        for _ in 0..5 {
            let snapshot = SnapshotWrite {
                rows: 1,
                columns: 1,
                cursor_row: 0,
                cursor_col: 0,
                cursor_visible: false,
                cursor_style: 0,
                mode_flags: ModeFlags::default(),
                cells: &cells,
                damages: &[],
                full_snapshot: true,
                source_damage_generation: 0,
            };
            generations.push(writer.publish(&snapshot).unwrap());
        }
        assert_eq!(generations, vec![1, 2, 3, 4, 5]);
        assert_eq!(writer.committed_generation(), 5);
        let read = read_latest(&memory, &region).unwrap();
        assert_eq!(read.generation, 5);
    }

    #[test]
    fn publish_rejects_cell_count_mismatch_before_touching_memory() {
        let region = small_region_header(2, 2, 4096);
        let (_storage, memory) = aligned_region(region.region_bytes as usize);
        let mut writer = Writer::new(memory, region).unwrap();
        let cells = blank_cells(2, 2);
        let snapshot = SnapshotWrite {
            rows: 2,
            columns: 2,
            cursor_row: 0,
            cursor_col: 0,
            cursor_visible: false,
            cursor_style: 0,
            mode_flags: ModeFlags::default(),
            cells: &cells[..3], // one short
            damages: &[],
            full_snapshot: true,
            source_damage_generation: 0,
        };
        assert_eq!(
            writer.publish(&snapshot),
            Err(WriterError::InvalidInput(LayoutError::InvalidCellCount))
        );
        // A rejected publish must not have advanced the generation counter
        // or left a torn/odd sequence behind.
        assert_eq!(writer.committed_generation(), 0);
        assert_eq!(
            read_latest(&memory, &region),
            Err(ReaderError::NoReadableGeneration)
        );
    }

    #[test]
    fn aggressive_concurrent_writer_and_reader_never_observe_a_torn_generation() {
        use std::sync::atomic::AtomicBool;
        use std::sync::Arc;

        let region = small_region_header(3, 5, 4096);
        let (storage, memory) = aligned_region(region.region_bytes as usize);
        let mut header_bytes = [0u8; REGION_HEADER_LEN];
        region.encode(&mut header_bytes).unwrap();
        memory.write_bytes(0, &header_bytes).unwrap();
        let mut writer = Writer::new(memory, region).unwrap();

        const TOTAL_GENERATIONS: u64 = 20_000;
        let stop = Arc::new(AtomicBool::new(false));

        let reader_region = region;
        let reader_memory = memory;
        let reader_stop = Arc::clone(&stop);
        let reader = std::thread::spawn(move || {
            let mut observed_max = 0u64;
            let mut torn = 0u64;
            while !reader_stop.load(Ordering::Relaxed) {
                match read_latest(&reader_memory, &reader_region) {
                    Ok(snapshot) => {
                        assert!(
                            snapshot.generation >= observed_max,
                            "generations must never move backward: saw {} after {}",
                            snapshot.generation,
                            observed_max
                        );
                        assert_eq!(snapshot.cells.len(), snapshot.header.cell_count as usize);
                        observed_max = snapshot.generation;
                    }
                    Err(ReaderError::NoReadableGeneration) => {}
                    Err(ReaderError::TornReadExceededRetries) => {
                        torn += 1;
                    }
                    Err(other) => panic!("unexpected reader error: {other:?}"),
                }
            }
            (observed_max, torn)
        });

        let cells = blank_cells(3, 5);
        for generation in 1..=TOTAL_GENERATIONS {
            let snapshot = SnapshotWrite {
                rows: 3,
                columns: 5,
                cursor_row: (generation % 3) as u16,
                cursor_col: (generation % 5) as u16,
                cursor_visible: generation % 2 == 0,
                cursor_style: 0,
                mode_flags: ModeFlags::default(),
                cells: &cells,
                damages: &[],
                full_snapshot: true,
                source_damage_generation: generation,
            };
            let published = writer.publish(&snapshot).unwrap();
            assert_eq!(published, generation);
        }
        stop.store(true, Ordering::Relaxed);
        let (observed_max, torn_read_count) = reader.join().unwrap();
        assert!(observed_max <= TOTAL_GENERATIONS);
        // A bounded number of retried-away torn reads is expected under
        // contention; the invariant under test is that none is ever
        // returned as if it were valid (checked above), not that retries
        // never happen.
        let _ = torn_read_count;
        drop(storage);
    }
}
