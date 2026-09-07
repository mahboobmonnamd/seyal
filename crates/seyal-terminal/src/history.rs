//! Canonical retained primary history and width-derived reflow.
//!
//! History owns source text units, rather than the physical rows used by the
//! active screen.  It deliberately has no persistence or renderer dependency;
//! sealed segments are the bounded handoff seam for those consumers.

use crate::{grapheme_store::GraphemeStore, Cell, CellRole, LineId, Style};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};

pub const HISTORY_SEGMENT_PAYLOAD_TARGET: usize = 16 * 1024;
pub const HISTORY_TAIL_PAYLOAD_LIMIT: usize = 2 * HISTORY_SEGMENT_PAYLOAD_TARGET;
pub const HISTORY_PER_EXECUTION_BYTE_CAP: usize = 32 * 1024 * 1024;
pub const HISTORY_RUNTIME_AGGREGATE_BYTE_CAP: usize = 256 * 1024 * 1024;
pub const HISTORY_PER_EXECUTION_DERIVED_INDEX_CAP: usize = 4 * 1024 * 1024;
pub const HISTORY_RUNTIME_DERIVED_INDEX_CAP: usize = 32 * 1024 * 1024;

static NEXT_SEGMENT_AGE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryBreakAfter {
    HardBreak,
    SoftWrap,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HistoryAnchor {
    pub line_id: LineId,
    pub unit_offset: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HistoryUnit {
    pub utf8: Vec<u8>,
    pub width: u8,
    pub style: Style,
}

impl HistoryUnit {
    fn encoded_len(&self) -> usize {
        self.utf8.len() + 3
    }

    fn first_scalar(&self) -> char {
        std::str::from_utf8(&self.utf8)
            .ok()
            .and_then(|text| text.chars().next())
            .unwrap_or('\u{FFFD}')
    }

    fn as_cell(&self) -> Cell {
        Cell::lead_inline(self.first_scalar(), self.width.max(1), self.style)
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
    fn payload_len(&self) -> usize {
        self.units.iter().map(HistoryUnit::encoded_len).sum()
    }

    pub(crate) fn cells(&self) -> Vec<Cell> {
        let mut cells = Vec::new();
        for unit in &self.units {
            cells.push(unit.as_cell());
            if unit.width >= 2 {
                cells.push(Cell::continuation());
            }
        }
        cells
    }
}

#[derive(Clone, Debug)]
struct Segment {
    lines: Vec<HistoryLine>,
    payload_bytes: usize,
    age: u64,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct HistoryStore {
    segments: VecDeque<Segment>,
    tail: Vec<HistoryLine>,
    tail_payload_bytes: usize,
    resident_bytes: usize,
    eviction_generation: u64,
}

impl HistoryStore {
    pub(crate) fn entries(&self) -> impl Iterator<Item = &HistoryLine> {
        self.segments
            .iter()
            .flat_map(|segment| segment.lines.iter())
            .chain(self.tail.iter())
    }

    pub(crate) fn resident_bytes(&self) -> usize {
        self.resident_bytes
    }

    pub(crate) fn eviction_generation(&self) -> u64 {
        self.eviction_generation
    }

    pub(crate) fn append_row(
        &mut self,
        line_id: LineId,
        break_after: HistoryBreakAfter,
        cells: &[Cell],
        store: &GraphemeStore,
    ) {
        let units = cells
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
            .collect::<Vec<_>>();
        self.append_line(HistoryLine {
            line_id,
            units,
            break_after,
            start_offset: 0,
        });
    }

    fn append_line(&mut self, line: HistoryLine) {
        let bytes = line.payload_len();
        // A source line can be arbitrarily long. Fragmenting at unit boundaries
        // keeps the tail bounded while preserving its LineId and break lineage.
        if bytes > HISTORY_SEGMENT_PAYLOAD_TARGET {
            let mut fragment = Vec::new();
            let mut fragment_bytes = 0;
            let mut source_offset = line.start_offset;
            let line_id = line.line_id;
            let final_break = line.break_after;
            for unit in line.units {
                let unit_bytes = unit.encoded_len();
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
        self.evict_to_cap();
    }

    fn push_fragment(&mut self, line: HistoryLine) {
        let bytes = line.payload_len();
        if self.tail_payload_bytes + bytes > HISTORY_SEGMENT_PAYLOAD_TARGET && !self.tail.is_empty()
        {
            self.seal_tail();
        }
        self.tail_payload_bytes += bytes;
        self.resident_bytes = self.resident_bytes.saturating_add(bytes);
        self.tail.push(line);
        if self.tail_payload_bytes >= HISTORY_SEGMENT_PAYLOAD_TARGET {
            self.seal_tail();
        }
    }

    fn seal_tail(&mut self) {
        if self.tail.is_empty() {
            return;
        }
        let segment = Segment {
            lines: std::mem::take(&mut self.tail),
            payload_bytes: self.tail_payload_bytes,
            age: NEXT_SEGMENT_AGE.fetch_add(1, Ordering::Relaxed),
        };
        self.tail_payload_bytes = 0;
        self.segments.push_back(segment);
    }

    fn evict_to_cap(&mut self) {
        while self.resident_bytes > HISTORY_PER_EXECUTION_BYTE_CAP {
            let Some(segment) = self.segments.pop_front() else {
                // The tail is capped by source-unit fragmentation and cannot
                // be discarded piecemeal without an explicit fragment policy.
                break;
            };
            self.resident_bytes = self.resident_bytes.saturating_sub(segment.payload_bytes);
            self.eviction_generation = self.eviction_generation.wrapping_add(1);
        }
    }

    pub(crate) fn oldest_segment_age(&self) -> Option<u64> {
        self.segments.front().map(|segment| segment.age)
    }

    pub(crate) fn evict_oldest_segment(&mut self) -> usize {
        let Some(segment) = self.segments.pop_front() else {
            return 0;
        };
        self.resident_bytes = self.resident_bytes.saturating_sub(segment.payload_bytes);
        self.eviction_generation = self.eviction_generation.wrapping_add(1);
        segment.payload_bytes
    }

    pub(crate) fn reflow(&self, cols: u16, max_rows: usize) -> Vec<ReflowRow> {
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
            for (unit_index, unit) in line.units.iter().enumerate() {
                let unit_width = usize::from(unit.width.max(1));
                if !current.cells.is_empty() && current.cells.len() + unit_width > width {
                    rows.push(std::mem::take(&mut current));
                    if rows.len() >= max_rows {
                        return rows;
                    }
                }
                current.anchors.push(HistoryAnchor {
                    line_id: line.line_id,
                    unit_offset: line.start_offset.saturating_add(unit_index as u32),
                });
                current.cells.push(unit.as_cell());
                if unit_width == 2 {
                    current.cells.push(Cell::continuation());
                }
            }
            previous_break = Some(line.break_after);
            if line.break_after == HistoryBreakAfter::HardBreak {
                rows.push(std::mem::take(&mut current));
                if rows.len() >= max_rows {
                    break;
                }
                previous_break = None;
            }
        }
        if !current.cells.is_empty() && rows.len() < max_rows {
            rows.push(current);
        }
        rows
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReflowRow {
    pub anchors: Vec<HistoryAnchor>,
    pub cells: Vec<Cell>,
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
        for id in 0..2_000 {
            store.append_line(ascii_line(id, "abcdefghij", HistoryBreakAfter::SoftWrap));
        }
        assert!(store.segments.len() >= 2);
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
}
