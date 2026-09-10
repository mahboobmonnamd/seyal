//! Canonical retained primary history and width-derived reflow.
//!
//! History owns source text units, rather than the physical rows used by the
//! active screen.  It deliberately has no persistence or renderer dependency;
//! sealed segments are the bounded handoff seam for those consumers.

use crate::{grapheme_store::GraphemeStore, Cell, CellRole, LineId, Style};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::mem::size_of;
use std::sync::atomic::{AtomicU64, Ordering};
use unicode_segmentation::UnicodeSegmentation;

pub const HISTORY_SEGMENT_PAYLOAD_TARGET: usize = 16 * 1024;
pub const HISTORY_TAIL_PAYLOAD_LIMIT: usize = 2 * HISTORY_SEGMENT_PAYLOAD_TARGET;
pub const HISTORY_PER_EXECUTION_BYTE_CAP: usize = 32 * 1024 * 1024;
pub const HISTORY_RUNTIME_AGGREGATE_BYTE_CAP: usize = 256 * 1024 * 1024;
pub const HISTORY_PER_EXECUTION_DERIVED_INDEX_CAP: usize = 4 * 1024 * 1024;
pub const HISTORY_RUNTIME_DERIVED_INDEX_CAP: usize = 32 * 1024 * 1024;
pub const HISTORY_SELECTION_UNIT_CAP: usize = 64 * 1024;
pub const MAX_EVICTED_ID_RANGES: usize = 1_024;

