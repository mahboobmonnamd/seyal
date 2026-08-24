//! Bridges canonical `seyal_terminal::TerminalState` into the SPEC-004
//! projection wire representation. This is the single sanctioned producer:
//! it reads the canonical visible state and calls
//! `TerminalExecution::take_damage()` (never a second/independent
//! consumer), then builds a bounded, fully owned [`SnapshotWrite`].

use seyal_terminal::{Color, TerminalState};

use crate::projection::layout::{CellRecord, DamageRecord, ModeFlags, WireAttributes, WireColor};
use crate::projection::writer::SnapshotWrite;

fn convert_color(color: Color) -> WireColor {
    match color {
        Color::Default => WireColor::Default,
        Color::Indexed(index) => WireColor::Indexed(index),
        Color::Rgb { r, g, b } => WireColor::Rgb { r, g, b },
    }
}

/// A fully owned, bounded snapshot ready to hand to
/// [`crate::projection::writer::Writer::publish`]. Owning the buffers here
/// (rather than borrowing from `TerminalState`) keeps the borrow of the
/// canonical state as short as possible.
pub struct OwnedSnapshot {
    pub rows: u16,
    pub columns: u16,
    pub cursor_row: u16,
    pub cursor_col: u16,
    pub cursor_visible: bool,
    pub mode_flags: ModeFlags,
    pub cells: Vec<CellRecord>,
    pub damages: Vec<DamageRecord>,
    pub full_snapshot: bool,
    pub source_damage_generation: u64,
}

impl OwnedSnapshot {
    pub fn as_snapshot_write(&self) -> SnapshotWrite<'_> {
        SnapshotWrite {
            rows: self.rows,
            columns: self.columns,
            cursor_row: self.cursor_row,
            cursor_col: self.cursor_col,
            cursor_visible: self.cursor_visible,
            cursor_style: 0,
            mode_flags: self.mode_flags,
            cells: &self.cells,
            damages: &self.damages,
            full_snapshot: self.full_snapshot,
            source_damage_generation: self.source_damage_generation,
        }
    }
}

/// Builds a full visible-snapshot [`OwnedSnapshot`] from `terminal`.
///
/// ABI 1.0 slots always carry a complete snapshot (SPEC-004 section 9.3),
/// so this always walks every visible cell; `damage` narrows only the
/// renderer-guidance damage descriptor, never which cells are included.
pub fn full_snapshot(terminal: &TerminalState, source_damage_generation: u64) -> OwnedSnapshot {
    let rows = terminal.rows();
    let columns = terminal.cols();
    let cursor = terminal.cursor();
    let modes = terminal.modes();

    let mut cells = Vec::with_capacity(rows as usize * columns as usize);
    for row in 0..rows {
        for col in 0..columns {
            let cell = terminal.cell(col, row).unwrap_or_default();
            cells.push(CellRecord {
                scalar: cell.character,
                foreground: convert_color(cell.style.fg),
                background: convert_color(cell.style.bg),
                attributes: WireAttributes {
                    bold: cell.style.bold,
                    underline: cell.style.underline,
                    inverse: cell.style.inverse,
                },
            });
        }
    }

    OwnedSnapshot {
        rows,
        columns,
        cursor_row: cursor.row,
        cursor_col: cursor.col,
        cursor_visible: cursor.visible,
        mode_flags: ModeFlags {
            alternate_screen: modes.alternate_screen,
            cursor_visible: cursor.visible,
        },
        cells,
        damages: vec![DamageRecord {
            first_row: 0,
            last_row: rows.saturating_sub(1),
            full: true,
        }],
        full_snapshot: true,
        source_damage_generation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_snapshot_covers_every_visible_cell_with_a_full_damage_descriptor() {
        let mut terminal = TerminalState::new(4, 2).unwrap();
        terminal.feed(b"hi").unwrap();
        let snapshot = full_snapshot(&terminal, 3);
        assert_eq!(snapshot.rows, 2);
        assert_eq!(snapshot.columns, 4);
        assert_eq!(snapshot.cells.len(), 8);
        assert_eq!(snapshot.cells[0].scalar, 'h');
        assert_eq!(snapshot.cells[1].scalar, 'i');
        assert!(snapshot.full_snapshot);
        assert_eq!(snapshot.damages.len(), 1);
        assert!(snapshot.damages[0].full);
        assert_eq!(snapshot.damages[0].last_row, 1);
    }
}
