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
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalProjectionSnapshot {
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
    damage: ProjectionDamage,
) -> TerminalProjectionSnapshot {
    let rows = terminal.rows();
    let columns = terminal.cols();
    let cursor = terminal.cursor();
    let modes = terminal.modes();
    let mut cells = Vec::with_capacity(rows as usize * columns as usize);

    for row in 0..rows {
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

    TerminalProjectionSnapshot {
        rows,
        columns,
        cursor_row: cursor.row,
        cursor_col: cursor.col,
        cursor_visible: cursor.visible,
        alternate_screen: modes.alternate_screen,
        source_damage_generation,
        damage,
        cells,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_snapshot_copies_visible_state_without_exposing_terminal_types() {
        let mut terminal = TerminalState::new(4, 2).unwrap();
        terminal.feed(b"hi").unwrap();
        let damage = ProjectionDamage {
            full: false,
            first_row: 0,
            last_row: 0,
        };
        let snapshot = snapshot(&terminal, 7, damage);
        assert_eq!(snapshot.rows, 2);
        assert_eq!(snapshot.columns, 4);
        assert_eq!(snapshot.cells.len(), 8);
        assert_eq!(snapshot.cells[0].scalar, 'h');
        assert_eq!(snapshot.cells[1].scalar, 'i');
        assert_eq!(snapshot.source_damage_generation, 7);
        assert_eq!(snapshot.damage, damage);
    }
}
