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

fn line_entirely_before(line: &HistoryLine, from: HistoryAnchor) -> bool {
    if line.line_id < from.line_id {
        return true;
    }
    if line.line_id > from.line_id {
        return false;
    }
    line.start_offset
        .saturating_add(u32::try_from(line.units.len()).unwrap_or(u32::MAX))
        <= from.unit_offset
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

#[derive(Clone, Copy, Debug)]
struct WrapFragment {
    line_id: LineId,
    start_offset: u32,
    unit_count: u32,
    prefix_units: u32,
}

/// Cols-dependent occupancy after each RLE run (from column 0). Lets aperiodic
/// `wrap_column_before` answer mid-chain cuts in O(log runs + cols) after one
/// O(runs·cols) build for that width (SPEC-010 §7).
#[derive(Clone, Debug)]
struct WrapOccupancySpine {
    cols: u16,
    run_len: usize,
    after_run: Vec<u8>,
    units_after: Vec<u32>,
}

#[derive(Clone, Debug, Default)]
struct WrapChain {
    fragments: Vec<WrapFragment>,
    runs: Vec<(u8, u32)>,
    /// Repeating RLE prefix length used for occupancy. `2..=WRAP_PATTERN_MAX`
    /// is a compact template; occupancy then ignores the stored run tail.
    pattern_len: u32,
    /// Units already dropped from the front of a compacted repeating template.
    pattern_phase: u32,
    open: bool,
    /// Built lazily for aperiodic chains; invalidated when `runs` change.
    spine: RefCell<Option<WrapOccupancySpine>>,
}

impl WrapChain {
    fn invalidate_spine(&self) {
        *self.spine.borrow_mut() = None;
    }

    fn units_before(&self, from: HistoryAnchor) -> u32 {
        let index = match self.fragments.binary_search_by(|fragment| {
            (fragment.line_id, fragment.start_offset).cmp(&(from.line_id, from.unit_offset))
        }) {
            Ok(index) => return self.fragments[index].prefix_units,
            Err(index) => index,
        };
        if index == 0 {
            return 0;
        }
        let previous = &self.fragments[index - 1];
        if previous.line_id == from.line_id && from.unit_offset > previous.start_offset {
            previous.prefix_units.saturating_add(
                from.unit_offset
                    .saturating_sub(previous.start_offset)
                    .min(previous.unit_count),
            )
        } else if previous.line_id < from.line_id {
            previous.prefix_units.saturating_add(previous.unit_count)
        } else {
            0
        }
    }
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

    fn unit_len(self) -> u32 {
        match self {
            Self::Sealed(_, line) => line.unit_len,
            Self::Tail(line) => u32::try_from(line.units.len()).unwrap_or(u32::MAX),
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
    /// SoftWrap-chain width runs used to answer resize carry columns.
    /// Derived (§9.1), not resident source; closed hard-broken rows are omitted.
    wrap_chains: VecDeque<WrapChain>,
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

    pub(crate) fn reverse_entries(&self) -> impl Iterator<Item = HistoryLineRef<'_>> {
        self.tail
            .iter()
            .rev()
            .map(HistoryLineRef::Tail)
            .chain(self.segments.iter().rev().flat_map(|segment| {
                segment
                    .lines
                    .iter()
                    .rev()
                    .map(move |line| HistoryLineRef::Sealed(segment, line))
            }))
    }

    pub(crate) fn derived_cache_bytes(&self) -> usize {
        self.wrap_index_allocated_bytes().saturating_add(
            self.reflow_cache
                .borrow()
                .as_ref()
                .map_or(0, |cache| reflow_rows_allocated_bytes(&cache.rows)),
        )
    }

    pub(crate) fn drop_derived_cache(&mut self) {
        self.reflow_cache.get_mut().take();
        self.wrap_chains.clear();
        self.wrap_chains.shrink_to_fit();
    }

    pub(crate) fn eviction_generation(&self) -> u64 {
        self.eviction_generation
    }

    pub(crate) fn replace_payload(&mut self, lines: Vec<HistoryLine>) {
        self.reflow_cache.get_mut().take();
        self.wrap_chains.clear();
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

    /// Active surface plus two screenfuls of slack (SPEC-010 §7).
    pub(crate) fn eager_resize_row_budget(rows: u16) -> usize {
        usize::from(rows).saturating_mul(3).max(1)
    }

    /// Clone only the retained-history suffix that can affect the new
    /// viewport. Older sealed source stays in place and is not copied.
    ///
    /// When the cut lands inside an aperiodic SoftWrap chain, prefer extending
    /// the suffix back to the preceding HardBreak (within an extend budget) so
    /// `start_col` is 0 and resize does not walk retained wrap runs. If the
    /// chain is too large to extend, fall back to cols-dependent spine occupancy.
    pub(crate) fn eager_resize_suffix(
        &self,
        cols: u16,
        visual_rows: usize,
    ) -> (Vec<HistoryLine>, Option<HistoryAnchor>, usize) {
        let budget_cells = usize::from(cols.max(1)).saturating_mul(visual_rows.max(1));
        let extend_budget = budget_cells.saturating_mul(APERIODIC_EXTEND_CELL_FACTOR);
        let mut collected: Vec<HistoryLine> = Vec::new();
        let mut cells = 0usize;
        let mut omitted_joins = false;
        let mut entries = self.reverse_entries();
        while let Some(entry) = entries.next() {
            if cells >= budget_cells {
                if entry.break_after() != HistoryBreakAfter::SoftWrap {
                    break;
                }
                let from_probe = collected.last().map(|line: &HistoryLine| HistoryAnchor {
                    line_id: line.line_id,
                    unit_offset: line.start_offset,
                });
                let aperiodic_long = from_probe
                    .and_then(|from| self.wrap_chain_containing(from))
                    .is_some_and(|chain| {
                        wrap_chain_pattern(chain).is_none()
                            && chain.runs.len() > APERIODIC_INLINE_RUN_BOUND
                    });
                if !aperiodic_long {
                    omitted_joins = true;
                    break;
                }
                // Extend through older SoftWrap members until HardBreak or budget.
                let mut cur = entry;
                let mut reached_hard_boundary = false;
                loop {
                    let add = cur
                        .to_owned_line()
                        .units
                        .iter()
                        .map(|unit| usize::from(unit.width.max(1)))
                        .sum::<usize>();
                    if cells.saturating_add(add) > extend_budget && !collected.is_empty() {
                        omitted_joins = true;
                        break;
                    }
                    collected.push(cur.to_owned_line());
                    cells = cells.saturating_add(add);
                    match entries.next() {
                        None => {
                            reached_hard_boundary = true;
                            break;
                        }
                        Some(next) if next.break_after() == HistoryBreakAfter::SoftWrap => {
                            cur = next;
                        }
                        Some(_) => {
                            // Older HardBreak ends the previous paragraph; soft
                            // chain starts at the lines already collected.
                            reached_hard_boundary = true;
                            break;
                        }
                    }
                }
                if reached_hard_boundary {
                    omitted_joins = false;
                }
                break;
            }
            collected.push(entry.to_owned_line());
            cells = cells.saturating_add(
                collected
                    .last()
                    .map(|line| {
                        line.units
                            .iter()
                            .map(|unit| usize::from(unit.width.max(1)))
                            .sum()
                    })
                    .unwrap_or(0),
            );
        }
        collected.reverse();
        let from = collected.first().map(|line| HistoryAnchor {
            line_id: line.line_id,
            unit_offset: line.start_offset,
        });
        let start_col = if omitted_joins {
            self.wrap_column_before(from, cols)
        } else {
            0
        };
        (collected, from, start_col)
    }

    /// Display column at `from` for a new width. Repeating mixed-width
    /// patterns use a closed-form occupancy path; suffix trims do not scan
    /// retained fragments. If the derived wrap index was dropped, occupancy is
    /// rebuilt from the current SoftWrap chain only (SPEC-010 §9.1).
    pub(crate) fn wrap_column_before(&self, from: Option<HistoryAnchor>, cols: u16) -> usize {
        let Some(from) = from else {
            return 0;
        };
        let width = usize::from(cols);
        if width == 0 {
            return 0;
        }
        if let Some(chain) = self.wrap_chain_containing(from) {
            return wrap_occupancy_indexed(chain, width, chain.units_before(from));
        }
        wrap_occupancy_from_canonical(self, from, width)
    }

    fn wrap_chain_containing(&self, from: HistoryAnchor) -> Option<&WrapChain> {
        let index = self.wrap_chains.partition_point(|chain| {
            chain.fragments.first().is_some_and(|first| {
                first.line_id < from.line_id
                    || (first.line_id == from.line_id && first.start_offset < from.unit_offset)
            })
        });
        index
            .checked_sub(1)
            .and_then(|index| self.wrap_chains.get(index))
    }

    fn extend_wrap_line(&mut self, line: &HistoryLine) {
        let continue_chain = self.wrap_chains.back().is_some_and(|chain| chain.open);
        if !continue_chain && line.break_after == HistoryBreakAfter::HardBreak {
            return;
        }
        if line.units.is_empty() {
            if continue_chain && line.break_after == HistoryBreakAfter::HardBreak {
                if let Some(chain) = self.wrap_chains.back_mut() {
                    chain.open = false;
                }
            }
            return;
        }
        if !continue_chain {
            self.wrap_chains.push_back(WrapChain::default());
        }
        let chain = self
            .wrap_chains
            .back_mut()
            .expect("wrap chain exists after open-or-push");
        let unit_count = u32::try_from(line.units.len()).unwrap_or(u32::MAX);
        let prefix_units = chain.fragments.last().map_or(0, |fragment| {
            fragment.prefix_units.saturating_add(fragment.unit_count)
        });
        chain.fragments.push(WrapFragment {
            line_id: line.line_id,
            start_offset: line.start_offset,
            unit_count,
            prefix_units,
        });
        let run_units = wrap_runs_unit_sum(&chain.runs);
        let compacted = wrap_chain_is_compacted(chain, run_units, prefix_units);
        if compacted {
            append_compacted_wrap_runs(chain, &line.units, prefix_units);
        } else {
            for unit in &line.units {
                let width = unit.width.max(1);
                let pushed_new = if let Some((last_width, count)) = chain.runs.last_mut()
                    && *last_width == width
                {
                    *count = count.saturating_add(1);
                    false
                } else {
                    chain.runs.push((width, 1));
                    true
                };
                note_appended_wrap_run(chain, pushed_new);
            }
            compact_repeating_wrap_runs(chain);
        }
        chain.open = line.break_after == HistoryBreakAfter::SoftWrap;
        self.enforce_wrap_index_cap();
    }

    fn trim_wrap_suffix_from(&mut self, from: HistoryAnchor) {
        while let Some(chain) = self.wrap_chains.back() {
            match chain.fragments.first() {
                None => {
                    self.wrap_chains.pop_back();
                }
                Some(first)
                    if first.line_id > from.line_id
                        || (first.line_id == from.line_id
                            && first.start_offset >= from.unit_offset) =>
                {
                    self.wrap_chains.pop_back();
                }
                _ => break,
            }
        }
        let Some(chain) = self.wrap_chains.back_mut() else {
            return;
        };
        let keep_units = chain.units_before(from);
        let total = chain.fragments.last().map_or(0, |fragment| {
            fragment.prefix_units.saturating_add(fragment.unit_count)
        });
        if keep_units >= total {
            return;
        }
        let split = chain.fragments.partition_point(|fragment| {
            fragment.line_id < from.line_id
                || (fragment.line_id == from.line_id && fragment.start_offset < from.unit_offset)
        });
        chain.fragments.truncate(split);
        if let Some(last) = chain.fragments.last_mut()
            && last.line_id == from.line_id
            && last.start_offset < from.unit_offset
        {
            last.unit_count = from
                .unit_offset
                .saturating_sub(last.start_offset)
                .min(last.unit_count);
        }
        let run_units = wrap_runs_unit_sum(&chain.runs);
        let compacted = wrap_chain_is_compacted(chain, run_units, total);
        if compacted {
            if keep_units == 0 {
                chain.runs.clear();
                chain.pattern_len = 0;
                chain.pattern_phase = 0;
                chain.invalidate_spine();
            }
        } else {
            drop_suffix_wrap_runs(&mut chain.runs, total.saturating_sub(keep_units));
            note_pattern_after_suffix_trim(chain);
            chain.invalidate_spine();
        }
        chain.open = true;
    }

    fn trim_wrap_before_retained(&mut self) {
        let Some(first) = self.entries().next() else {
            self.wrap_chains.clear();
            return;
        };
        let keep = HistoryAnchor {
            line_id: first.line_id(),
            unit_offset: first.start_offset(),
        };
        while let Some(chain) = self.wrap_chains.front() {
            match chain.fragments.last() {
                None => {
                    self.wrap_chains.pop_front();
                }
                Some(last)
                    if last.line_id < keep.line_id
                        || (last.line_id == keep.line_id
                            && last.start_offset.saturating_add(last.unit_count)
                                <= keep.unit_offset) =>
                {
                    self.wrap_chains.pop_front();
                }
                _ => break,
            }
        }
        let Some(chain) = self.wrap_chains.front_mut() else {
            return;
        };
        let drop_units = chain.units_before(keep);
        let split = chain.fragments.partition_point(|fragment| {
            fragment.line_id < keep.line_id
                || (fragment.line_id == keep.line_id && fragment.start_offset < keep.unit_offset)
        });
        if split > 0 {
            chain.fragments.drain(0..split);
        }
        let mut prefix = 0u32;
        for fragment in &mut chain.fragments {
            fragment.prefix_units = prefix;
            prefix = prefix.saturating_add(fragment.unit_count);
        }
        let run_units = wrap_runs_unit_sum(&chain.runs);
        let compacted =
            wrap_chain_is_compacted(chain, run_units, drop_units.saturating_add(prefix));
        if compacted {
            if prefix == 0 {
                chain.runs.clear();
                chain.pattern_len = 0;
                chain.pattern_phase = 0;
            } else {
                chain.pattern_phase = chain.pattern_phase.saturating_add(drop_units);
            }
            chain.invalidate_spine();
        } else {
            let period_units = wrap_pattern_period_units(chain);
            drop_prefix_wrap_runs(&mut chain.runs, drop_units);
            note_pattern_after_prefix_trim(chain, drop_units, period_units);
            chain.invalidate_spine();
        }
    }

    fn enforce_wrap_index_cap(&mut self) {
        for chain in &mut self.wrap_chains {
            compact_repeating_wrap_runs(chain);
        }
        while self.wrap_index_allocated_bytes() > HISTORY_PER_EXECUTION_DERIVED_INDEX_CAP
            && self.wrap_chains.len() > 1
        {
            let drop_closed = self.wrap_chains.front().is_some_and(|chain| !chain.open);
            if !drop_closed {
                break;
            }
            self.wrap_chains.pop_front();
        }
        if self.wrap_index_allocated_bytes() > HISTORY_PER_EXECUTION_DERIVED_INDEX_CAP {
            self.wrap_chains.clear();
            self.wrap_chains.shrink_to_fit();
        }
    }

    fn wrap_index_allocated_bytes(&self) -> usize {
        const RUN: usize = size_of::<(u8, u32)>();
        self.wrap_chains
            .capacity()
            .saturating_mul(size_of::<WrapChain>())
            .saturating_add(
                self.wrap_chains
                    .iter()
                    .map(|chain| {
                        let spine_bytes = chain.spine.borrow().as_ref().map_or(0, |spine| {
                            spine
                                .after_run
                                .capacity()
                                .saturating_add(spine.units_after.capacity().saturating_mul(4))
                        });
                        chain
                            .fragments
                            .capacity()
                            .saturating_mul(size_of::<WrapFragment>())
                            .saturating_add(chain.runs.capacity().saturating_mul(RUN))
                            .saturating_add(spine_bytes)
                    })
                    .sum::<usize>(),
            )
    }

    /// Drop source units at/after `from` without rewriting older segments.
    pub(crate) fn truncate_from(&mut self, from: HistoryAnchor) {
        self.reflow_cache.get_mut().take();
        while let Some(last) = self.tail.last() {
            if line_entirely_before(last, from) {
                break;
            }
            if last.line_id == from.line_id && last.start_offset < from.unit_offset {
                let keep = from
                    .unit_offset
                    .saturating_sub(last.start_offset)
                    .try_into()
                    .unwrap_or(usize::MAX)
                    .min(last.units.len());
                if keep == 0 {
                    self.tail.pop();
                } else if let Some(last) = self.tail.last_mut() {
                    last.units.truncate(keep);
                    last.break_after = HistoryBreakAfter::SoftWrap;
                }
                self.recount_tail();
                self.update_resident_bytes();
                self.trim_wrap_suffix_from(from);
                return;
            }
            self.tail.pop();
        }
        self.recount_tail();
        loop {
            let Some(segment) = self.segments.back() else {
                break;
            };
            let Some(last_line) = segment.lines.last() else {
                self.segments.pop_back();
                continue;
            };
            let last_end = last_line.start_offset.saturating_add(last_line.unit_len);
            if last_line.line_id < from.line_id
                || (last_line.line_id == from.line_id && last_end <= from.unit_offset)
            {
                break;
            }
            let segment = self.segments.pop_back().expect("back existed");
            self.segments_resident_bytes = self
                .segments_resident_bytes
                .saturating_sub(segment.resident_bytes);
            let mut keep = Vec::new();
            let mut reached_split = false;
            for line in segment.lines.iter() {
                let owned = HistoryLineRef::Sealed(&segment, line).to_owned_line();
                if line_entirely_before(&owned, from) {
                    keep.push(owned);
                    continue;
                }
                if owned.line_id == from.line_id && owned.start_offset < from.unit_offset {
                    let count = from
                        .unit_offset
                        .saturating_sub(owned.start_offset)
                        .try_into()
                        .unwrap_or(usize::MAX)
                        .min(owned.units.len());
                    if count > 0 {
                        keep.push(HistoryLine {
                            line_id: owned.line_id,
                            units: owned.units[..count].to_vec(),
                            break_after: HistoryBreakAfter::SoftWrap,
                            start_offset: owned.start_offset,
                        });
                    }
                }
                reached_split = true;
                break;
            }
            for line in keep {
                self.push_fragment_inner(line, false);
            }
            if reached_split {
                break;
            }
        }
        self.recount_tail();
        self.update_resident_bytes();
        self.trim_wrap_suffix_from(from);
    }

    fn recount_tail(&mut self) {
        self.tail_payload_bytes = self
            .tail
            .iter()
            .map(HistoryLine::canonical_payload_len)
            .sum();
        self.tail_resident_bytes = self
            .tail
            .iter()
            .map(HistoryLine::allocated_bytes)
            .sum::<usize>()
            .saturating_add(
                self.tail
                    .capacity()
                    .saturating_mul(size_of::<HistoryLine>()),
            );
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
        self.push_fragment_inner(line, true);
    }

    fn push_fragment_inner(&mut self, line: HistoryLine, record_wrap: bool) {
        let bytes = line.canonical_payload_len();
        if !self.tail.is_empty()
            && (self.tail_payload_bytes + bytes > HISTORY_SEGMENT_PAYLOAD_TARGET
                || self.tail_resident_bytes >= HISTORY_TAIL_PAYLOAD_LIMIT)
        {
            self.seal_tail();
        }
        self.tail_payload_bytes += bytes;
        let line_resident_bytes = line.allocated_bytes();
        let old_capacity = self.tail.capacity();
        if record_wrap {
            self.extend_wrap_line(&line);
        }
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
        if self.tail_payload_bytes >= HISTORY_SEGMENT_PAYLOAD_TARGET
            || self.tail_resident_bytes >= HISTORY_TAIL_PAYLOAD_LIMIT
        {
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
        let mut evicted = false;
        while self.resident_bytes > HISTORY_PER_EXECUTION_BYTE_CAP {
            if self.segments.is_empty() {
                if self.tail.is_empty() {
                    break;
                }
                self.seal_tail();
                if self.segments.is_empty() {
                    break;
                }
            }
            let Some(segment) = self.segments.pop_front() else {
                break;
            };
            self.record_evicted_segment(&segment);
            self.segments_resident_bytes = self
                .segments_resident_bytes
                .saturating_sub(segment.resident_bytes);
            self.update_resident_bytes();
            self.eviction_generation = self.eviction_generation.wrapping_add(1);
            evicted = true;
        }
        if evicted {
            self.trim_wrap_before_retained();
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
        self.trim_wrap_before_retained();
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
        self.reflow_from(cols, max_rows, 0)
    }

    pub(crate) fn reflow_from(
        &self,
        cols: u16,
        max_rows: usize,
        start_col: usize,
    ) -> Vec<ReflowRow> {
        let generation = self.eviction_generation;
        if start_col == 0 {
            if let Some(cache) = self.reflow_cache.borrow().as_ref()
                && (cache.columns, cache.max_rows, cache.eviction_generation)
                    == (cols, max_rows, generation)
            {
                return cache.rows.clone();
            }
        }
        let rows = self.reflow_uncached_from(cols, max_rows, start_col);
        if start_col == 0 {
            let estimated = reflow_rows_allocated_bytes(&rows);
            if estimated <= HISTORY_PER_EXECUTION_DERIVED_INDEX_CAP {
                *self.reflow_cache.borrow_mut() = Some(ReflowCache {
                    columns: cols,
                    max_rows,
                    eviction_generation: generation,
                    rows: rows.clone(),
                });
            }
        }
        rows
    }

    pub(crate) fn reflow_uncached(&self, cols: u16, max_rows: usize) -> Vec<ReflowRow> {
        self.reflow_uncached_from(cols, max_rows, 0)
    }

    pub(crate) fn reflow_uncached_from(
        &self,
        cols: u16,
        max_rows: usize,
        start_col: usize,
    ) -> Vec<ReflowRow> {
        if cols == 0 || max_rows == 0 {
            return Vec::new();
        }
        let width = usize::from(cols);
        let mut rows = Vec::new();
        let mut current = ReflowRow::default();
        let mut used = start_col.min(width);
        let mut previous_break = None;
        for line in self.entries() {
            let joins = previous_break == Some(HistoryBreakAfter::SoftWrap);
            if !joins && !current.cells.is_empty() {
                rows.push(std::mem::take(&mut current));
                used = 0;
                if rows.len() >= max_rows {
                    break;
                }
            }
            for (unit_index, unit) in line.units().enumerate() {
                let unit_width = usize::from(unit.width().max(1));
                if used > 0 && used + unit_width > width {
                    if !current.cells.is_empty() {
                        current.break_after = Some(HistoryBreakAfter::SoftWrap);
                        rows.push(std::mem::take(&mut current));
                        if rows.len() >= max_rows {
                            return rows;
                        }
                    }
                    used = 0;
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
                    used = 0;
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
                used = used.saturating_add(unit_width);
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
                used = 0;
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

const WRAP_PATTERN_MAX: usize = 8;
/// Aperiodic SoftWrap chains above this run count prefer HardBreak extension
/// (or spine occupancy) instead of a linear mid-chain run walk on resize.
const APERIODIC_INLINE_RUN_BOUND: usize = 64;
/// Max cells cloned when extending an aperiodic SoftWrap cut back to HardBreak.
const APERIODIC_EXTEND_CELL_FACTOR: usize = 8;

fn wrap_pattern_period_units(chain: &WrapChain) -> u32 {
    let Some(pattern) = wrap_chain_pattern(chain) else {
        return 0;
    };
    wrap_runs_unit_sum(pattern)
}

fn wrap_chain_pattern(chain: &WrapChain) -> Option<&[(u8, u32)]> {
    let k = chain.pattern_len as usize;
    if chain.pattern_len >= 2 && chain.runs.len() >= k {
        Some(&chain.runs[..k])
    } else {
        None
    }
}

fn wrap_chain_is_compacted(chain: &WrapChain, run_units: u32, virtual_units: u32) -> bool {
    let k = chain.pattern_len as usize;
    chain.pattern_len >= 2 && k > 0 && chain.runs.len() <= k && run_units < virtual_units
}

fn wrap_runs_unit_sum(runs: &[(u8, u32)]) -> u32 {
    runs.iter()
        .map(|(_, count)| *count)
        .fold(0u32, u32::saturating_add)
}

fn compact_repeating_wrap_runs(chain: &mut WrapChain) {
    let k = chain.pattern_len as usize;
    if chain.pattern_len >= 2 && chain.runs.len() > k {
        chain.invalidate_spine();
        chain.runs.truncate(k);
        chain.runs.shrink_to_fit();
    }
}

fn wrap_pattern_width_at(pattern: &[(u8, u32)], mut index: u32) -> Option<u8> {
    let period = wrap_runs_unit_sum(pattern);
    if period == 0 {
        return None;
    }
    index %= period;
    for &(width, count) in pattern {
        if index < count {
            return Some(width);
        }
        index = index.saturating_sub(count);
    }
    None
}

fn wrap_pattern_slice(pattern: &[(u8, u32)], start: u32, count: u32) -> Vec<(u8, u32)> {
    let period = wrap_runs_unit_sum(pattern);
    if period == 0 || count == 0 {
        return Vec::new();
    }
    let mut offset = start % period;
    let mut remaining = count;
    let mut runs: Vec<(u8, u32)> = Vec::new();
    while remaining > 0 {
        let before = remaining;
        let mut idx = 0u32;
        for &(width, run_count) in pattern {
            if remaining == 0 {
                break;
            }
            let run_end = idx.saturating_add(run_count);
            if offset >= run_end {
                idx = run_end;
                continue;
            }
            let skip = offset.saturating_sub(idx);
            let take = run_count.saturating_sub(skip).min(remaining);
            if take > 0 {
                if let Some((last_width, last_count)) = runs.last_mut()
                    && *last_width == width
                {
                    *last_count = last_count.saturating_add(take);
                } else {
                    runs.push((width, take));
                }
                remaining = remaining.saturating_sub(take);
                offset = 0;
            }
            idx = run_end;
        }
        if remaining == before {
            break;
        }
    }
    runs
}

fn append_compacted_wrap_runs(chain: &mut WrapChain, units: &[HistoryUnit], prefix_units: u32) {
    for (index, unit) in units.iter().enumerate() {
        let width = unit.width.max(1);
        let unit_index = chain
            .pattern_phase
            .saturating_add(prefix_units)
            .saturating_add(index as u32);
        match wrap_pattern_width_at(&chain.runs, unit_index) {
            Some(expected) if expected == width => {}
            _ => {
                chain.invalidate_spine();
                chain.runs = wrap_pattern_slice(
                    &chain.runs,
                    chain.pattern_phase,
                    prefix_units.saturating_add(index as u32),
                );
                chain.pattern_phase = 0;
                chain.pattern_len = 0;
                for unit in &units[index..] {
                    let width = unit.width.max(1);
                    if let Some((last_width, count)) = chain.runs.last_mut()
                        && *last_width == width
                    {
                        *count = count.saturating_add(1);
                    } else {
                        chain.runs.push((width, 1));
                    }
                }
                return;
            }
        }
    }
}

fn wrap_occupancy_repeating_from(
    pattern: &[(u8, u32)],
    phase: u32,
    cols: usize,
    units: u32,
) -> usize {
    let period = wrap_runs_unit_sum(pattern);
    if period == 0 {
        return 0;
    }
    let phase = phase % period;
    if phase == 0 {
        return wrap_occupancy_repeating(pattern, cols, units);
    }
    let rotated = wrap_pattern_slice(pattern, phase, period);
    wrap_occupancy_repeating(&rotated, cols, units)
}

fn wrap_occupancy_indexed(chain: &WrapChain, cols: usize, units: u32) -> usize {
    if let Some(pattern) = wrap_chain_pattern(chain) {
        wrap_occupancy_repeating_from(pattern, chain.pattern_phase, cols, units)
    } else {
        wrap_occupancy_aperiodic(chain, cols, units)
    }
}

fn wrap_occupancy_aperiodic(chain: &WrapChain, cols: usize, units: u32) -> usize {
    if cols == 0 || units == 0 || chain.runs.is_empty() {
        return 0;
    }
    if chain.runs.len() > APERIODIC_INLINE_RUN_BOUND {
        let cols_u16 = u16::try_from(cols).unwrap_or(u16::MAX);
        ensure_aperiodic_spine(chain, cols_u16);
        if let Some(spine) = chain.spine.borrow().as_ref() {
            return spine_occupancy(spine, &chain.runs, cols, units);
        }
    }
    wrap_occupancy_for_runs(&chain.runs, cols, units)
}

fn ensure_aperiodic_spine(chain: &WrapChain, cols: u16) {
    {
        let spine = chain.spine.borrow();
        if let Some(existing) = spine.as_ref()
            && existing.cols == cols
            && existing.run_len == chain.runs.len()
        {
            return;
        }
    }
    let width = usize::from(cols);
    let mut after_run = Vec::with_capacity(chain.runs.len());
    let mut units_after = Vec::with_capacity(chain.runs.len());
    let mut used = 0usize;
    let mut total_units = 0u32;
    let mut seen = vec![None; width.saturating_add(1)];
    for &(unit_width, count) in &chain.runs {
        used = wrap_occupancy_run(used, width, unit_width, count, &mut seen);
        total_units = total_units.saturating_add(count);
        after_run.push(u8::try_from(used.min(width)).unwrap_or(u8::MAX));
        units_after.push(total_units);
    }
    *chain.spine.borrow_mut() = Some(WrapOccupancySpine {
        cols,
        run_len: chain.runs.len(),
        after_run,
        units_after,
    });
}

fn spine_occupancy(
    spine: &WrapOccupancySpine,
    runs: &[(u8, u32)],
    cols: usize,
    unit_limit: u32,
) -> usize {
    if unit_limit == 0 || cols == 0 {
        return 0;
    }
    let fully = spine
        .units_after
        .partition_point(|&units| units <= unit_limit);
    let (mut used, consumed) = if fully == 0 {
        (0usize, 0u32)
    } else {
        (
            usize::from(spine.after_run[fully - 1]),
            spine.units_after[fully - 1],
        )
    };
    if consumed >= unit_limit {
        return used.min(cols);
    }
    if fully < runs.len() {
        let (width, count) = runs[fully];
        let take = count.min(unit_limit.saturating_sub(consumed));
        let mut seen = vec![None; cols.saturating_add(1)];
        used = wrap_occupancy_run(used, cols, width, take, &mut seen);
    }
    used.min(cols)
}

/// Incremental period-k detection for k in 2..=WRAP_PATTERN_MAX.
/// Must not walk retained runs (SPEC-010 §7).
fn note_appended_wrap_run(chain: &mut WrapChain, pushed_new: bool) {
    chain.invalidate_spine();
    let n = chain.runs.len();
    if n <= 1 {
        chain.pattern_len = n as u32;
        return;
    }
    if chain.pattern_len >= 2 {
        let k = chain.pattern_len as usize;
        if n <= k {
            return;
        }
        if wrap_run_extends_pattern(chain, k, pushed_new) {
            return;
        }
        if n <= WRAP_PATTERN_MAX {
            chain.pattern_len = n as u32;
            return;
        }
        chain.pattern_len = 0;
        return;
    }
    if n <= WRAP_PATTERN_MAX {
        chain.pattern_len = n as u32;
    }
}

fn wrap_run_extends_pattern(chain: &WrapChain, k: usize, pushed_new: bool) -> bool {
    let n = chain.runs.len();
    if k == 0 || n <= k {
        return true;
    }
    if pushed_new {
        let prev = n - 2;
        if chain.runs[prev] != chain.runs[prev % k] {
            return false;
        }
    }
    let last = n - 1;
    let template = chain.runs[last % k];
    let run = chain.runs[last];
    run.0 == template.0 && run.1 <= template.1
}

fn note_pattern_after_suffix_trim(chain: &mut WrapChain) {
    let n = chain.runs.len();
    if n <= 1 {
        chain.pattern_len = n as u32;
        return;
    }
    if chain.pattern_len >= 2 {
        let k = chain.pattern_len as usize;
        if n <= k {
            chain.pattern_len = n as u32;
            return;
        }
        let last = n - 1;
        let template = chain.runs[last % k];
        let run = chain.runs[last];
        if run.0 != template.0 || run.1 > template.1 {
            chain.pattern_len = if n <= WRAP_PATTERN_MAX { n as u32 } else { 0 };
        }
        return;
    }
    if n <= WRAP_PATTERN_MAX {
        chain.pattern_len = n as u32;
    }
}

fn note_pattern_after_prefix_trim(chain: &mut WrapChain, drop_units: u32, period_units: u32) {
    let n = chain.runs.len();
    if n <= 1 {
        chain.pattern_len = n as u32;
        return;
    }
    if n <= WRAP_PATTERN_MAX && n <= chain.pattern_len as usize {
        chain.pattern_len = n as u32;
        return;
    }
    if chain.pattern_len >= 2 && period_units > 0 && drop_units % period_units == 0 {
        return;
    }
    chain.pattern_len = 0;
}

fn drop_suffix_wrap_runs(runs: &mut Vec<(u8, u32)>, drop_units: u32) {
    let mut remaining = drop_units;
    while remaining > 0 {
        let Some(last) = runs.last_mut() else {
            break;
        };
        if last.1 <= remaining {
            remaining = remaining.saturating_sub(last.1);
            runs.pop();
        } else {
            last.1 = last.1.saturating_sub(remaining);
            break;
        }
    }
}

fn drop_prefix_wrap_runs(runs: &mut Vec<(u8, u32)>, drop_units: u32) {
    let mut remaining = drop_units;
    let mut drain = 0usize;
    while remaining > 0 && drain < runs.len() {
        let count = runs[drain].1;
        if count <= remaining {
            remaining = remaining.saturating_sub(count);
            drain += 1;
        } else {
            runs[drain].1 = count.saturating_sub(remaining);
            remaining = 0;
        }
    }
    if drain > 0 {
        runs.drain(..drain);
    }
}

fn wrap_advance(used: usize, cols: usize, unit_width: u8) -> usize {
    let unit_width = usize::from(unit_width.max(1));
    let mut used = used;
    if used > 0 && used + unit_width > cols {
        used = 0;
    }
    if unit_width > cols {
        return 0;
    }
    used = used.saturating_add(unit_width);
    if used >= cols {
        0
    } else {
        used
    }
}

fn wrap_occupancy_run(
    mut used: usize,
    cols: usize,
    unit_width: u8,
    count: u32,
    seen: &mut [Option<usize>],
) -> usize {
    if cols == 0 {
        return 0;
    }
    let mut remaining = count as usize;
    if remaining == 0 {
        return used.min(cols);
    }
    seen.fill(None);
    while remaining > 0 {
        if used <= cols
            && let Some(remaining_then) = seen[used]
        {
            let cycle = remaining_then.saturating_sub(remaining);
            if cycle > 0 {
                remaining %= cycle;
                if remaining == 0 {
                    return used.min(cols);
                }
                seen.fill(None);
                continue;
            }
        } else if used <= cols {
            seen[used] = Some(remaining);
        }
        used = wrap_advance(used, cols, unit_width);
        remaining -= 1;
    }
    used.min(cols)
}

fn line_wholly_after(entry: HistoryLineRef<'_>, from: HistoryAnchor) -> bool {
    entry.line_id() > from.line_id
        || (entry.line_id() == from.line_id && entry.start_offset() >= from.unit_offset)
}

fn line_wholly_before(entry: HistoryLineRef<'_>, from: HistoryAnchor) -> bool {
    let end = entry.start_offset().saturating_add(entry.unit_len());
    entry.line_id() < from.line_id || (entry.line_id() == from.line_id && end <= from.unit_offset)
}

/// Occupancy of the SoftWrap chain containing `from` when the derived wrap
/// index is absent. Walks only that chain, not all retained history.
fn wrap_occupancy_from_canonical(store: &HistoryStore, from: HistoryAnchor, cols: usize) -> usize {
    let mut fragments_rev: Vec<Vec<(u8, u32)>> = Vec::new();
    let mut saw_soft = false;
    for entry in store.reverse_entries() {
        if line_wholly_after(entry, from) {
            continue;
        }
        if !fragments_rev.is_empty()
            && entry.break_after() == HistoryBreakAfter::HardBreak
            && line_wholly_before(entry, from)
        {
            break;
        }
        if entry.break_after() == HistoryBreakAfter::SoftWrap {
            saw_soft = true;
        }
        let mut fragment_runs: Vec<(u8, u32)> = Vec::new();
        let mut offset = entry.start_offset();
        for unit in entry.units() {
            if entry.line_id() == from.line_id && offset >= from.unit_offset {
                break;
            }
            let width = unit.width().max(1);
            if let Some((last_width, count)) = fragment_runs.last_mut()
                && *last_width == width
            {
                *count = count.saturating_add(1);
            } else {
                fragment_runs.push((width, 1));
            }
            offset = offset.saturating_add(1);
        }
        fragments_rev.push(fragment_runs);
    }
    if !saw_soft {
        return 0;
    }
    let mut runs: Vec<(u8, u32)> = Vec::new();
    for fragment in fragments_rev.into_iter().rev() {
        for (width, count) in fragment {
            if let Some((last_width, last_count)) = runs.last_mut()
                && *last_width == width
            {
                *last_count = last_count.saturating_add(count);
            } else {
                runs.push((width, count));
            }
        }
    }
    let units = wrap_runs_unit_sum(&runs);
    // Same closed-form / ephemeral-spine path as the indexed miss-free case so
    // dropping the derived wrap index cannot reintroduce a linear run walk.
    wrap_occupancy_for_runs(&runs, cols, units)
}

fn wrap_runs_match_period(runs: &[(u8, u32)], k: usize) -> bool {
    if k < 2 || runs.len() < k {
        return false;
    }
    for (index, run) in runs.iter().enumerate().skip(k) {
        let template = runs[index % k];
        if run.0 != template.0 {
            return false;
        }
        if index + 1 == runs.len() {
            if run.1 > template.1 {
                return false;
            }
        } else if run.1 != template.1 {
            return false;
        }
    }
    true
}

fn wrap_repeating_period(runs: &[(u8, u32)]) -> Option<usize> {
    let n = runs.len();
    if n < 2 {
        return None;
    }
    let max_k = n.min(WRAP_PATTERN_MAX);
    (2..=max_k).find(|&k| n > k && wrap_runs_match_period(runs, k))
}

fn wrap_occupancy_for_runs(runs: &[(u8, u32)], cols: usize, unit_limit: u32) -> usize {
    if let Some(k) = wrap_repeating_period(runs) {
        wrap_occupancy_repeating(&runs[..k], cols, unit_limit)
    } else if runs.len() > APERIODIC_INLINE_RUN_BOUND {
        wrap_occupancy_with_ephemeral_spine(runs, cols, unit_limit)
    } else {
        wrap_occupancy_runs(runs, cols, unit_limit)
    }
}

fn wrap_occupancy_with_ephemeral_spine(runs: &[(u8, u32)], cols: usize, unit_limit: u32) -> usize {
    if cols == 0 || unit_limit == 0 || runs.is_empty() {
        return 0;
    }
    let cols_u16 = u16::try_from(cols).unwrap_or(u16::MAX);
    let mut after_run = Vec::with_capacity(runs.len());
    let mut units_after = Vec::with_capacity(runs.len());
    let mut used = 0usize;
    let mut total_units = 0u32;
    let mut seen = vec![None; cols.saturating_add(1)];
    for &(unit_width, count) in runs {
        used = wrap_occupancy_run(used, cols, unit_width, count, &mut seen);
        total_units = total_units.saturating_add(count);
        after_run.push(u8::try_from(used.min(cols)).unwrap_or(u8::MAX));
        units_after.push(total_units);
    }
    let spine = WrapOccupancySpine {
        cols: cols_u16,
        run_len: runs.len(),
        after_run,
        units_after,
    };
    spine_occupancy(&spine, runs, cols, unit_limit)
}

fn wrap_occupancy_runs(runs: &[(u8, u32)], cols: usize, unit_limit: u32) -> usize {
    let mut used = 0usize;
    let mut left = unit_limit;
    let mut seen = vec![None; cols.saturating_add(1)];
    for &(width, count) in runs {
        if left == 0 {
            break;
        }
        let take = count.min(left);
        used = wrap_occupancy_run(used, cols, width, take, &mut seen);
        left = left.saturating_sub(take);
    }
    used.min(cols)
}

fn wrap_occupancy_repeating(pattern: &[(u8, u32)], cols: usize, unit_limit: u32) -> usize {
    let pattern_units = pattern
        .iter()
        .map(|(_, count)| *count)
        .fold(0u32, u32::saturating_add);
    if cols == 0 || pattern_units == 0 || unit_limit == 0 {
        return 0;
    }
    let mut used = 0usize;
    let mut remaining = unit_limit;
    let mut seen: Vec<Option<u32>> = vec![None; cols.saturating_add(1)];
    let mut scratch: Vec<Option<usize>> = vec![None; cols.saturating_add(1)];
    while remaining > 0 {
        if used <= cols
            && let Some(remaining_then) = seen[used]
        {
            let cycle = remaining_then.saturating_sub(remaining);
            if cycle > 0 {
                remaining %= cycle;
                seen.fill(None);
                continue;
            }
        } else if used <= cols {
            seen[used] = Some(remaining);
        }
        let take = remaining.min(pattern_units);
        used = wrap_occupancy_pattern_once(used, pattern, cols, take, &mut scratch);
        remaining = remaining.saturating_sub(take);
    }
    used.min(cols)
}

fn wrap_occupancy_pattern_once(
    mut used: usize,
    pattern: &[(u8, u32)],
    cols: usize,
    unit_limit: u32,
    seen: &mut [Option<usize>],
) -> usize {
    let mut left = unit_limit;
    for &(width, count) in pattern {
        if left == 0 {
            break;
        }
        let take = count.min(left);
        used = wrap_occupancy_run(used, cols, width, take, seen);
        left = left.saturating_sub(take);
    }
    used.min(cols)
}

#[cfg(test)]
fn wrap_occupancy(unit_widths: &[u8], cols: usize) -> usize {
    if cols == 0 {
        return 0;
    }
    wrap_occupancy_runs(
        &unit_widths
            .iter()
            .map(|&width| (width.max(1), 1u32))
            .collect::<Vec<_>>(),
        cols,
        u32::try_from(unit_widths.len()).unwrap_or(u32::MAX),
    )
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

    fn ascii_fragment(
        id: u64,
        start_offset: u32,
        text: &str,
        break_after: HistoryBreakAfter,
    ) -> HistoryLine {
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
            start_offset,
        }
    }

    #[test]
    fn wrap_column_before_walks_only_the_current_soft_wrap_chain() {
        let mut store = HistoryStore::default();
        for id in 0..8_000 {
            store.append_line(ascii_line(id, "xxxx", HistoryBreakAfter::HardBreak));
        }
        store.append_line(ascii_fragment(
            8_000,
            0,
            "abcde",
            HistoryBreakAfter::SoftWrap,
        ));
        store.append_line(ascii_fragment(8_000, 5, "fg", HistoryBreakAfter::HardBreak));
        let (suffix, from, start_col) = store.eager_resize_suffix(2, 1);
        assert!(
            suffix.len() <= 8,
            "eager suffix cloned {} lines including hard-broken prefix",
            suffix.len()
        );
        assert_eq!(from.unwrap().line_id, LineId(8_000));
        assert_eq!(from.unwrap().unit_offset, 5);
        assert_eq!(start_col, 1);
    }

    #[test]
    fn wrap_column_before_is_closed_form_for_a_resident_soft_wrap_chain() {
        let mut store = HistoryStore::default();
        for id in 0..8_000 {
            store.append_line(ascii_line(id, "x", HistoryBreakAfter::SoftWrap));
        }
        let chain = store.wrap_chains.back().expect("open wrap chain");
        assert_eq!(chain.runs, [(1, 8_000)]);
        assert_eq!(chain.fragments.len(), 8_000);
        assert_eq!(chain.fragments.last().unwrap().prefix_units, 7_999);
        let (suffix, from, start_col) = store.eager_resize_suffix(8, 6);
        assert!(
            suffix.len() <= 48,
            "eager suffix cloned {} lines from a 8000-unit SoftWrap chain",
            suffix.len()
        );
        assert_eq!(from.unwrap().line_id, LineId(8_000 - 48));
        assert_eq!(start_col, 0);
        assert_eq!(
            store.wrap_column_before(from, 8),
            wrap_occupancy(&[1; 7_952], 8)
        );
    }

    #[test]
    fn blank_rows_seal_and_stay_inside_the_resident_cap() {
        let mut store = HistoryStore::default();
        for id in 0..50_000 {
            store.append_line(HistoryLine {
                line_id: LineId(id),
                units: Vec::new(),
                break_after: HistoryBreakAfter::HardBreak,
                start_offset: 0,
            });
        }
        assert!(
            !store.segments.is_empty(),
            "blank rows must seal instead of remaining an unbounded tail"
        );
        assert!(store.tail_resident_bytes <= HISTORY_TAIL_PAYLOAD_LIMIT);
        assert!(store.resident_bytes <= HISTORY_PER_EXECUTION_BYTE_CAP);
        assert!(store.derived_cache_bytes() <= HISTORY_PER_EXECUTION_DERIVED_INDEX_CAP);
    }

    #[test]
    fn eager_resize_suffix_is_bounded_for_hard_broken_history() {
        let mut store = HistoryStore::default();
        for id in 0..8_000 {
            store.append_line(ascii_line(id, "x", HistoryBreakAfter::HardBreak));
        }
        let (suffix, from, start_col) = store.eager_resize_suffix(8, 6);
        assert!(
            suffix.len() <= 48,
            "eager suffix cloned {} lines from 8000 hard-broken rows",
            suffix.len()
        );
        assert!(suffix.len() < 8_000);
        assert_eq!(start_col, 0);
        assert!(from.expect("suffix").line_id.0 > 0);
        store.truncate_from(from.unwrap());
        assert!(matches!(
            store.resolve_anchor(HistoryAnchor {
                line_id: LineId(0),
                unit_offset: 0
            }),
            HistoryAnchorResolution::Resolved { .. }
        ));
    }

    #[test]
    fn wrap_index_is_derived_and_omits_closed_hard_broken_rows() {
        let mut store = HistoryStore::default();
        for id in 0..8_000 {
            store.append_line(ascii_line(id, "x", HistoryBreakAfter::HardBreak));
        }
        assert!(store.wrap_chains.is_empty());
        store.append_line(ascii_line(8_000, "x", HistoryBreakAfter::SoftWrap));
        let wrap = store.wrap_index_allocated_bytes();
        assert!(wrap > 0);
        assert!(store.derived_cache_bytes() >= wrap);
        store.append_line(ascii_line(8_001, "y", HistoryBreakAfter::HardBreak));
        let (_, from, start_col) = store.eager_resize_suffix(8, 6);
        store.truncate_from(from.unwrap());
        assert_eq!(start_col, 0);
        assert!(matches!(
            store.resolve_anchor(HistoryAnchor {
                line_id: LineId(0),
                unit_offset: 0
            }),
            HistoryAnchorResolution::Resolved { .. }
        ));
    }

    #[test]
    fn wrap_column_before_is_closed_form_for_alternating_widths() {
        let mut store = HistoryStore::default();
        let mut widths = Vec::new();
        for id in 0..8_000 {
            let width = if id % 2 == 0 { 1 } else { 2 };
            widths.push(width);
            store.append_line(HistoryLine {
                line_id: LineId(id),
                units: vec![HistoryUnit {
                    utf8: vec![b'x'],
                    width,
                    style: Style::default(),
                }],
                break_after: HistoryBreakAfter::SoftWrap,
                start_offset: 0,
            });
        }
        {
            let chain = store.wrap_chains.back().expect("open wrap chain");
            assert_eq!(chain.pattern_len, 2);
            assert_eq!(chain.runs.as_slice(), &[(1, 1), (2, 1)]);
        }
        let (suffix, from, start_col) = store.eager_resize_suffix(8, 6);
        assert!(
            suffix.len() <= 48,
            "eager suffix cloned {} lines from an 8000-unit mixed SoftWrap chain",
            suffix.len()
        );
        let from = from.expect("suffix cut");
        let prefix = from.line_id.0 as usize;
        assert_eq!(start_col, wrap_occupancy(&widths[..prefix], 8));
        assert_eq!(store.wrap_column_before(Some(from), 8), start_col);
        store.truncate_from(from);
        assert!(matches!(
            store.resolve_anchor(HistoryAnchor {
                line_id: LineId(0),
                unit_offset: 0
            }),
            HistoryAnchorResolution::Resolved { .. }
        ));
    }

    #[test]
    fn wrap_column_before_preserves_period_two_run_counts() {
        let mut store = HistoryStore::default();
        let mut widths = Vec::new();
        for id in 0..2_000 {
            widths.extend_from_slice(&[1, 1, 2]);
            store.append_line(HistoryLine {
                line_id: LineId(id),
                units: vec![
                    HistoryUnit {
                        utf8: vec![b'a'],
                        width: 1,
                        style: Style::default(),
                    },
                    HistoryUnit {
                        utf8: vec![b'b'],
                        width: 1,
                        style: Style::default(),
                    },
                    HistoryUnit {
                        utf8: vec![b'c'],
                        width: 2,
                        style: Style::default(),
                    },
                ],
                break_after: HistoryBreakAfter::SoftWrap,
                start_offset: 0,
            });
        }
        {
            let chain = store.wrap_chains.back().expect("open wrap chain");
            assert_eq!(chain.pattern_len, 2);
            assert_eq!(chain.runs.as_slice(), &[(1, 2), (2, 1)]);
        }
        let (suffix, from, start_col) = store.eager_resize_suffix(8, 6);
        assert!(suffix.len() < 2_000);
        let from = from.expect("suffix cut");
        let prefix_units = store
            .wrap_chains
            .back()
            .map(|chain| chain.units_before(from))
            .unwrap_or(0) as usize;
        assert_eq!(start_col, wrap_occupancy(&widths[..prefix_units], 8));
        assert_eq!(store.wrap_column_before(Some(from), 8), start_col);
        store.truncate_from(from);
        let run_units = store
            .wrap_chains
            .back()
            .map(|chain| wrap_runs_unit_sum(&chain.runs) as usize)
            .unwrap_or(0);
        if run_units < prefix_units {
            assert_eq!(
                store.wrap_chains.back().map(|chain| chain.runs.len()),
                Some(2)
            );
        } else {
            assert_eq!(run_units, prefix_units);
        }
        assert_eq!(store.wrap_column_before(Some(from), 8), start_col);
        for line in suffix {
            store.append_line(line);
        }
        let chain = store.wrap_chains.back().expect("re-appended wrap chain");
        assert_eq!(chain.runs.first().copied(), Some((1, 2)));
        assert_eq!(chain.runs.get(1).copied(), Some((2, 1)));
        assert_eq!(chain.pattern_len, 2);
    }

    #[test]
    fn wrap_suffix_trim_drops_stale_two_run_tail_before_reappend() {
        let mut store = HistoryStore::default();
        store.append_line(HistoryLine {
            line_id: LineId(0),
            units: vec![
                HistoryUnit {
                    utf8: vec![b'a'],
                    width: 1,
                    style: Style::default(),
                },
                HistoryUnit {
                    utf8: vec![b'b'],
                    width: 1,
                    style: Style::default(),
                },
                HistoryUnit {
                    utf8: vec![b'c'],
                    width: 2,
                    style: Style::default(),
                },
            ],
            break_after: HistoryBreakAfter::SoftWrap,
            start_offset: 0,
        });
        {
            let chain = store.wrap_chains.back().expect("open wrap chain");
            assert_eq!(chain.pattern_len, 2);
            assert_eq!(chain.runs.as_slice(), &[(1, 2), (2, 1)]);
        }
        let from = HistoryAnchor {
            line_id: LineId(0),
            unit_offset: 1,
        };
        store.truncate_from(from);
        assert_eq!(
            store
                .wrap_chains
                .back()
                .expect("trimmed wrap chain")
                .runs
                .as_slice(),
            &[(1, 1)]
        );
        store.append_line(HistoryLine {
            line_id: LineId(0),
            units: vec![
                HistoryUnit {
                    utf8: vec![b'b'],
                    width: 1,
                    style: Style::default(),
                },
                HistoryUnit {
                    utf8: vec![b'c'],
                    width: 2,
                    style: Style::default(),
                },
            ],
            break_after: HistoryBreakAfter::SoftWrap,
            start_offset: 1,
        });
        let chain = store.wrap_chains.back().expect("re-appended wrap chain");
        assert_eq!(chain.runs.as_slice(), &[(1, 2), (2, 1)]);
        assert_eq!(chain.pattern_len, 2);
        assert_eq!(
            store.wrap_column_before(
                Some(HistoryAnchor {
                    line_id: LineId(0),
                    unit_offset: 3
                }),
                8
            ),
            wrap_occupancy(&[1, 1, 2], 8)
        );
    }

    #[test]
    fn drop_derived_cache_drops_wrap_index_without_touching_payload() {
        let mut store = HistoryStore::default();
        for id in 0..8_000 {
            store.append_line(ascii_line(id, "x", HistoryBreakAfter::SoftWrap));
        }
        assert!(store.wrap_index_allocated_bytes() > 0);
        let resident = store.resident_bytes();
        let from = HistoryAnchor {
            line_id: LineId(4_001),
            unit_offset: 0,
        };
        assert_eq!(store.wrap_column_before(Some(from), 8), 1);
        store.drop_derived_cache();
        assert_eq!(store.wrap_index_allocated_bytes(), 0);
        assert_eq!(store.derived_cache_bytes(), 0);
        assert_eq!(store.resident_bytes(), resident);
        assert_eq!(store.wrap_column_before(Some(from), 8), 1);
        assert!(matches!(
            store.resolve_anchor(HistoryAnchor {
                line_id: LineId(0),
                unit_offset: 0
            }),
            HistoryAnchorResolution::Resolved { .. }
        ));
        store.append_line(ascii_line(8_000, "y", HistoryBreakAfter::SoftWrap));
        assert!(store.wrap_index_allocated_bytes() > 0);
    }

    fn width_line(id: u64, widths: &[u8], break_after: HistoryBreakAfter) -> HistoryLine {
        HistoryLine {
            line_id: LineId(id),
            units: widths
                .iter()
                .map(|&width| HistoryUnit {
                    utf8: vec![b'x'],
                    width,
                    style: Style::default(),
                })
                .collect(),
            break_after,
            start_offset: 0,
        }
    }

    #[test]
    fn wrap_repeating_period_finds_small_periods() {
        assert_eq!(
            wrap_repeating_period(&[(1, 1), (2, 1), (1, 1), (2, 1)]),
            Some(2)
        );
        assert_eq!(
            wrap_repeating_period(&[
                (1, 1),
                (2, 2),
                (1, 3),
                (2, 1),
                (1, 1),
                (2, 2),
                (1, 3),
                (2, 1)
            ]),
            Some(4)
        );
        assert_eq!(
            wrap_repeating_period(&[
                (1, 1),
                (2, 1),
                (1, 2),
                (2, 1),
                (1, 3),
                (2, 1),
                (1, 4),
                (2, 1),
                (1, 5),
                (2, 1)
            ]),
            None
        );
    }

    #[test]
    fn wrap_column_before_is_closed_form_for_period_four_mixed_widths() {
        let mut store = HistoryStore::default();
        let period: &[u8] = &[1, 2, 2, 1, 1, 1, 2];
        let mut widths = Vec::new();
        for id in 0..2_000 {
            widths.extend_from_slice(period);
            store.append_line(width_line(id, period, HistoryBreakAfter::SoftWrap));
        }
        {
            let chain = store.wrap_chains.back().expect("open wrap chain");
            assert_eq!(chain.pattern_len, 4);
            assert_eq!(chain.runs.as_slice(), &[(1, 1), (2, 2), (1, 3), (2, 1)]);
        }
        let (suffix, from, start_col) = store.eager_resize_suffix(8, 6);
        assert!(suffix.len() < 2_000);
        let from = from.expect("suffix cut");
        let prefix_units = store
            .wrap_chains
            .back()
            .map(|chain| chain.units_before(from))
            .unwrap_or(0) as usize;
        assert_eq!(start_col, wrap_occupancy(&widths[..prefix_units], 8));
        assert_eq!(store.wrap_column_before(Some(from), 8), start_col);
        store.drop_derived_cache();
        assert_eq!(store.wrap_column_before(Some(from), 8), start_col);
    }

    #[test]
    fn wrap_column_before_matches_occupancy_for_aperiodic_mixed_widths() {
        let mut store = HistoryStore::default();
        let mut widths = Vec::new();
        for id in 0..256u64 {
            let ones = u8::try_from((id % 9) + 1).unwrap();
            let mut line = vec![1u8; usize::from(ones)];
            line.push(2);
            widths.extend_from_slice(&line);
            store.append_line(width_line(id, &line, HistoryBreakAfter::SoftWrap));
        }
        {
            let chain = store.wrap_chains.back().expect("open wrap chain");
            assert_eq!(chain.pattern_len, 0);
            assert!(chain.runs.len() > WRAP_PATTERN_MAX);
            assert!(chain.runs.len() > APERIODIC_INLINE_RUN_BOUND);
        }
        let (suffix, from, start_col) = store.eager_resize_suffix(8, 6);
        assert!(suffix.len() < 256);
        let from = from.expect("suffix cut");
        let prefix_units = store
            .wrap_chains
            .back()
            .map(|chain| chain.units_before(from))
            .unwrap_or(0) as usize;
        assert_eq!(start_col, wrap_occupancy(&widths[..prefix_units], 8));
        assert_eq!(store.wrap_column_before(Some(from), 8), start_col);
        // Spine must answer a second query without depending on a fresh linear walk.
        assert_eq!(store.wrap_column_before(Some(from), 8), start_col);
        {
            let chain = store.wrap_chains.back().expect("open wrap chain");
            let spine = chain.spine.borrow();
            assert!(
                spine
                    .as_ref()
                    .is_some_and(|spine| spine.cols == 8 && spine.run_len == chain.runs.len()),
                "aperiodic mid-chain cuts must retain a cols-dependent occupancy spine"
            );
        }
        // Miss path after derived-cache drop must stay exact (ephemeral spine).
        store.drop_derived_cache();
        assert_eq!(store.wrap_column_before(Some(from), 8), start_col);
    }

    #[test]
    fn eager_resize_extends_aperiodic_soft_wrap_to_hard_break_when_budget_allows() {
        let mut store = HistoryStore::default();
        store.append_line(ascii_line(0, "hard-prefix", HistoryBreakAfter::HardBreak));
        // Truly aperiodic run stream (no period ≤ WRAP_PATTERN_MAX), but cell
        // count fits the extend budget so resize can snap to HardBreak.
        for id in 1..50u64 {
            let ones = u8::try_from((id % 9) + 1).unwrap();
            let mut line = vec![1u8; usize::from(ones)];
            line.push(2);
            store.append_line(width_line(id, &line, HistoryBreakAfter::SoftWrap));
        }
        {
            let chain = store.wrap_chains.back().expect("open wrap chain");
            assert_eq!(chain.pattern_len, 0);
            assert!(chain.runs.len() > APERIODIC_INLINE_RUN_BOUND);
            let soft_cells: usize = chain
                .runs
                .iter()
                .map(|(width, count)| usize::from(*width) * (*count as usize))
                .sum();
            assert!(
                soft_cells
                    <= usize::from(8u16)
                        .saturating_mul(6)
                        .saturating_mul(APERIODIC_EXTEND_CELL_FACTOR),
                "fixture must fit extend budget; soft_cells={soft_cells}"
            );
        }
        let (suffix, from, start_col) = store.eager_resize_suffix(8, 6);
        assert_eq!(
            start_col, 0,
            "extend-to-HardBreak must clear mid-chain start_col"
        );
        let from = from.expect("suffix");
        assert_eq!(from.line_id, LineId(1));
        assert!(suffix.iter().any(|line| line.line_id == LineId(1)));
        assert!(!suffix.iter().any(|line| line.line_id == LineId(0)));
    }
}
