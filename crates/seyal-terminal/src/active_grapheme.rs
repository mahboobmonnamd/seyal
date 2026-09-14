//! Active grapheme anchor and printable-unit placement (SPEC-011 §§6–9).

#![allow(dead_code)]

use crate::{
    grapheme_store::{GraphemeStore, StoreAdmit, INLINE_STORE_ID, MAX_ACTIVE_GRAPHEME_BYTES},
    width::{extends_active_grapheme, grapheme_terminal_width, AmbiguousWidthPolicy},
    Cell, Style,
};

#[derive(Clone, Debug, Default)]
pub(crate) struct ActiveGrapheme {
    pub col: u16,
    pub row: u16,
    pub utf8: String,
    pub width: u8,
    pub style: Style,
    pub store_id: u32,
    pub overflow: bool,
}

impl ActiveGrapheme {
    pub(crate) fn as_str(&self) -> &str {
        &self.utf8
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PlaceOutcome {
    pub mutation_row: u16,
    pub mutation_row_end: u16,
    pub soft_wrapped: bool,
}

/// Decides how a newly completed width interacts with DECAWM at the final column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EdgeDecision {
    Place,
    IgnoreUnit,
    RejectExtension,
}

pub(crate) fn edge_decision_new_unit(
    col: u16,
    cols: u16,
    width: u8,
    wraparound: bool,
) -> EdgeDecision {
    if width < 2 {
        return EdgeDecision::Place;
    }
    if u16::from(width) > cols {
        return EdgeDecision::IgnoreUnit;
    }
    if col + 1 < cols {
        return EdgeDecision::Place;
    }
    // Needs two cells starting at `col` but only the final column remains.
    if wraparound {
        EdgeDecision::Place // caller wraps then places
    } else {
        EdgeDecision::IgnoreUnit
    }
}

pub(crate) fn edge_decision_late_widen(
    col: u16,
    cols: u16,
    new_width: u8,
    wraparound: bool,
) -> EdgeDecision {
    if new_width < 2 {
        return EdgeDecision::Place;
    }
    if u16::from(new_width) > cols {
        return EdgeDecision::RejectExtension;
    }
    if col + 1 < cols {
        return EdgeDecision::Place;
    }
    if col == cols - 1 {
        if wraparound {
            EdgeDecision::Place
        } else {
            EdgeDecision::RejectExtension
        }
    } else {
        EdgeDecision::Place
    }
}

pub(crate) fn try_append_scalar(active: &ActiveGrapheme, next: char, unicode_core: bool) -> bool {
    if !unicode_core {
        return false;
    }
    if active.overflow {
        // Overflowed grapheme: do not grow further until a new boundary.
        return extends_active_grapheme(active.as_str(), next);
    }
    extends_active_grapheme(active.as_str(), next)
}

pub(crate) fn append_payload(
    active: &mut ActiveGrapheme,
    next: char,
    store: &mut GraphemeStore,
    ambiguous: AmbiguousWidthPolicy,
) -> AppendResult {
    if active.overflow {
        // Still same grapheme boundary-wise but payload frozen.
        return AppendResult {
            width: active.width,
            rejected_extension: false,
            width_changed: false,
        };
    }

    let extra = next.len_utf8();
    if active.utf8.len().saturating_add(extra) > MAX_ACTIVE_GRAPHEME_BYTES {
        store.grapheme_payload_overflow_count =
            store.grapheme_payload_overflow_count.saturating_add(1);
        active.overflow = true;
        // Occupation stays; payload becomes sentinel semantics.
        if active.store_id != INLINE_STORE_ID {
            store.release(active.store_id);
            active.store_id = INLINE_STORE_ID;
        }
        return AppendResult {
            width: active.width,
            rejected_extension: false,
            width_changed: false,
        };
    }

    active.utf8.push(next);
    let new_width = grapheme_terminal_width(&active.utf8, ambiguous);
    let width_changed = new_width != active.width;
    AppendResult {
        width: new_width,
        rejected_extension: false,
        width_changed,
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct AppendResult {
    pub width: u8,
    pub rejected_extension: bool,
    pub width_changed: bool,
}

pub(crate) fn build_lead_cell(
    text: &str,
    width: u8,
    style: Style,
    overflow: bool,
    store: &mut GraphemeStore,
    release_first: Option<u32>,
) -> Cell {
    if overflow {
        return Cell::lead_from_admit(
            '\u{FFFD}',
            width.max(1),
            style,
            StoreAdmit::OverflowSentinel,
        );
    }
    let mut chars = text.chars();
    let first = chars.next().unwrap_or('\u{FFFD}');
    let multi = chars.next().is_some() || text.len() > first.len_utf8();
    if !multi {
        let _ = store.insert_replacing(&[], release_first);
        return Cell::lead_inline(first, width.max(1), style);
    }
    let admit = store.insert_replacing(text.as_bytes(), release_first);
    Cell::lead_from_admit(first, width.max(1), style, admit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decawm_reset_ignores_wide_at_final() {
        assert_eq!(
            edge_decision_new_unit(2, 3, 2, false),
            EdgeDecision::IgnoreUnit
        );
    }

    #[test]
    fn decawm_reset_rejects_late_widen_at_final() {
        assert_eq!(
            edge_decision_late_widen(2, 3, 2, false),
            EdgeDecision::RejectExtension
        );
    }

    #[test]
    fn unit_wider_than_grid_is_rejected_even_with_decawm() {
        assert_eq!(
            edge_decision_new_unit(0, 1, 2, true),
            EdgeDecision::IgnoreUnit
        );
        assert_eq!(
            edge_decision_late_widen(0, 1, 2, true),
            EdgeDecision::RejectExtension
        );
    }
}