static NEXT_SEGMENT_AGE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryBreakAfter {
    HardBreak,
    SoftWrap,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HistoryAnchor {
    pub line_id: LineId,
    pub unit_offset: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HistoryMatch {
    pub start: HistoryAnchor,
    pub end: HistoryAnchor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryRangeError {
    Stale,
    Unrepresentable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HistoryAnchorResolution {
    Resolved {
        text: String,
        width: u8,
        style: Style,
    },
    Unavailable,
    Invalid,
}

/// Canonical source unit projection for history consumers that need complete
/// grapheme payloads. The scalar `Cell` projection remains available for
/// legacy display framing, but it cannot carry a multi-scalar payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryUnitView {
    pub anchor: HistoryAnchor,
    pub text: String,
    pub width: u8,
    pub style: Style,
}

/// Physical history-wire cell. Continuation placeholders carry no text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryWireCell {
    pub text: String,
    pub width: u8,
    pub style: Style,
    pub continuation: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HistoryUnit {
    pub utf8: Vec<u8>,
    pub width: u8,
    pub style: Style,
}

impl HistoryUnit {
    fn canonical_payload_len(&self) -> usize {
        self.utf8.len()
    }

    fn allocated_bytes(&self) -> usize {
        // The containing units Vec allocation accounts for width/style and
        // the HistoryUnit Vec metadata; this is the UTF-8 allocation itself.
        self.utf8.capacity()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HistoryLine {
    pub line_id: LineId,
    pub units: Vec<HistoryUnit>,
    pub break_after: HistoryBreakAfter,
    pub start_offset: u32,
}

impl HistoryLine {
    pub(crate) fn from_cells(
        line_id: LineId,
        break_after: HistoryBreakAfter,
        cells: &[Cell],
        store: &GraphemeStore,
    ) -> Self {
        let content_end = cells
            .iter()
            .rposition(|cell| cell.role != CellRole::Empty)
            .map_or(0, |index| index + 1);
        let units = cells[..content_end]
            .iter()
            .filter_map(|cell| match cell.role {
                CellRole::Continuation => None,
                CellRole::Empty => Some(HistoryUnit {
                    utf8: vec![b' '],
                    width: 1,
                    style: cell.style,
                }),
                CellRole::Lead => {
                    let utf8 = if cell.overflow {
                        vec![0xef, 0xbf, 0xbd]
                    } else {
                        store.get(cell.store_id).map_or_else(
                            || cell.character.to_string().into_bytes(),
                            ToOwned::to_owned,
                        )
                    };
                    Some(HistoryUnit {
                        utf8,
                        width: cell.width.max(1),
                        style: cell.style,
                    })
                }
            })
            .collect();
        Self {
            line_id,
            units,
            break_after,
            start_offset: 0,
        }
    }

    fn canonical_payload_len(&self) -> usize {
        self.units
            .iter()
            .map(HistoryUnit::canonical_payload_len)
            .sum()
    }

    fn allocated_bytes(&self) -> usize {
        self.units
            .capacity()
            .saturating_mul(size_of::<HistoryUnit>())
            .saturating_add(
                self.units
                    .iter()
                    .map(HistoryUnit::allocated_bytes)
                    .sum::<usize>(),
            )
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SegmentLine {
    line_id: LineId,
    unit_start: u32,
    unit_len: u32,
    start_offset: u32,
    break_after: HistoryBreakAfter,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SegmentUnit {
    payload_start: u32,
    payload_len: u32,
    width: u8,
    style: Style,
}

#[derive(Clone, Debug)]
pub(crate) struct Segment {
    lines: Box<[SegmentLine]>,
    units: Box<[SegmentUnit]>,
    payload: Box<[u8]>,
    age: u64,
    resident_bytes: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct EvictedIdRange {
    first: LineId,
    last: LineId,
}

#[derive(Clone, Copy)]
pub(crate) enum HistoryLineRef<'a> {
    Sealed(&'a Segment, &'a SegmentLine),
    Tail(&'a HistoryLine),
}

#[derive(Clone, Debug)]
struct ReflowCache {
    columns: u16,
    max_rows: usize,
    eviction_generation: u64,
    rows: Vec<ReflowRow>,
}

#[derive(Clone, Copy)]
enum HistoryUnitRef<'a> {
    Sealed(&'a Segment, &'a SegmentUnit),
    Tail(&'a HistoryUnit),
}

enum HistoryUnits<'a> {
    Sealed {
        segment: &'a Segment,
        units: std::slice::Iter<'a, SegmentUnit>,
    },
    Tail(std::slice::Iter<'a, HistoryUnit>),
}

impl<'a> Iterator for HistoryUnits<'a> {
    type Item = HistoryUnitRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Sealed { segment, units } => units
                .next()
                .map(|unit| HistoryUnitRef::Sealed(segment, unit)),
            Self::Tail(units) => units.next().map(HistoryUnitRef::Tail),
        }
    }
}

impl<'a> HistoryUnitRef<'a> {
    fn utf8(self) -> &'a [u8] {
        match self {
            Self::Sealed(segment, unit) => {
                let start = unit.payload_start as usize;
                let end = start.saturating_add(unit.payload_len as usize);
                segment.payload.get(start..end).unwrap_or(&[])
            }
            Self::Tail(unit) => &unit.utf8,
        }
    }

    fn width(self) -> u8 {
        match self {
            Self::Sealed(_, unit) => unit.width,
            Self::Tail(unit) => unit.width,
        }
    }

    fn style(self) -> Style {
        match self {
            Self::Sealed(_, unit) => unit.style,
            Self::Tail(unit) => unit.style,
        }
    }

    fn first_scalar(self) -> char {
        std::str::from_utf8(self.utf8())
            .ok()
            .and_then(|text| text.chars().next())
            .unwrap_or('\u{FFFD}')
    }

    fn presentation_cell(self) -> Cell {
        Cell::lead_inline(self.first_scalar(), self.width().max(1), self.style())
    }

    fn reflow_cell(self) -> Cell {
        self.presentation_cell()
    }
}

impl<'a> HistoryLineRef<'a> {
    pub(crate) fn line_id(self) -> LineId {
        match self {
            Self::Sealed(_, line) => line.line_id,
            Self::Tail(line) => line.line_id,
        }
    }

    pub(crate) fn start_offset(self) -> u32 {
        match self {
            Self::Sealed(_, line) => line.start_offset,
            Self::Tail(line) => line.start_offset,
        }
    }

    pub(crate) fn break_after(self) -> HistoryBreakAfter {
        match self {
            Self::Sealed(_, line) => line.break_after,
            Self::Tail(line) => line.break_after,
        }
    }

    fn units(self) -> HistoryUnits<'a> {
        match self {
            Self::Sealed(segment, line) => {
                let start = line.unit_start as usize;
                let end = start.saturating_add(line.unit_len as usize);
                HistoryUnits::Sealed {
                    segment,
                    units: segment.units[start..end].iter(),
                }
            }
            Self::Tail(line) => HistoryUnits::Tail(line.units.iter()),
        }
    }

    pub(crate) fn presentation_cells(self) -> Vec<Cell> {
        let mut cells = Vec::new();
        for unit in self.units() {
            cells.push(unit.presentation_cell());
            if unit.width() >= 2 {
                cells.push(Cell::continuation());
            }
        }
        cells
    }

    pub(crate) fn to_owned_line(self) -> HistoryLine {
        HistoryLine {
            line_id: self.line_id(),
            break_after: self.break_after(),
            start_offset: self.start_offset(),
            units: self
                .units()
                .map(|unit| HistoryUnit {
                    utf8: unit.utf8().to_vec(),
                    width: unit.width(),
                    style: unit.style(),
                })
                .collect(),
        }
    }

    pub(crate) fn wire_cells(self) -> Vec<HistoryWireCell> {
        let mut cells = Vec::new();
        for unit in self.units() {
            cells.push(HistoryWireCell {
                text: String::from_utf8_lossy(unit.utf8()).into_owned(),
                width: unit.width(),
                style: unit.style(),
                continuation: false,
            });
            if unit.width() >= 2 {
                cells.push(HistoryWireCell {
                    text: String::new(),
                    width: 0,
                    style: unit.style(),
                    continuation: true,
                });
            }
        }
        cells
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct HistoryStore {
    segments: VecDeque<Segment>,
    tail: Vec<HistoryLine>,
    tail_payload_bytes: usize,
    tail_resident_bytes: usize,
    segments_resident_bytes: usize,
    resident_bytes: usize,
    eviction_generation: u64,
    // Exact disjoint runs of source identities whose canonical payload was
    // evicted. Alternate-screen allocations create gaps and must not be
    // absorbed into an unavailable range. The Vec allocation is included in
    // resident_bytes, so this metadata participates in the same hard cap.
    evicted_id_ranges: Vec<EvictedIdRange>,
    evicted_through: Option<LineId>,
    reflow_cache: RefCell<Option<ReflowCache>>,
}

impl HistoryStore {
    pub(crate) fn entries(&self) -> impl Iterator<Item = HistoryLineRef<'_>> {
        self.segments
            .iter()
            .flat_map(|segment| {
                segment
                    .lines
                    .iter()
                    .map(move |line| HistoryLineRef::Sealed(segment, line))
            })
            .chain(self.tail.iter().map(HistoryLineRef::Tail))
    }

    pub(crate) fn resident_bytes(&self) -> usize {
        self.resident_bytes
    }

    pub(crate) fn derived_cache_bytes(&self) -> usize {
        self.reflow_cache
            .borrow()
            .as_ref()
            .map_or(0, |cache| reflow_rows_allocated_bytes(&cache.rows))
    }

    pub(crate) fn drop_derived_cache(&self) {
        self.reflow_cache.borrow_mut().take();
    }

    pub(crate) fn eviction_generation(&self) -> u64 {
        self.eviction_generation
    }

    pub(crate) fn replace_payload(&mut self, lines: Vec<HistoryLine>) {
        self.reflow_cache.get_mut().take();
        self.segments.clear();
        self.tail.clear();
        self.tail_payload_bytes = 0;
        self.tail_resident_bytes = 0;
        self.segments_resident_bytes = 0;
        self.update_resident_bytes();
        for line in lines {
            self.append_line(line);
        }
    }

    pub(crate) fn append_row(
        &mut self,
        line_id: LineId,
        break_after: HistoryBreakAfter,
        cells: &[Cell],
        store: &GraphemeStore,
    ) {
        self.append_line(HistoryLine::from_cells(line_id, break_after, cells, store));
    }

    pub(crate) fn append_line(&mut self, line: HistoryLine) {
        self.reflow_cache.get_mut().take();
        let bytes = line.canonical_payload_len();
        // A source line can be arbitrarily long. Fragmenting at unit boundaries
        // keeps the tail bounded while preserving its LineId and break lineage.
        if bytes > HISTORY_SEGMENT_PAYLOAD_TARGET {
            let mut fragment = Vec::new();
            let mut fragment_bytes = 0usize;
            let mut source_offset = line.start_offset;
            let line_id = line.line_id;
            let final_break = line.break_after;
            for unit in line.units {
                let unit_bytes = unit.canonical_payload_len();
                if !fragment.is_empty()
                    && fragment_bytes + unit_bytes > HISTORY_SEGMENT_PAYLOAD_TARGET
                {
                    let fragment_start = source_offset - fragment.len() as u32;
                    self.push_fragment(HistoryLine {
                        line_id,
                        units: std::mem::take(&mut fragment),
                        break_after: HistoryBreakAfter::SoftWrap,
                        start_offset: fragment_start,
                    });
                    fragment_bytes = 0;
                }
                fragment_bytes += unit_bytes;
                fragment.push(unit);
                source_offset = source_offset.saturating_add(1);
            }
            if !fragment.is_empty() {
                let fragment_start = source_offset - fragment.len() as u32;
                self.push_fragment(HistoryLine {
                    line_id,
                    units: fragment,
                    break_after: final_break,
                    start_offset: fragment_start,
                });
            }
        } else {
            self.push_fragment(line);
        }
        self.update_resident_bytes();
        self.evict_to_cap();
    }

    fn push_fragment(&mut self, line: HistoryLine) {
        let bytes = line.canonical_payload_len();
        if self.tail_payload_bytes + bytes > HISTORY_SEGMENT_PAYLOAD_TARGET && !self.tail.is_empty()
        {
            self.seal_tail();
        }
        self.tail_payload_bytes += bytes;
        let line_resident_bytes = line.allocated_bytes();
        let old_capacity = self.tail.capacity();
        self.tail.push(line);
        self.tail_resident_bytes = self
            .tail_resident_bytes
            .saturating_add(line_resident_bytes)
            .saturating_add(
                self.tail
                    .capacity()
                    .saturating_sub(old_capacity)
                    .saturating_mul(size_of::<HistoryLine>()),
            );
        self.update_resident_bytes();
        if self.tail_payload_bytes >= HISTORY_SEGMENT_PAYLOAD_TARGET {
            self.seal_tail();
        }
    }

    fn seal_tail(&mut self) {
        if self.tail.is_empty() {
            return;
        }
        let tail = std::mem::take(&mut self.tail);
        let mut lines = Vec::with_capacity(tail.len());
        let unit_count = tail.iter().map(|line| line.units.len()).sum();
        let payload_len = tail
            .iter()
            .flat_map(|line| &line.units)
            .map(|unit| unit.utf8.len())
            .sum();
        let mut units = Vec::with_capacity(unit_count);
        let mut payload = Vec::with_capacity(payload_len);
        for line in tail {
            let unit_start = u32::try_from(units.len()).unwrap_or(u32::MAX);
            for unit in line.units {
                let payload_start = u32::try_from(payload.len()).unwrap_or(u32::MAX);
                let payload_len = u32::try_from(unit.utf8.len()).unwrap_or(u32::MAX);
                payload.extend_from_slice(&unit.utf8);
                units.push(SegmentUnit {
                    payload_start,
                    payload_len,
                    width: unit.width,
                    style: unit.style,
                });
            }
            lines.push(SegmentLine {
                line_id: line.line_id,
                unit_start,
                unit_len: u32::try_from(units.len())
                    .unwrap_or(u32::MAX)
                    .saturating_sub(unit_start),
                start_offset: line.start_offset,
                break_after: line.break_after,
            });
        }
        let lines = lines.into_boxed_slice();
        let units = units.into_boxed_slice();
        let payload = payload.into_boxed_slice();
        // Segment values live in the VecDeque allocation accounted below.
        // This is the exact heap storage owned through the three boxed slices.
        let resident_bytes = lines
            .len()
            .saturating_mul(size_of::<SegmentLine>())
            .saturating_add(units.len().saturating_mul(size_of::<SegmentUnit>()))
            .saturating_add(payload.len());
        let segment = Segment {
            lines,
            units,
            payload,
            age: NEXT_SEGMENT_AGE.fetch_add(1, Ordering::Relaxed),
            resident_bytes,
        };
        self.tail_payload_bytes = 0;
        self.tail_resident_bytes = 0;
        let old_capacity = self.segments.capacity();
        self.segments.push_back(segment);
        self.segments_resident_bytes = self
            .segments_resident_bytes
            .saturating_add(
                self.segments
                    .back()
                    .map_or(0, |segment| segment.resident_bytes),
            )
            .saturating_add(
                self.segments
                    .capacity()
                    .saturating_sub(old_capacity)
                    .saturating_mul(size_of::<Segment>()),
            );
        self.update_resident_bytes();
    }

    fn update_resident_bytes(&mut self) {
        self.resident_bytes = size_of::<Self>()
            .saturating_add(self.tail_resident_bytes)
            .saturating_add(self.segments_resident_bytes)
            .saturating_add(
                self.evicted_id_ranges
                    .capacity()
                    .saturating_mul(size_of::<EvictedIdRange>()),
            );
    }

    fn evict_to_cap(&mut self) {
        self.update_resident_bytes();
        while self.resident_bytes > HISTORY_PER_EXECUTION_BYTE_CAP {
            let Some(segment) = self.segments.pop_front() else {
                // The tail is capped by source-unit fragmentation and cannot
                // be discarded piecemeal without an explicit fragment policy.
                break;
            };
            self.record_evicted_segment(&segment);
            self.segments_resident_bytes = self
                .segments_resident_bytes
                .saturating_sub(segment.resident_bytes);
            self.update_resident_bytes();
            self.eviction_generation = self.eviction_generation.wrapping_add(1);
        }
    }

    pub(crate) fn oldest_segment_age(&self) -> Option<u64> {
        self.segments.front().map(|segment| segment.age)
    }

    pub(crate) fn evict_oldest_segment(&mut self) -> usize {
        self.reflow_cache.get_mut().take();
        let before = self.resident_bytes;
        let Some(segment) = self.segments.pop_front() else {
            return 0;
        };
        self.record_evicted_segment(&segment);
        self.segments_resident_bytes = self
            .segments_resident_bytes
            .saturating_sub(segment.resident_bytes);
        self.update_resident_bytes();
        self.eviction_generation = self.eviction_generation.wrapping_add(1);
        before.saturating_sub(self.resident_bytes)
    }

    fn record_evicted_segment(&mut self, segment: &Segment) {
        for line_id in segment.lines.iter().map(|line| line.line_id) {
            let Some(last) = self.evicted_id_ranges.last_mut() else {
                self.evicted_id_ranges.push(EvictedIdRange {
                    first: line_id,
                    last: line_id,
                });
                continue;
            };
            if line_id <= last.last {
                continue;
            }
            if last.last.0.checked_add(1) == Some(line_id.0) {
                last.last = line_id;
            } else {
                self.evicted_id_ranges.push(EvictedIdRange {
                    first: line_id,
                    last: line_id,
                });
            }
        }
        self.compact_evicted_id_ranges();
    }

    #[cfg(test)]
    fn record_evicted_range_for_test(&mut self, first: u64, last: u64) {
        self.evicted_id_ranges.push(EvictedIdRange {
            first: LineId(first),
            last: LineId(last),
        });
        self.compact_evicted_id_ranges();
    }

    fn compact_evicted_id_ranges(&mut self) {
        while self.evicted_id_ranges.len() > MAX_EVICTED_ID_RANGES {
            let oldest = self.evicted_id_ranges.remove(0);
            self.evicted_through = Some(match self.evicted_through {
                Some(current) => current.max(oldest.last),
                None => oldest.last,
            });
        }
    }

    pub(crate) fn range_intersects_evicted(&self, start: LineId, end: LineId) -> bool {
        if let Some(through) = self.evicted_through
            && start <= through
        {
            return true;
        }
        let index = self
            .evicted_id_ranges
            .partition_point(|range| range.last < start);
        self.evicted_id_ranges
            .get(index)
            .is_some_and(|range| range.first <= end)
    }

    fn line_was_evicted(&self, line_id: LineId) -> bool {
        if let Some(through) = self.evicted_through
            && line_id <= through
        {
            return true;
        }
        let index = self
            .evicted_id_ranges
            .partition_point(|range| range.last < line_id);
        self.evicted_id_ranges
            .get(index)
            .is_some_and(|range| line_id >= range.first)
    }

    pub(crate) fn resolve_anchor(&self, anchor: HistoryAnchor) -> HistoryAnchorResolution {
        let line = self.entries().find(|line| {
            line.line_id() == anchor.line_id
                && anchor.unit_offset >= line.start_offset()
                && anchor.unit_offset.saturating_sub(line.start_offset())
                    < line.units().count() as u32
        });
        let Some(line) = line else {
            return if self.line_was_evicted(anchor.line_id) {
                HistoryAnchorResolution::Unavailable
            } else {
                HistoryAnchorResolution::Invalid
            };
        };
        let Some(relative) = anchor.unit_offset.checked_sub(line.start_offset()) else {
            return HistoryAnchorResolution::Invalid;
        };
        let Ok(relative) = usize::try_from(relative) else {
            return HistoryAnchorResolution::Invalid;
        };
        let Some(unit) = line.units().nth(relative) else {
            return HistoryAnchorResolution::Invalid;
        };
        let Ok(text) = String::from_utf8(unit.utf8().to_vec()) else {
            return HistoryAnchorResolution::Unavailable;
        };
        HistoryAnchorResolution::Resolved {
            text,
            width: unit.width(),
            style: unit.style(),
        }
    }

    pub(crate) fn reflow(&self, cols: u16, max_rows: usize) -> Vec<ReflowRow> {
        let generation = self.eviction_generation;
        if let Some(cache) = self.reflow_cache.borrow().as_ref()
            && (cache.columns, cache.max_rows, cache.eviction_generation)
                == (cols, max_rows, generation)
        {
            return cache.rows.clone();
        }
        let rows = self.reflow_uncached(cols, max_rows);
        let estimated = reflow_rows_allocated_bytes(&rows);
        if estimated <= HISTORY_PER_EXECUTION_DERIVED_INDEX_CAP {
            *self.reflow_cache.borrow_mut() = Some(ReflowCache {
                columns: cols,
                max_rows,
                eviction_generation: generation,
                rows: rows.clone(),
            });
        }
        rows
    }

    pub(crate) fn reflow_uncached(&self, cols: u16, max_rows: usize) -> Vec<ReflowRow> {
        if cols == 0 || max_rows == 0 {
            return Vec::new();
        }
        let width = usize::from(cols);
        let mut rows = Vec::new();
        let mut current = ReflowRow::default();
        let mut previous_break = None;
        for line in self.entries() {
            let joins = previous_break == Some(HistoryBreakAfter::SoftWrap);
            if !joins && !current.cells.is_empty() {
                rows.push(std::mem::take(&mut current));
                if rows.len() >= max_rows {
                    break;
                }
            }
            for (unit_index, unit) in line.units().enumerate() {
                let unit_width = usize::from(unit.width().max(1));
                if !current.cells.is_empty() && current.cells.len() + unit_width > width {
                    current.break_after = Some(HistoryBreakAfter::SoftWrap);
                    rows.push(std::mem::take(&mut current));
                    if rows.len() >= max_rows {
                        return rows;
                    }
                }
                let anchor = HistoryAnchor {
                    line_id: line.line_id(),
                    unit_offset: line.start_offset().saturating_add(unit_index as u32),
                };
                if unit_width > width {
                    if !current.cells.is_empty() || !current.anchors.is_empty() {
                        current.break_after = Some(HistoryBreakAfter::SoftWrap);
                        rows.push(std::mem::take(&mut current));
                        if rows.len() >= max_rows {
                            return rows;
                        }
                    }
                    rows.push(ReflowRow {
                        source_line_id: Some(line.line_id()),
                        anchors: vec![anchor],
                        cells: Vec::new(),
                        break_after: Some(HistoryBreakAfter::SoftWrap),
                        unavailable: true,
                    });
                    if rows.len() >= max_rows {
                        return rows;
                    }
                    continue;
                }
                current.source_line_id.get_or_insert(line.line_id());
                current.anchors.push(anchor);
                current.cells.push(unit.reflow_cell());
                if unit_width == 2 {
                    current.cells.push(Cell::continuation());
                }
            }
            previous_break = Some(line.break_after());
            if line.break_after() == HistoryBreakAfter::HardBreak {
                if current.cells.is_empty() && current.anchors.is_empty() {
                    if let Some(last) = rows.last_mut().filter(|row| row.unavailable) {
                        last.break_after = Some(HistoryBreakAfter::HardBreak);
                    } else {
                        current.source_line_id = Some(line.line_id());
                        current.break_after = Some(HistoryBreakAfter::HardBreak);
                        rows.push(std::mem::take(&mut current));
                    }
                } else {
                    current.break_after = Some(HistoryBreakAfter::HardBreak);
                    rows.push(std::mem::take(&mut current));
                }
                if rows.len() >= max_rows {
                    break;
                }
                previous_break = None;
            }
        }
        if !current.cells.is_empty() && rows.len() < max_rows {
            current.break_after = previous_break;
            rows.push(current);
        }
        rows
    }

    pub(crate) fn source_units(
        &self,
        start: LineId,
        end: LineId,
        max_units: usize,
    ) -> Vec<HistoryUnitView> {
        if max_units == 0 || end < start {
            return Vec::new();
        }
        let mut units = Vec::new();
        for line in self.entries() {
            if line.line_id() < start {
                continue;
            }
            if line.line_id() > end {
                break;
            }
            for (index, unit) in line.units().enumerate() {
                units.push(HistoryUnitView {
                    anchor: HistoryAnchor {
                        line_id: line.line_id(),
                        unit_offset: line.start_offset().saturating_add(index as u32),
                    },
                    text: String::from_utf8_lossy(unit.utf8()).into_owned(),
                    width: unit.width(),
                    style: unit.style(),
                });
                if units.len() >= max_units {
                    return units;
                }
            }
        }
        units
    }

    pub(crate) fn search(&self, needle: &str, max_matches: usize) -> Vec<HistoryMatch> {
        if needle.is_empty() || max_matches == 0 {
            return Vec::new();
        }
        let needle_units = needle.graphemes(true).count();
        let mut window: Vec<HistoryUnitView> = Vec::with_capacity(needle_units.max(1));
        let mut matches = Vec::new();
        for line in self.entries() {
            for (index, unit) in line.units().enumerate() {
                window.push(HistoryUnitView {
                    anchor: HistoryAnchor {
                        line_id: line.line_id(),
                        unit_offset: line.start_offset().saturating_add(index as u32),
                    },
                    text: String::from_utf8_lossy(unit.utf8()).into_owned(),
                    width: unit.width(),
                    style: unit.style(),
                });
                while window.len() > needle_units {
                    window.remove(0);
                }
                let text = window
                    .iter()
                    .map(|unit| unit.text.as_str())
                    .collect::<String>();
                if text == needle {
                    matches.push(HistoryMatch {
                        start: window[0].anchor,
                        end: window[window.len() - 1].anchor,
                    });
                    if matches.len() >= max_matches {
                        return matches;
                    }
                }
            }
            if line.break_after() == HistoryBreakAfter::HardBreak {
                window.clear();
            }
        }
        matches
    }

    pub(crate) fn selection(
        &self,
        start: HistoryAnchor,
        end: HistoryAnchor,
    ) -> Result<Vec<HistoryUnitView>, HistoryRangeError> {
        if start > end {
            return Err(HistoryRangeError::Unrepresentable);
        }
        if self.range_intersects_evicted(start.line_id, end.line_id) {
            return Err(HistoryRangeError::Stale);
        }
        let mut selected = Vec::new();
        for line in self.entries() {
            if line.line_id() < start.line_id {
                continue;
            }
            if line.line_id() > end.line_id {
                break;
            }
            for (index, unit) in line.units().enumerate() {
                let view = HistoryUnitView {
                    anchor: HistoryAnchor {
                        line_id: line.line_id(),
                        unit_offset: line.start_offset().saturating_add(index as u32),
                    },
                    text: String::from_utf8_lossy(unit.utf8()).into_owned(),
                    width: unit.width(),
                    style: unit.style(),
                };
                if view.anchor >= start && view.anchor <= end {
                    if selected.len() >= HISTORY_SELECTION_UNIT_CAP {
                        return Err(HistoryRangeError::Unrepresentable);
                    }
                    selected.push(view);
                }
            }
        }
        if selected.is_empty() {
            return Err(HistoryRangeError::Stale);
        }
        Ok(selected)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReflowRow {
    pub source_line_id: Option<LineId>,
    pub anchors: Vec<HistoryAnchor>,
    pub cells: Vec<Cell>,
    pub break_after: Option<HistoryBreakAfter>,
    pub unavailable: bool,
}

fn reflow_rows_allocated_bytes(rows: &[ReflowRow]) -> usize {
    size_of::<Vec<ReflowRow>>()
        .saturating_add(rows.len().saturating_mul(size_of::<ReflowRow>()))
        .saturating_add(
            rows.iter()
                .map(|row| {
                    row.cells
                        .capacity()
                        .saturating_mul(size_of::<Cell>())
                        .saturating_add(
                            row.anchors
                                .capacity()
                                .saturating_mul(size_of::<HistoryAnchor>()),
                        )
                })
                .sum::<usize>(),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ascii_line(id: u64, text: &str, break_after: HistoryBreakAfter) -> HistoryLine {
        HistoryLine {
            line_id: LineId(id),
            units: text
                .bytes()
                .map(|byte| HistoryUnit {
                    utf8: vec![byte],
                    width: 1,
                    style: Style::default(),
                })
                .collect(),
            break_after,
            start_offset: 0,
        }
    }

    #[test]
    fn seals_by_payload_bytes_and_reflows_soft_chain() {
        let mut store = HistoryStore::default();
        for id in 0..4_000 {
            store.append_line(ascii_line(id, "abcdefghij", HistoryBreakAfter::SoftWrap));
        }
        assert!(
            store.segments.len() >= 2,
            "canonical UTF-8 payload should exceed {} bytes twice: segments={} payload={}",
            HISTORY_SEGMENT_PAYLOAD_TARGET,
            store.segments.len(),
            store
                .segments
                .iter()
                .map(|s| s.payload.len())
                .sum::<usize>()
        );
        assert!(store.tail_payload_bytes <= HISTORY_SEGMENT_PAYLOAD_TARGET);
        assert!(store.resident_bytes > 0);

        let rows = store.reflow(16, 8);
        assert_eq!(rows.len(), 8);
        assert_eq!(rows[0].cells.len(), 16);
        assert_eq!(
            rows[0].anchors[0],
            HistoryAnchor {
                line_id: LineId(0),
                unit_offset: 0
            }
        );
        assert_eq!(rows[1].anchors[0].unit_offset, 6);
    }

    #[test]
    fn resident_bytes_include_allocation_and_metadata_overhead() {
        let mut store = HistoryStore::default();
        store.append_line(ascii_line(7, "abc", HistoryBreakAfter::HardBreak));

        let payload_bytes = store.tail_payload_bytes;
        assert!(store.resident_bytes() > payload_bytes);
    }

    #[test]
    fn oldest_eviction_reports_actual_resident_delta() {
        let mut store = HistoryStore::default();
        for id in 0..2_000 {
            store.append_line(ascii_line(id, "abcdefghij", HistoryBreakAfter::HardBreak));
        }
        let before = store.resident_bytes();
        let removed = store.evict_oldest_segment();
        assert!(removed > 0);
        assert_eq!(before - removed, store.resident_bytes());
    }

    #[test]
    fn sealed_segments_use_exact_contiguous_payload_and_offset_metadata() {
        let mut store = HistoryStore::default();
        for id in 0..4_000 {
            store.append_line(ascii_line(id, "abcdefghij", HistoryBreakAfter::HardBreak));
        }

        assert!(
            store.segments.len() >= 2,
            "canonical UTF-8 payload should exceed {} bytes twice: segments={}",
            HISTORY_SEGMENT_PAYLOAD_TARGET,
            store.segments.len()
        );
        for segment in &store.segments {
            let exact_content_bytes = segment
                .lines
                .len()
                .saturating_mul(size_of::<SegmentLine>())
                .saturating_add(segment.units.len().saturating_mul(size_of::<SegmentUnit>()))
                .saturating_add(segment.payload.len());
            assert!(segment.payload.len() <= HISTORY_SEGMENT_PAYLOAD_TARGET);
            assert_eq!(segment.resident_bytes, exact_content_bytes);

            let mut next_unit = 0usize;
            for line in &segment.lines {
                assert_eq!(line.unit_start as usize, next_unit);
                next_unit = next_unit.saturating_add(line.unit_len as usize);
            }
            assert_eq!(next_unit, segment.units.len());

            let mut next_payload = 0usize;
            for unit in &segment.units {
                assert_eq!(unit.payload_start as usize, next_payload);
                next_payload = next_payload.saturating_add(unit.payload_len as usize);
            }
            assert_eq!(next_payload, segment.payload.len());
        }
    }

    #[test]
    fn hard_break_stops_reflow_chain_and_wide_units_stay_atomic() {
        let mut store = HistoryStore::default();
        store.append_line(ascii_line(1, "abc", HistoryBreakAfter::HardBreak));
        store.append_line(HistoryLine {
            line_id: LineId(2),
            units: vec![HistoryUnit {
                utf8: "界".as_bytes().to_vec(),
                width: 2,
                style: Style::default(),
            }],
            break_after: HistoryBreakAfter::HardBreak,
            start_offset: 0,
        });
        let rows = store.reflow(2, 8);
        assert_eq!(rows[0].cells.len(), 2);
        let wide = rows
            .iter()
            .find(|row| row.cells.first().is_some_and(|cell| cell.width == 2))
            .expect("wide source unit has a visual row");
        assert_eq!(wide.cells.len(), 2);
        assert!(wide.cells[1].is_continuation());
    }

    #[test]
    fn oversized_fragment_later_source_offset_resolves() {
        let mut store = HistoryStore::default();
        let line = ascii_line(
            7,
            &"a".repeat(HISTORY_SEGMENT_PAYLOAD_TARGET * 2),
            HistoryBreakAfter::HardBreak,
        );
        store.append_line(line);
        let anchor = HistoryAnchor {
            line_id: LineId(7),
            unit_offset: (HISTORY_SEGMENT_PAYLOAD_TARGET + 1) as u32,
        };
        assert!(matches!(
            store.resolve_anchor(anchor),
            HistoryAnchorResolution::Resolved { .. }
        ));
    }

    #[test]
    fn evicted_id_metadata_is_capped_without_coarsening_later_live_ids() {
        let mut store = HistoryStore::default();
        for i in 0..(MAX_EVICTED_ID_RANGES + 8) {
            let first = (i as u64) * 10 + 1;
            store.record_evicted_range_for_test(first, first);
        }
        assert!(store.evicted_id_ranges.len() <= MAX_EVICTED_ID_RANGES);
        // Folded watermark covers the oldest compacted identity, not a later
        // never-evicted primary identity sitting in an alternate-screen gap.
        assert!(!store.line_was_evicted(LineId(MAX_EVICTED_ID_RANGES as u64 * 10 + 50)));
        assert!(store.range_intersects_evicted(LineId(1), LineId(1)));
    }
}
