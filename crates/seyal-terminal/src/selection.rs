//! Host selection, copy-text formatting, search cursor, and keyboard copy mode.
//!
//! `TerminalState` remains the sole terminal authority. Selection and search
//! are derived views over canonical source anchors (SPEC-010 §11–§12). They
//! never become a second transcript store and never run on the PTY feed path.

use crate::{CellRole, HistoryBreakAfter, HistoryMatch, HistoryRangeError, HistoryUnitView};

/// Bounded paste payload before bracket wrapping. Stays inside the existing
/// local-IPC input ceiling (`MAX_INPUT_BYTES` = 64 KiB) after wrappers.
pub const MAX_PASTE_BYTES: usize = 65_520;
pub const MAX_SEARCH_MATCHES: usize = 1_024;
pub const MAX_COPY_BYTES: usize = 256 * 1024;

const BRACKET_PASTE_START: &[u8] = b"\x1b[200~";
const BRACKET_PASTE_END: &[u8] = b"\x1b[201~";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SelectionKind {
    #[default]
    Linear,
    Rectangular,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VisualPos {
    pub col: u16,
    pub row: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CopyModeMotion {
    Left,
    Right,
    Up,
    Down,
    LineStart,
    LineEnd,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SelectionSession {
    pub kind: SelectionKind,
    pub start: Option<VisualPos>,
    pub end: Option<VisualPos>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SearchSession {
    pub needle: String,
    pub index: Option<usize>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CopyMode {
    pub active: bool,
    pub cursor: VisualPos,
    pub anchor: Option<VisualPos>,
    pub kind: SelectionKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PasteError {
    Empty,
    TooLarge,
}

/// Copy text from canonical history units.
///
/// A hard break becomes `\n` only when the selection actually crosses that
/// source-line boundary. Soft wraps never fabricate a newline.
pub fn format_history_copy(units: &[HistoryUnitView]) -> Result<String, HistoryRangeError> {
    let mut out = String::new();
    for (index, unit) in units.iter().enumerate() {
        push_bounded(&mut out, &unit.text)?;
        let Some(next) = units.get(index + 1) else {
            break;
        };
        if next.anchor.line_id != unit.anchor.line_id
            && unit.break_after == HistoryBreakAfter::HardBreak
        {
            push_bounded(&mut out, "\n")?;
        }
    }
    Ok(out)
}

pub fn sanitize_paste(bytes: &[u8]) -> Result<Vec<u8>, PasteError> {
    if bytes.is_empty() {
        return Err(PasteError::Empty);
    }
    if bytes.len() > MAX_PASTE_BYTES {
        return Err(PasteError::TooLarge);
    }
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0 {
            i += 1;
            continue;
        }
        if let Some(skip) = bracket_paste_marker_len(&bytes[i..]) {
            i += skip;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    if out.is_empty() {
        return Err(PasteError::Empty);
    }
    if out.len() > MAX_PASTE_BYTES {
        return Err(PasteError::TooLarge);
    }
    Ok(out)
}

pub fn encode_paste(bytes: &[u8], bracketed: bool) -> Result<Vec<u8>, PasteError> {
    let sanitized = sanitize_paste(bytes)?;
    if !bracketed {
        return Ok(sanitized);
    }
    let wrapped_len = BRACKET_PASTE_START
        .len()
        .saturating_add(sanitized.len())
        .saturating_add(BRACKET_PASTE_END.len());
    if wrapped_len
        > MAX_PASTE_BYTES.saturating_add(BRACKET_PASTE_START.len() + BRACKET_PASTE_END.len())
    {
        return Err(PasteError::TooLarge);
    }
    let mut out = Vec::with_capacity(wrapped_len);
    out.extend_from_slice(BRACKET_PASTE_START);
    out.extend_from_slice(&sanitized);
    out.extend_from_slice(BRACKET_PASTE_END);
    Ok(out)
}

fn bracket_paste_marker_len(bytes: &[u8]) -> Option<usize> {
    for marker in [BRACKET_PASTE_START, BRACKET_PASTE_END] {
        if bytes.starts_with(marker) {
            return Some(marker.len());
        }
    }
    None
}

fn push_bounded(out: &mut String, text: &str) -> Result<(), HistoryRangeError> {
    if out.len().saturating_add(text.len()) > MAX_COPY_BYTES {
        return Err(HistoryRangeError::Unrepresentable);
    }
    out.push_str(text);
    Ok(())
}

impl SelectionSession {
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn set_linear(&mut self, start: VisualPos, end: VisualPos) {
        self.kind = SelectionKind::Linear;
        self.start = Some(start);
        self.end = Some(end);
    }

    pub fn set_rectangular(&mut self, start: VisualPos, end: VisualPos) {
        self.kind = SelectionKind::Rectangular;
        self.start = Some(start);
        self.end = Some(end);
    }

    pub fn ordered_corners(self) -> Option<(VisualPos, VisualPos)> {
        let start = self.start?;
        let end = self.end?;
        Some(order_visual(start, end))
    }

    /// Visual coverage matching `copy_visual_cells`. `cols` is the current
    /// screen width so linear mid-line spans can fill to the row end.
    pub fn contains_cell(self, col: u16, row: u16, cols: u16) -> bool {
        let Some((start, end)) = self.ordered_corners() else {
            return false;
        };
        let min_row = start.row.min(end.row);
        let max_row = start.row.max(end.row);
        if row < min_row || row > max_row {
            return false;
        }
        let last_col = cols.saturating_sub(1);
        let (row_start, row_end) = if self.kind == SelectionKind::Rectangular
            || (row == min_row && row == max_row)
        {
            (start.col.min(end.col), start.col.max(end.col))
        } else if row == min_row {
            if (start.row, start.col) <= (end.row, end.col) {
                (start.col, last_col)
            } else {
                (end.col, last_col)
            }
        } else if row == max_row {
            if (start.row, start.col) <= (end.row, end.col) {
                (0, end.col)
            } else {
                (0, start.col)
            }
        } else {
            (0, last_col)
        };
        col >= row_start && col <= row_end
    }
}

#[cfg(test)]
mod selection_contains_tests {
    use super::*;

    #[test]
    fn linear_contains_fills_intermediate_rows() {
        let session = SelectionSession {
            kind: SelectionKind::Linear,
            start: Some(VisualPos { col: 2, row: 0 }),
            end: Some(VisualPos { col: 1, row: 2 }),
        };
        assert!(session.contains_cell(2, 0, 4));
        assert!(session.contains_cell(3, 0, 4));
        assert!(session.contains_cell(0, 1, 4));
        assert!(session.contains_cell(3, 1, 4));
        assert!(session.contains_cell(0, 2, 4));
        assert!(session.contains_cell(1, 2, 4));
        assert!(!session.contains_cell(2, 2, 4));
        assert!(!session.contains_cell(1, 0, 4));
    }

    #[test]
    fn rectangular_contains_is_a_column_box() {
        let session = SelectionSession {
            kind: SelectionKind::Rectangular,
            start: Some(VisualPos { col: 1, row: 0 }),
            end: Some(VisualPos { col: 2, row: 1 }),
        };
        assert!(session.contains_cell(1, 0, 4));
        assert!(session.contains_cell(2, 1, 4));
        assert!(!session.contains_cell(0, 0, 4));
        assert!(!session.contains_cell(3, 1, 4));
    }
}

impl SearchSession {
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn step<'a>(
        &mut self,
        matches: &'a [HistoryMatch],
        forward: bool,
    ) -> Option<&'a HistoryMatch> {
        if matches.is_empty() {
            self.index = None;
            return None;
        }
        let next = match self.index {
            None => {
                if forward {
                    0
                } else {
                    matches.len() - 1
                }
            }
            Some(index) => {
                if forward {
                    (index + 1) % matches.len()
                } else if index == 0 {
                    matches.len() - 1
                } else {
                    index - 1
                }
            }
        };
        self.index = Some(next);
        matches.get(next)
    }
}

impl CopyMode {
    pub fn enter(&mut self, cursor: VisualPos) {
        *self = Self {
            active: true,
            cursor,
            anchor: None,
            kind: SelectionKind::Linear,
        };
    }

    pub fn exit(&mut self) {
        *self = Self::default();
    }

    pub fn apply_motion(&mut self, motion: CopyModeMotion, cols: u16, rows: u16) {
        if !self.active || cols == 0 || rows == 0 {
            return;
        }
        match motion {
            CopyModeMotion::Left => {
                self.cursor.col = self.cursor.col.saturating_sub(1);
            }
            CopyModeMotion::Right => {
                self.cursor.col = self
                    .cursor
                    .col
                    .saturating_add(1)
                    .min(cols.saturating_sub(1));
            }
            CopyModeMotion::Up => {
                self.cursor.row = self.cursor.row.saturating_sub(1);
            }
            CopyModeMotion::Down => {
                self.cursor.row = self
                    .cursor
                    .row
                    .saturating_add(1)
                    .min(rows.saturating_sub(1));
            }
            CopyModeMotion::LineStart => self.cursor.col = 0,
            CopyModeMotion::LineEnd => self.cursor.col = cols.saturating_sub(1),
        }
    }

    pub fn toggle_anchor(&mut self) {
        if !self.active {
            return;
        }
        if self.anchor.is_some() {
            self.anchor = None;
        } else {
            self.anchor = Some(self.cursor);
        }
    }

    pub fn toggle_kind(&mut self) {
        if !self.active {
            return;
        }
        self.kind = match self.kind {
            SelectionKind::Linear => SelectionKind::Rectangular,
            SelectionKind::Rectangular => SelectionKind::Linear,
        };
    }

    pub fn selection(&self) -> Option<SelectionSession> {
        if !self.active {
            return None;
        }
        let start = self.anchor?;
        Some(SelectionSession {
            kind: self.kind,
            start: Some(start),
            end: Some(self.cursor),
        })
    }
}

pub fn order_visual(a: VisualPos, b: VisualPos) -> (VisualPos, VisualPos) {
    if (a.row, a.col) <= (b.row, b.col) {
        (a, b)
    } else {
        (b, a)
    }
}

pub fn skip_continuation(role: CellRole, moving_right: bool, col: u16, cols: u16) -> u16 {
    if role != CellRole::Continuation {
        return col;
    }
    if moving_right {
        col.saturating_add(1).min(cols.saturating_sub(1))
    } else {
        col.saturating_sub(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{HistoryAnchor, LineId, Style};

    fn unit(line: u64, offset: u32, text: &str, br: HistoryBreakAfter) -> HistoryUnitView {
        HistoryUnitView {
            anchor: HistoryAnchor {
                line_id: LineId(line),
                unit_offset: offset,
            },
            text: text.to_owned(),
            width: 1,
            style: Style::default(),
            break_after: br,
        }
    }

    #[test]
    fn hard_break_becomes_newline_only_when_selection_crosses_it() {
        let units = [
            unit(1, 0, "a", HistoryBreakAfter::HardBreak),
            unit(1, 1, "b", HistoryBreakAfter::HardBreak),
            unit(2, 0, "c", HistoryBreakAfter::HardBreak),
        ];
        assert_eq!(format_history_copy(&units).unwrap(), "ab\nc");
    }

    #[test]
    fn soft_wrap_does_not_fabricate_newline() {
        let units = [
            unit(1, 0, "a", HistoryBreakAfter::SoftWrap),
            unit(2, 0, "b", HistoryBreakAfter::HardBreak),
        ];
        assert_eq!(format_history_copy(&units).unwrap(), "ab");
    }

    #[test]
    fn mid_line_copy_omits_trailing_newline() {
        let units = [
            unit(1, 1, "b", HistoryBreakAfter::HardBreak),
            unit(1, 2, "c", HistoryBreakAfter::HardBreak),
        ];
        assert_eq!(format_history_copy(&units).unwrap(), "bc");
    }

    #[test]
    fn paste_strips_nul_and_embedded_bracket_markers() {
        let raw = b"ab\x00c\x1b[200~d\x1b[201~e";
        assert_eq!(sanitize_paste(raw).unwrap(), b"abcde");
    }

    #[test]
    fn paste_wraps_only_when_bracketed() {
        let plain = encode_paste(b"hi", false).unwrap();
        assert_eq!(plain, b"hi");
        let wrapped = encode_paste(b"hi", true).unwrap();
        assert_eq!(wrapped, b"\x1b[200~hi\x1b[201~");
    }
}
