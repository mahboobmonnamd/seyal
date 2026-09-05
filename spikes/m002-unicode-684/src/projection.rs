use std::mem::size_of;

const SIDECAR_TAG: u32 = 1 << 31;
const SIDECAR_INDEX_MASK: u32 = !SIDECAR_TAG;
const ROLE_EMPTY: u32 = 0;
const ROLE_LEAD: u32 = 1;
const ROLE_CONTINUATION: u32 = 2;
const ROLE_SHIFT: u32 = 0;
const WIDTH_SHIFT: u32 = 2;

/// Spike-only Candidate-D v2 pressure model. This deliberately keeps the
/// current 16-byte fixed cell footprint by moving only multi-scalar grapheme
/// payloads into a bounded batch-local sidecar. It is not a wire proposal yet.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct PackedCell {
    text_key: u32,
    foreground: u32,
    background: u32,
    meta: u32,
}

#[derive(Debug, Default)]
struct ProjectionBatch {
    cells: Vec<PackedCell>,
    sidecar: Vec<Vec<u8>>,
}

fn meta(role: u32, width: u8) -> u32 {
    (role << ROLE_SHIFT) | (u32::from(width) << WIDTH_SHIFT)
}

fn role(cell: PackedCell) -> u32 {
    cell.meta & 0b11
}

fn width(cell: PackedCell) -> u8 {
    ((cell.meta >> WIDTH_SHIFT) & 0b11) as u8
}

impl ProjectionBatch {
    fn push_empty(&mut self) {
        self.cells.push(PackedCell {
            meta: meta(ROLE_EMPTY, 1),
            ..PackedCell::default()
        });
    }

    fn push_cluster(&mut self, text: &str, cell_width: u8) {
        assert!((1..=2).contains(&cell_width));
        let mut chars = text.chars();
        let first = chars.next().expect("cluster text is non-empty");
        let text_key = if chars.next().is_none() {
            first as u32
        } else {
            let index = u32::try_from(self.sidecar.len()).expect("spike sidecar index remains bounded");
            self.sidecar.push(text.as_bytes().to_vec());
            SIDECAR_TAG | index
        };

        self.cells.push(PackedCell {
            text_key,
            meta: meta(ROLE_LEAD, cell_width),
            ..PackedCell::default()
        });
        if cell_width == 2 {
            self.cells.push(PackedCell {
                meta: meta(ROLE_CONTINUATION, 0),
                ..PackedCell::default()
            });
        }
    }

    fn text_for(&self, cell: PackedCell) -> Option<String> {
        if role(cell) != ROLE_LEAD {
            return None;
        }
        if cell.text_key & SIDECAR_TAG == 0 {
            return char::from_u32(cell.text_key).map(|character| character.to_string());
        }
        let index = (cell.text_key & SIDECAR_INDEX_MASK) as usize;
        self.sidecar
            .get(index)
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
            .map(str::to_owned)
    }

    fn sidecar_wire_bytes(&self) -> usize {
        self.sidecar
            .iter()
            .map(|entry| size_of::<u32>() + entry.len())
            .sum()
    }

    fn total_wire_payload_bytes(&self) -> usize {
        self.cells.len() * size_of::<PackedCell>() + self.sidecar_wire_bytes()
    }
}

pub(crate) fn report_projection_pressure() {
    let mut batch = ProjectionBatch::default();
    batch.push_empty();
    batch.push_cluster("A", 1);
    batch.push_cluster("界", 2);
    batch.push_cluster("e\u{301}", 1);
    batch.push_cluster("👩‍💻", 2);
    batch.push_cluster("👨‍👩‍👧‍👦", 2);

    let baseline = batch.cells.len() * 16;
    println!(
        "PROJECTION\tfixed_cell_bytes={}\tphysical_cells={}\tsidecar_entries={}\tsidecar_wire_bytes={}\tbaseline_fixed_bytes={}\tcandidate_total_bytes={}\toverhead_bytes={}",
        size_of::<PackedCell>(),
        batch.cells.len(),
        batch.sidecar.len(),
        batch.sidecar_wire_bytes(),
        baseline,
        batch.total_wire_payload_bytes(),
        batch.total_wire_payload_bytes().saturating_sub(baseline),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_keeps_fixed_cell_record_at_current_sixteen_bytes() {
        assert_eq!(size_of::<PackedCell>(), 16);
    }

    #[test]
    fn single_scalar_clusters_need_no_sidecar_payload() {
        let mut batch = ProjectionBatch::default();
        batch.push_cluster("A", 1);
        batch.push_cluster("界", 2);
        assert!(batch.sidecar.is_empty());
        assert_eq!(batch.text_for(batch.cells[0]).as_deref(), Some("A"));
        assert_eq!(batch.text_for(batch.cells[1]).as_deref(), Some("界"));
        assert_eq!(role(batch.cells[2]), ROLE_CONTINUATION);
    }

    #[test]
    fn multi_scalar_grapheme_round_trips_through_sidecar() {
        let mut batch = ProjectionBatch::default();
        batch.push_cluster("e\u{301}", 1);
        batch.push_cluster("👩‍💻", 2);
        assert_eq!(batch.sidecar.len(), 2);
        assert_eq!(batch.text_for(batch.cells[0]).as_deref(), Some("e\u{301}"));
        assert_eq!(batch.text_for(batch.cells[1]).as_deref(), Some("👩‍💻"));
        assert_eq!(width(batch.cells[1]), 2);
        assert_eq!(role(batch.cells[2]), ROLE_CONTINUATION);
    }

    #[test]
    fn continuation_has_no_independent_text_authority() {
        let mut batch = ProjectionBatch::default();
        batch.push_cluster("👨‍👩‍👧‍👦", 2);
        let continuation = batch.cells[1];
        assert_eq!(role(continuation), ROLE_CONTINUATION);
        assert_eq!(width(continuation), 0);
        assert_eq!(batch.text_for(continuation), None);
    }
}
