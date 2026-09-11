use std::sync::Arc;

use seyal_terminal::{CellRole, Color, TerminalState};

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectionCell {
    pub role: CellRole,
    pub width: u8,
    /// Full grapheme UTF-8 for Lead; empty for Empty/Continuation.
    pub text: Arc<[u8]>,
    /// First scalar convenience for scalar-only encode paths.
    pub scalar: char,
    pub foreground: ProjectionColor,
    pub background: ProjectionColor,
    pub attributes: ProjectionAttributes,
}

impl ProjectionCell {
    pub fn lead(
        scalar: char,
        foreground: ProjectionColor,
        background: ProjectionColor,
        attributes: ProjectionAttributes,
    ) -> Self {
        let mut buf = [0u8; 4];
        let encoded = scalar.encode_utf8(&mut buf);
        Self {
            role: CellRole::Lead,
            width: 1,
            text: Arc::from(encoded.as_bytes().to_vec()),
            scalar,
            foreground,
            background,
            attributes,
        }
    }

    pub fn lead_scalar(scalar: char, attributes: ProjectionAttributes) -> Self {
        Self::lead(
            scalar,
            ProjectionColor::Default,
            ProjectionColor::Default,
            attributes,
        )
    }

    pub fn is_scalar_lossless(&self) -> bool {
        match self.role {
            CellRole::Empty => self.text.is_empty() && self.width == 0,
            CellRole::Continuation => false,
            CellRole::Lead => {
                self.width == 1
                    && !self.text.is_empty()
                    && std::str::from_utf8(&self.text)
                        .ok()
                        .is_some_and(|s| s.chars().count() == 1)
            }
        }
    }
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
    /// Full-redraw guidance retained for comparator/reference compatibility.
    pub damage: ProjectionDamage,
    pub cells: Vec<ProjectionCell>,
}

impl TerminalProjectionSnapshot {
    pub fn is_scalar_lossless(&self) -> bool {
        self.cells.iter().all(ProjectionCell::is_scalar_lossless)
    }
}

/// Damage-sized, projection-neutral steady-state update.
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

impl TerminalProjectionUpdate {
    pub fn is_scalar_lossless(&self) -> bool {
        self.cells.iter().all(ProjectionCell::is_scalar_lossless)
    }
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
        damage: ProjectionDamage::full(rows),
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
    let mut cells: Vec<ProjectionCell> = Vec::with_capacity(row_count as usize * columns as usize);
    for row in first_row..first_row.saturating_add(row_count) {
        for col in 0..columns {
            let cell = terminal.cell(col, row).unwrap_or_default();
            let text = terminal
                .lead_utf8(col, row)
                .map(|cow| Arc::<[u8]>::from(cow.into_owned()))
                .unwrap_or_else(|| Arc::from([]));
            let scalar = match cell.role {
                CellRole::Lead => std::str::from_utf8(&text)
                    .ok()
                    .and_then(|s| s.chars().next())
                    .unwrap_or(cell.canonical_scalar()),
                CellRole::Empty | CellRole::Continuation => ' ',
            };
            // Continuations are only valid immediately after their lead in the
            // same row. Do not carry the previous row's last cell across a row
            // boundary when projecting reconnect snapshots.
            let previous = (col > 0)
                .then(|| cells.last())
                .flatten()
                .map(|cell| (cell.role, cell.width));
            let role = normalized_role(previous, cell.role);
            // Continuation cells are structural slots with a default terminal
            // style. The display wire contract repeats the lead's style on its
            // continuation, so inherit it at this projection boundary.
            let continuation_style = (role == CellRole::Continuation)
                .then(|| cells.last())
                .flatten()
                .map(|lead| (lead.foreground, lead.background, lead.attributes));
            let (foreground, background, attributes) = continuation_style.unwrap_or((
                cell.style.fg.into(),
                cell.style.bg.into(),
                ProjectionAttributes {
                    bold: cell.style.bold,
                    underline: cell.style.underline,
                    inverse: cell.style.inverse,
                },
            ));
            cells.push(ProjectionCell {
                role,
                width: if role == CellRole::Empty {
                    0
                } else {
                    cell.width
                },
                text: if role == CellRole::Empty {
                    Arc::from([])
                } else {
                    text
                },
                scalar: if role == CellRole::Empty { ' ' } else { scalar },
                foreground,
                background,
                attributes,
            });
        }
    }
    cells
}

fn normalized_role(previous: Option<(CellRole, u8)>, role: CellRole) -> CellRole {
    if role == CellRole::Continuation
        && !previous
            .is_some_and(|(previous_role, width)| previous_role == CellRole::Lead && width >= 2)
    {
        CellRole::Empty
    } else {
        role
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orphan_continuation_is_projected_as_empty() {
        assert_eq!(
            normalized_role(None, CellRole::Continuation),
            CellRole::Empty
        );
        assert_eq!(
            normalized_role(Some((CellRole::Lead, 1)), CellRole::Continuation),
            CellRole::Empty
        );
        assert_eq!(
            normalized_role(Some((CellRole::Lead, 2)), CellRole::Continuation),
            CellRole::Continuation
        );
    }

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
        assert_eq!(snapshot.damage, ProjectionDamage::full(2));
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

    #[test]
    fn multi_scalar_lead_projects_store_utf8() {
        let mut terminal = TerminalState::new(8, 2).unwrap();
        terminal.feed("❤️".as_bytes()).unwrap();
        let snapshot = snapshot(&terminal, 1);
        let lead = &snapshot.cells[0];
        assert_eq!(lead.role, CellRole::Lead);
        assert_eq!(lead.text.as_ref(), "❤️".as_bytes());
        if lead.width == 2 {
            assert_eq!(snapshot.cells[1].role, CellRole::Continuation);
            assert!(snapshot.cells[1].text.is_empty());
        }
    }

    #[test]
    fn wide_continuation_inherits_lead_style_for_wire_contract() {
        let mut terminal = TerminalState::new(8, 2).unwrap();
        terminal
            .feed(b"\x1b[38;5;208m\xF0\x9F\x91\xA9\xE2\x80\x8D\xF0\x9F\x92\xBB")
            .unwrap();
        let snapshot = snapshot(&terminal, 1);
        let lead = &snapshot.cells[0];
        if lead.width == 2 {
            assert_eq!(lead.foreground, ProjectionColor::Indexed(208));
            assert_eq!(snapshot.cells[1].role, CellRole::Continuation);
            assert_eq!(snapshot.cells[1].foreground, lead.foreground);
            assert_eq!(snapshot.cells[1].background, lead.background);
            assert_eq!(snapshot.cells[1].attributes, lead.attributes);
        }
    }
}
