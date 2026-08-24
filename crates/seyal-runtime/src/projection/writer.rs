//! SPEC-004 section 10 generation publication and race safety.
//!
//! The Runtime is the sole writer. Shared bytes are never represented by
//! ordinary Rust references while another thread/process may mutate them.
//! Atomic sequence/publication words use release/acquire ordering, while the
//! fixed-width payload is copied through aligned `AtomicU64` relaxed loads and
//! stores. The surrounding sequence protocol detects a generation rollover.

use std::sync::atomic::{AtomicU64, Ordering};

use crate::projection::layout::{
    CELL_LEN, CellRecord, DAMAGE_LEN, DamageRecord, LayoutError, ModeFlags,
    PUBLICATION_WORD_OFFSET, REGION_HEADER_LEN, RegionHeader, SLOT_HEADER_LEN,
    SLOT_SEQUENCE_OFFSET, SlotHeader,
};

#[derive(Clone, Copy)]
pub struct RegionMemory {
    ptr: *mut u8,
    len: usize,
}

// SAFETY: every concurrently accessible word is read/written through
// `AtomicU64`; no Rust slice/reference to mapped payload storage is created.
unsafe impl Send for RegionMemory {}
// SAFETY: same as `Send`. The sole-writer rule is maintained by `Writer` and
// readers only perform atomic loads into private buffers.
unsafe impl Sync for RegionMemory {}

impl RegionMemory {
    /// # Safety
    /// `ptr` must remain valid and at least 8-byte aligned for `len` bytes for
    /// the lifetime of every copied `RegionMemory` handle.
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

    fn checked_word_range(&self, offset: usize, len: usize) -> Result<(), WriterError> {
        if !offset.is_multiple_of(8) || !len.is_multiple_of(8) {
            return Err(WriterError::UnalignedAccess);
        }
        let end = offset.checked_add(len).ok_or(WriterError::OutOfBounds)?;
        if end > self.len {
            return Err(WriterError::OutOfBounds);
        }
        Ok(())
    }

    fn read_atomic_bytes(&self, offset: usize, len: usize) -> Result<Vec<u8>, WriterError> {
        self.checked_word_range(offset, len)?;
        let mut out = vec![0u8; len];
        for word_index in 0..(len / 8) {
            let word = self.atomic_load(offset + word_index * 8, Ordering::Relaxed);
            out[word_index * 8..(word_index + 1) * 8].copy_from_slice(&word.to_le_bytes());
        }
        Ok(out)
    }

    fn write_atomic_bytes(&self, offset: usize, bytes: &[u8]) -> Result<(), WriterError> {
        self.checked_word_range(offset, bytes.len())?;
        for (word_index, chunk) in bytes.chunks_exact(8).enumerate() {
            let word = u64::from_le_bytes(chunk.try_into().expect("exact 8-byte chunk"));
            self.atomic_store(offset + word_index * 8, word, Ordering::Relaxed);
        }
        Ok(())
    }

    fn atomic_load(&self, offset: usize, ordering: Ordering) -> u64 {
        debug_assert_eq!(offset % 8, 0);
        debug_assert!(offset + 8 <= self.len);
        // SAFETY: constructor and bounds/alignment checks guarantee a live,
        // aligned word. Every concurrent access to this word is atomic.
        let word: &AtomicU64 = unsafe { AtomicU64::from_ptr(self.ptr.add(offset).cast()) };
        word.load(ordering)
    }

