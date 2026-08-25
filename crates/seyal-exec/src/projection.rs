use seyal_terminal::{Color, TerminalState};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectionColor {
    Default,
    Indexed(u8),
    Rgb { r: u8, g: u8, b: u8 },
}

impl From<Color> for ProjectionColor {
    fn from(value: Color) -> Self {
        match value {
            Color::Default => Self::Default,
            Color::Indexed(index) => Self::Indexed(index),
            Color::Rgb { r, g, b } => Self::Rgb { r, g, b },
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProjectionAttributes {
    pub bold: bool,
    pub underline: bool,
    pub inverse: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectionCell {
    pub scalar: char,
    pub foreground: ProjectionColor,
    pub background: ProjectionColor,
    pub attributes: ProjectionAttributes,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectionDamage {
    pub full: bool,
    pub first_row: u16,
    pub last_row: u16,
}

impl ProjectionDamage {
    pub fn full(rows: u16) -> Self {
        Self {
            full: true,
            first_row: 0,
            last_row: rows.saturating_sub(1),
        }
    }

    pub fn row_count(self) -> u16 {
        self.last_row
            .checked_sub(self.first_row)
            .and_then(|value| value.checked_add(1))
            .unwrap_or(0)
    }
}

/// Complete, owned visible state used only for attach/reconnect/resync.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalProjectionSnapshot {
    pub rows: u16,
    pub columns: u16,
    pub cursor_row: u16,
    pub cursor_col: u16,
    pub cursor_visible: bool,
    pub alternate_screen: bool,
    pub source_damage_generation: u64,
    pub cells: Vec<ProjectionCell>,
}

/// Damage-sized, projection-neutral steady-state update.
///
/// `cells` contains only the rows described by `damage`, in row-major order.
/// A full damage record therefore contains the complete visible grid, while a
/// one-row change copies exactly one canonical row. This type deliberately has
/// no UDS/framing knowledge; `seyal-runtime` remains the display-wire owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalProjectionUpdate {
    pub rows: u16,
    pub columns: u16,
    pub cursor_row: u16,
    pub cursor_col: u16,
    pub cursor_visible: bool,
    pub alternate_screen: bool,
    pub source_damage_generation: u64,
    pub damage: ProjectionDamage,
    pub cells: Vec<ProjectionCell>,
}

pub(crate) fn snapshot(
    terminal: &TerminalState,
    source_damage_generation: u64,
) -> TerminalProjectionSnapshot {
    let rows = terminal.rows();
    TerminalProjectionSnapshot {
        rows,
        columns: terminal.cols(),
        cursor_row: terminal.cursor().row,
        cursor_col: terminal.cursor().col,
        cursor_visible: terminal.cursor().visible,
        alternate_screen: terminal.modes().alternate_screen,
        source_damage_generation,
        cells: copy_rows(terminal, 0, rows),
    }
}

pub(crate) fn update(
    terminal: &TerminalState,
    source_damage_generation: u64,
    damage: ProjectionDamage,
) -> TerminalProjectionUpdate {
    let rows = terminal.rows();
    let columns = terminal.cols();
    let cursor = terminal.cursor();
    let modes = terminal.modes();
    let effective_damage = if damage.full {
        ProjectionDamage::full(rows)
    } else {
        debug_assert!(damage.first_row <= damage.last_row);
        debug_assert!(damage.last_row < rows);
        damage
    };

    TerminalProjectionUpdate {
        rows,
        columns,
        cursor_row: cursor.row,
        cursor_col: cursor.col,
        cursor_visible: cursor.visible,
        alternate_screen: modes.alternate_screen,
        source_damage_generation,
        damage: effective_damage,
        cells: copy_rows(
            terminal,
            effective_damage.first_row,
            effective_damage.row_count(),
        ),
    }
}

fn copy_rows(terminal: &TerminalState, first_row: u16, row_count: u16) -> Vec<ProjectionCell> {
    let columns = terminal.cols();
    let mut cells = Vec::with_capacity(row_count as usize * columns as usize);
    for row in first_row..first_row.saturating_add(row_count) {
        for col in 0..columns {
            let cell = terminal.cell(col, row).unwrap_or_default();
            cells.push(ProjectionCell {
                scalar: cell.character,
                foreground: cell.style.fg.into(),
                background: cell.style.bg.into(),
                attributes: ProjectionAttributes {
                    bold: cell.style.bold,
                    underline: cell.style.underline,
                    inverse: cell.style.inverse,
                },
            });
        }
    }
    cells
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_copies_complete_visible_state() {
        let mut terminal = TerminalState::new(4, 2).unwrap();
        terminal.feed(b"hi").unwrap();
        let snapshot = snapshot(&terminal, 7);
        assert_eq!(snapshot.rows, 2);
        assert_eq!(snapshot.columns, 4);
        assert_eq!(snapshot.cells.len(), 8);
        assert_eq!(snapshot.cells[0].scalar, 'h');
        assert_eq!(snapshot.cells[1].scalar, 'i');
        assert_eq!(snapshot.source_damage_generation, 7);
    }

    #[test]
    fn steady_state_update_copies_only_damaged_rows() {
        let mut terminal = TerminalState::new(80, 24).unwrap();
        terminal.feed(b"hello").unwrap();
        let damage = ProjectionDamage {
            full: false,
            first_row: 0,
            last_row: 0,
        };
        let update = update(&terminal, 9, damage);
        assert_eq!(update.cells.len(), 80);
        assert_eq!(update.damage, damage);
        assert_eq!(update.cells[0].scalar, 'h');
        assert_eq!(update.source_damage_generation, 9);
    }

    #[test]
    fn full_update_copies_complete_visible_state() {
        let terminal = TerminalState::new(12, 5).unwrap();
        let update = update(&terminal, 4, ProjectionDamage::full(5));
        assert_eq!(update.cells.len(), 60);
        assert!(update.damage.full);
        assert_eq!((update.damage.first_row, update.damage.last_row), (0, 4));
    }
}