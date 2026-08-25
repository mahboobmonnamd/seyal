//! Converts the owned projection-neutral snapshot exposed by `seyal-exec`
//! into the SPEC-004 shared-projection wire representation.
//!
//! `seyal-runtime` deliberately does not depend directly on `seyal-terminal`.
//! `TerminalExecution` remains the sole owner of canonical terminal state and
//! the single consumer of canonical damage; Runtime only receives an owned
//! snapshot suitable for fanout to attached projections.

use seyal_exec::{ProjectionColor, TerminalProjectionSnapshot};

use crate::projection::layout::{CellRecord, DamageRecord, ModeFlags, WireAttributes, WireColor};
use crate::projection::writer::SnapshotWrite;

fn convert_color(color: ProjectionColor) -> WireColor {
    match color {
        ProjectionColor::Default => WireColor::Default,
        ProjectionColor::Indexed(index) => WireColor::Indexed(index),
        ProjectionColor::Rgb { r, g, b } => WireColor::Rgb { r, g, b },
    }
}

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

/// Converts one owned execution snapshot into ABI 1.0's required complete
/// visible-state projection. Every slot still contains every visible cell so
/// missed generations remain recoverable, while `damages` preserves the
/// canonical coalesced row range as renderer redraw guidance.
pub fn from_execution(snapshot: TerminalProjectionSnapshot) -> OwnedSnapshot {
    let cells = snapshot
        .cells
        .into_iter()
        .map(|cell| CellRecord {
            scalar: cell.scalar,
            foreground: convert_color(cell.foreground),
            background: convert_color(cell.background),
            attributes: WireAttributes {
                bold: cell.attributes.bold,
                underline: cell.attributes.underline,
                inverse: cell.attributes.inverse,
            },
        })
        .collect();

    OwnedSnapshot {
        rows: snapshot.rows,
        columns: snapshot.columns,
        cursor_row: snapshot.cursor_row,
        cursor_col: snapshot.cursor_col,
        cursor_visible: snapshot.cursor_visible,
        mode_flags: ModeFlags {
            alternate_screen: snapshot.alternate_screen,
            cursor_visible: snapshot.cursor_visible,
        },
        cells,
        damages: vec![DamageRecord {
            first_row: snapshot.damage.first_row,
            last_row: snapshot.damage.last_row,
            full: snapshot.damage.full,
        }],
        full_snapshot: true,
        source_damage_generation: snapshot.source_damage_generation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use seyal_exec::{ProjectionAttributes, ProjectionCell, ProjectionDamage};

    #[test]
    fn conversion_covers_every_visible_cell_and_preserves_damage_guidance() {
        let snapshot = TerminalProjectionSnapshot {
            rows: 2,
            columns: 2,
            cursor_row: 0,
            cursor_col: 1,
            cursor_visible: true,
            alternate_screen: false,
            source_damage_generation: 3,
            damage: ProjectionDamage {
                full: false,
                first_row: 1,
                last_row: 1,
            },
            cells: vec![
                ProjectionCell {
                    scalar: 'h',
                    foreground: ProjectionColor::Default,
                    background: ProjectionColor::Default,
                    attributes: ProjectionAttributes::default(),
                },
                ProjectionCell {
                    scalar: 'i',
                    foreground: ProjectionColor::Indexed(4),
                    background: ProjectionColor::Default,
                    attributes: ProjectionAttributes {
                        bold: true,
                        underline: false,
                        inverse: false,
                    },
                },
                ProjectionCell {
                    scalar: ' ',
                    foreground: ProjectionColor::Default,
                    background: ProjectionColor::Default,
                    attributes: ProjectionAttributes::default(),
                },
                ProjectionCell {
                    scalar: ' ',
                    foreground: ProjectionColor::Default,
                    background: ProjectionColor::Default,
                    attributes: ProjectionAttributes::default(),
                },
            ],
        };
        let owned = from_execution(snapshot);
        assert_eq!(owned.cells.len(), 4);
        assert_eq!(owned.cells[0].scalar, 'h');
        assert_eq!(owned.cells[1].scalar, 'i');
        assert!(owned.full_snapshot);
        assert_eq!(owned.damages.len(), 1);
        assert!(!owned.damages[0].full);
        assert_eq!(owned.damages[0].first_row, 1);
        assert_eq!(owned.damages[0].last_row, 1);
        assert_eq!(owned.source_damage_generation, 3);
    }
}