    fn atomic_store(&self, offset: usize, value: u64, ordering: Ordering) {
        debug_assert_eq!(offset % 8, 0);
        debug_assert!(offset + 8 <= self.len);
        // SAFETY: same invariants as `atomic_load`.
        let word: &AtomicU64 = unsafe { AtomicU64::from_ptr(self.ptr.add(offset).cast()) };
        word.store(value, ordering);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriterError {
    OutOfBounds,
    UnalignedAccess,
    InvalidInput(LayoutError),
    GenerationExhausted,
}

impl From<LayoutError> for WriterError {
    fn from(value: LayoutError) -> Self {
        Self::InvalidInput(value)
    }
}

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

pub struct Writer {
    memory: RegionMemory,
    region: RegionHeader,
    next_generation: u64,
    committed_slot: Option<u8>,
}

const MAX_GENERATION: u64 = (1u64 << 63) - 1;

impl Writer {
    pub fn new(memory: RegionMemory, region: RegionHeader) -> Result<Self, WriterError> {
        let slot0 = region.slot_offset(0)? as usize;
        let slot1 = region.slot_offset(1)? as usize;
        memory.atomic_store(slot0 + SLOT_SEQUENCE_OFFSET, 0, Ordering::Release);
        memory.atomic_store(slot1 + SLOT_SEQUENCE_OFFSET, 0, Ordering::Release);
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

        // Mark the non-committed slot as being written. This word is never
        // subsequently touched by the static-header copy below.
        self.memory.atomic_store(
            slot_offset + SLOT_SEQUENCE_OFFSET,
            2 * generation + 1,
            Ordering::Release,
        );

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
        self.memory
            .write_atomic_bytes(slot_offset + 8, &header_bytes[8..])?;

        let mut cell_bytes = vec![0u8; cells_len];
        for (index, cell) in snapshot.cells.iter().enumerate() {
            cell.encode(&mut cell_bytes[index * CELL_LEN..(index + 1) * CELL_LEN])?;
        }
        self.memory
            .write_atomic_bytes(slot_offset + cells_offset as usize, &cell_bytes)?;

        let mut damage_bytes = vec![0u8; snapshot.damages.len() * DAMAGE_LEN];
        for (index, damage) in snapshot.damages.iter().enumerate() {
            damage.encode(&mut damage_bytes[index * DAMAGE_LEN..(index + 1) * DAMAGE_LEN])?;
        }
        self.memory
            .write_atomic_bytes(slot_offset + damages_offset, &damage_bytes)?;

        self.memory.atomic_store(
            slot_offset + SLOT_SEQUENCE_OFFSET,
            2 * generation,
            Ordering::Release,
        );
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

fn validate_snapshot(
    region: &RegionHeader,
    snapshot: &SnapshotWrite<'_>,
) -> Result<(), WriterError> {
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
    let cells_len = expected_cells
        .checked_mul(CELL_LEN)
        .ok_or(WriterError::OutOfBounds)?;
    let damages_len = snapshot
        .damages
        .len()
        .checked_mul(DAMAGE_LEN)
        .ok_or(WriterError::OutOfBounds)?;
    let slot_payload_end = SLOT_HEADER_LEN
        .checked_add(cells_len)
        .and_then(|value| value.checked_add(damages_len))
        .ok_or(WriterError::OutOfBounds)?;
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
            WriterError::OutOfBounds | WriterError::UnalignedAccess | WriterError::GenerationExhausted => {
                Self::OutOfBounds
            }
            WriterError::InvalidInput(layout) => Self::Layout(layout),
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

/// Reads only the immutable/static parts of the region header using atomic
/// word loads, deliberately skipping the publication word at bytes 96..104.
pub fn read_region_header(memory: &RegionMemory) -> Result<RegionHeader, ReaderError> {
    if memory.len() < REGION_HEADER_LEN {
        return Err(ReaderError::OutOfBounds);
    }
    let mut bytes = [0u8; REGION_HEADER_LEN];
    let prefix = memory.read_atomic_bytes(0, PUBLICATION_WORD_OFFSET)?;
    bytes[..PUBLICATION_WORD_OFFSET].copy_from_slice(&prefix);
    let suffix_offset = PUBLICATION_WORD_OFFSET + 8;
    let suffix = memory.read_atomic_bytes(suffix_offset, REGION_HEADER_LEN - suffix_offset)?;
    bytes[suffix_offset..].copy_from_slice(&suffix);
    Ok(RegionHeader::decode(&bytes)?)
}

pub fn read_latest(
    memory: &RegionMemory,
    region: &RegionHeader,
) -> Result<SnapshotRead, ReaderError> {
    for _ in 0..MAX_READ_RETRIES {
        let publication = memory.atomic_load(PUBLICATION_WORD_OFFSET, Ordering::Acquire);
        let generation = publication >> 1;
        let slot = (publication & 1) as u8;
        if generation == 0 {
            return Err(ReaderError::NoReadableGeneration);
        }
        let slot_offset = region.slot_offset(slot)? as usize;
        let sequence_before =
            memory.atomic_load(slot_offset + SLOT_SEQUENCE_OFFSET, Ordering::Acquire);
        if !sequence_before.is_multiple_of(2) || sequence_before != 2 * generation {
            continue;
        }

        let mut header_bytes = [0u8; SLOT_HEADER_LEN];
        let static_header = memory.read_atomic_bytes(slot_offset + 8, SLOT_HEADER_LEN - 8)?;
        header_bytes[8..].copy_from_slice(&static_header);
        let header = SlotHeader::decode(&header_bytes, region.capacity_rows, region.capacity_cols)?;
        if header.generation != generation {
            continue;
        }
        let (cells_range, damages_range) = header.cell_and_damage_ranges(region.slot_stride)?;
        let cell_bytes = memory.read_atomic_bytes(
            slot_offset + cells_range.start,
            cells_range.end - cells_range.start,
        )?;
        let damage_bytes = memory.read_atomic_bytes(
            slot_offset + damages_range.start,
            damages_range.end - damages_range.start,
        )?;

        let mut cells = Vec::with_capacity(header.cell_count as usize);
        for chunk in cell_bytes.chunks_exact(CELL_LEN) {
            cells.push(CellRecord::decode(chunk)?);
        }
        let mut damages = Vec::with_capacity(header.damage_count as usize);
        for chunk in damage_bytes.chunks_exact(DAMAGE_LEN) {
            damages.push(DamageRecord::decode(chunk, header.rows)?);
        }

        let sequence_after =
            memory.atomic_load(slot_offset + SLOT_SEQUENCE_OFFSET, Ordering::Acquire);
        let publication_after = memory.atomic_load(PUBLICATION_WORD_OFFSET, Ordering::Acquire);
        if sequence_after == sequence_before
            && sequence_after == 2 * generation
            && publication_after == publication
        {
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
    use crate::projection::layout::{WireAttributes, WireColor};

    fn aligned_region(bytes: usize) -> (Box<[u64]>, RegionMemory) {
        let words = bytes.div_ceil(8);
        let mut storage = vec![0u64; words].into_boxed_slice();
        let ptr = storage.as_mut_ptr().cast::<u8>();
        // SAFETY: storage remains alive for every returned memory handle,
        // has `u64` alignment, and is at least `bytes` long.
        let memory = unsafe { RegionMemory::new(ptr, words * 8) };
        (storage, memory)
    }

    fn initialize_region_header(memory: RegionMemory, region: RegionHeader) {
        let mut bytes = [0u8; REGION_HEADER_LEN];
        region.encode(&mut bytes).unwrap();
        memory
            .write_atomic_bytes(0, &bytes[..PUBLICATION_WORD_OFFSET])
            .unwrap();
        memory
            .write_atomic_bytes(PUBLICATION_WORD_OFFSET + 8, &bytes[PUBLICATION_WORD_OFFSET + 8..])
            .unwrap();
    }

    fn small_region_header(
        capacity_rows: u16,
        capacity_cols: u16,
        slot_stride: u64,
    ) -> RegionHeader {
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
        initialize_region_header(memory, region);
        let _writer = Writer::new(memory, region).unwrap();
        assert_eq!(
            read_latest(&memory, &region),
            Err(ReaderError::NoReadableGeneration)
        );
    }

    #[test]
    fn region_header_reader_skips_live_publication_word() {
        let region = small_region_header(2, 2, 4096);
        let (_storage, memory) = aligned_region(region.region_bytes as usize);
        initialize_region_header(memory, region);
        memory.atomic_store(PUBLICATION_WORD_OFFSET, 1234, Ordering::Release);
        assert_eq!(read_region_header(&memory).unwrap(), region);
    }

    #[test]
    fn writer_never_overwrites_odd_sequence_with_static_header_copy() {
        let region = small_region_header(1, 1, 4096);
        let (_storage, memory) = aligned_region(region.region_bytes as usize);
        initialize_region_header(memory, region);
        let mut writer = Writer::new(memory, region).unwrap();
        let cells = blank_cells(1, 1);
        let snapshot = SnapshotWrite {
            rows: 1,
            columns: 1,
            cursor_row: 0,
            cursor_col: 0,
            cursor_visible: true,
            cursor_style: 0,
            mode_flags: ModeFlags::default(),
            cells: &cells,
            damages: &[],
            full_snapshot: true,
            source_damage_generation: 1,
        };
        writer.publish(&snapshot).unwrap();
        let slot0 = region.slot_offset(0).unwrap() as usize;
        assert_eq!(memory.atomic_load(slot0, Ordering::Acquire), 2);
    }

    #[test]
    fn writer_publish_then_reader_read_round_trips_a_full_snapshot() {
        let region = small_region_header(2, 3, 4096);
        let (_storage, memory) = aligned_region(region.region_bytes as usize);
        initialize_region_header(memory, region);
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
        assert_eq!(writer.publish(&snapshot).unwrap(), 1);
        let read = read_latest(&memory, &region).unwrap();
        assert_eq!(read.generation, 1);
        assert_eq!(read.cells, cells);
        assert_eq!(read.damages, damages);
    }

    #[test]
    fn writer_alternates_slots_and_generations_are_monotonic() {
        let region = small_region_header(1, 1, 4096);
        let (_storage, memory) = aligned_region(region.region_bytes as usize);
        initialize_region_header(memory, region);
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
        assert_eq!(read_latest(&memory, &region).unwrap().generation, 5);
    }

    #[test]
    fn publish_rejects_cell_count_mismatch_before_touching_memory() {
        let region = small_region_header(2, 2, 4096);
        let (_storage, memory) = aligned_region(region.region_bytes as usize);
        initialize_region_header(memory, region);
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
            cells: &cells[..3],
            damages: &[],
            full_snapshot: true,
            source_damage_generation: 0,
        };
        assert_eq!(
            writer.publish(&snapshot),
            Err(WriterError::InvalidInput(LayoutError::InvalidCellCount))
        );
        assert_eq!(writer.committed_generation(), 0);
        assert_eq!(
            read_latest(&memory, &region),
            Err(ReaderError::NoReadableGeneration)
        );
    }

    #[test]
    fn aggressive_concurrent_writer_and_reader_never_observe_a_torn_generation() {
        use std::sync::Arc;
        use std::sync::atomic::AtomicBool;

        let region = small_region_header(3, 5, 4096);
        let (storage, memory) = aligned_region(region.region_bytes as usize);
        initialize_region_header(memory, region);
        let mut writer = Writer::new(memory, region).unwrap();
        const TOTAL_GENERATIONS: u64 = 20_000;
        let stop = Arc::new(AtomicBool::new(false));
        let reader_stop = Arc::clone(&stop);
        let reader = std::thread::spawn(move || {
            let mut observed_max = 0u64;
            let mut retries = 0u64;
            while !reader_stop.load(Ordering::Relaxed) {
                match read_latest(&memory, &region) {
                    Ok(snapshot) => {
                        assert!(snapshot.generation >= observed_max);
                        assert_eq!(snapshot.cells.len(), snapshot.header.cell_count as usize);
                        observed_max = snapshot.generation;
                    }
                    Err(ReaderError::NoReadableGeneration) => {}
                    Err(ReaderError::TornReadExceededRetries) => retries += 1,
                    Err(other) => panic!("unexpected reader error: {other:?}"),
                }
            }
            (observed_max, retries)
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
            assert_eq!(writer.publish(&snapshot).unwrap(), generation);
        }
        stop.store(true, Ordering::Relaxed);
        let (observed_max, retries) = reader.join().unwrap();
        assert!(observed_max <= TOTAL_GENERATIONS);
        let _ = retries;
        drop(storage);
    }
}
