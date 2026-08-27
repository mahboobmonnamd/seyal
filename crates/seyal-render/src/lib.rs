//! Deterministic, platform-neutral preparation for Seyal's permanent terminal renderer.
//!
//! This crate owns no PTY, VT parser, canonical terminal state, native font object,
//! or GPU resource. It converts an already-committed disposable client display
//! state into reusable row-oriented presentation data. Native renderers consume
//! these prepared rows in coarse batches; no per-cell Rust/native callback is
//! required.

pub const MAX_RENDER_ROWS: u16 = 256;
pub const MAX_RENDER_COLUMNS: u16 = 512;
const DAMAGE_WORDS: usize = MAX_RENDER_ROWS as usize / u64::BITS as usize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderColor {
    Default,
    Indexed(u8),
    Rgb { r: u8, g: u8, b: u8 },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderAttributes {
    pub bold: bool,
    pub underline: bool,
    pub inverse: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderCell {
    pub scalar: char,
    pub foreground: RenderColor,
    pub background: RenderColor,
    pub attributes: RenderAttributes,
}

impl Default for RenderCell {
    fn default() -> Self {
        Self {
            scalar: ' ',
            foreground: RenderColor::Default,
            background: RenderColor::Default,
            attributes: RenderAttributes::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CursorState {
    pub row: u16,
    pub column: u16,
    pub visible: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct CommittedDisplay<'a> {
    pub generation: u64,
    pub rows: u16,
    pub columns: u16,
    pub cursor: CursorState,
    pub alternate_screen: bool,
    pub cells: &'a [RenderCell],
}

impl CommittedDisplay<'_> {
    fn validate(self) -> Result<(), PrepareError> {
        if self.rows == 0
            || self.columns == 0
            || self.rows > MAX_RENDER_ROWS
            || self.columns > MAX_RENDER_COLUMNS
        {
            return Err(PrepareError::InvalidGeometry);
        }
        if self.cursor.row >= self.rows || self.cursor.column >= self.columns {
            return Err(PrepareError::InvalidCursor);
        }
        let expected = (self.rows as usize)
            .checked_mul(self.columns as usize)
            .ok_or(PrepareError::Overflow)?;
        if self.cells.len() != expected {
            return Err(PrepareError::InvalidCellCount);
        }
        Ok(())
    }
}

/// Row-granular invalidation matching the current Candidate-D display contract,
/// which transports complete damaged rows. The representation is fixed-size so
/// coalescing does not allocate on the terminal presentation path.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RowDamage {
    words: [u64; DAMAGE_WORDS],
}

impl RowDamage {
    pub const fn none() -> Self {
        Self {
            words: [0; DAMAGE_WORDS],
        }
    }

    pub fn full(rows: u16) -> Self {
        let mut damage = Self::none();
        for row in 0..rows.min(MAX_RENDER_ROWS) {
            damage.mark_row(row);
        }
        damage
    }

    pub fn from_range(first_row: u16, row_count: u16) -> Result<Self, PrepareError> {
        let end = first_row
            .checked_add(row_count)
            .ok_or(PrepareError::InvalidDamage)?;
        if row_count == 0 || end > MAX_RENDER_ROWS {
            return Err(PrepareError::InvalidDamage);
        }
        let mut damage = Self::none();
        for row in first_row..end {
            damage.mark_row(row);
        }
        Ok(damage)
    }

    pub fn mark_row(&mut self, row: u16) {
        if row >= MAX_RENDER_ROWS {
            return;
        }
        let word = row as usize / u64::BITS as usize;
        let bit = row as usize % u64::BITS as usize;
        self.words[word] |= 1_u64 << bit;
    }

    pub fn contains(self, row: u16) -> bool {
        if row >= MAX_RENDER_ROWS {
            return false;
        }
        let word = row as usize / u64::BITS as usize;
        let bit = row as usize % u64::BITS as usize;
        self.words[word] & (1_u64 << bit) != 0
    }

    pub fn is_empty(self) -> bool {
        self.words.iter().all(|word| *word == 0)
    }

    pub fn union(&mut self, other: Self) {
        for (target, incoming) in self.words.iter_mut().zip(other.words) {
            *target |= incoming;
        }
    }

    pub fn count(self) -> usize {
        self.words
            .iter()
            .map(|word| word.count_ones() as usize)
            .sum()
    }

    fn valid_for_rows(self, rows: u16) -> bool {
        (rows..MAX_RENDER_ROWS).all(|row| !self.contains(row))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreparedStyle {
    pub bold: bool,
    pub underline: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreparedCell {
    pub column: u16,
    pub scalar: char,
    pub foreground: RenderColor,
    pub background: RenderColor,
    pub style: PreparedStyle,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedRow {
    row: u16,
    cells: Vec<PreparedCell>,
}

impl PreparedRow {
    fn with_capacity(row: u16, columns: u16) -> Self {
        Self {
            row,
            cells: Vec::with_capacity(columns as usize),
        }
    }

    pub fn row(&self) -> u16 {
        self.row
    }

    pub fn cells(&self) -> &[PreparedCell] {
        &self.cells
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrepareError {
    InvalidGeometry,
    InvalidCursor,
    InvalidCellCount,
    InvalidDamage,
    StaleGeneration,
    Overflow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreparationResult {
    pub generation: u64,
    pub rebuilt_rows: RowDamage,
    pub rebuilt_row_count: usize,
    pub rebuilt_cell_count: usize,
    pub full_rebuild: bool,
}

impl PreparationResult {
    pub fn did_rebuild(self) -> bool {
        !self.rebuilt_rows.is_empty()
    }
}

/// Reusable prepared presentation state for one visible terminal surface.
///
/// Rows keep their allocation across incremental generations. A fresh Metal
/// drawable may re-issue every cached row, but only rows selected by damage or
/// local cursor/geometry invalidation are rebuilt here.
#[derive(Debug, Default)]
pub struct PreparedSurface {
    generation: Option<u64>,
    rows: u16,
    columns: u16,
    cursor: CursorState,
    alternate_screen: bool,
    prepared_rows: Vec<PreparedRow>,
}

impl PreparedSurface {
    pub fn generation(&self) -> Option<u64> {
        self.generation
    }

    pub fn rows(&self) -> u16 {
        self.rows
    }

    pub fn columns(&self) -> u16 {
        self.columns
    }

    pub fn cursor(&self) -> CursorState {
        self.cursor
    }

    pub fn prepared_rows(&self) -> &[PreparedRow] {
        &self.prepared_rows
    }

    pub fn prepared_row(&self, row: u16) -> Option<&PreparedRow> {
        self.prepared_rows.get(row as usize)
    }

    pub fn prepare(
        &mut self,
        display: CommittedDisplay<'_>,
        mut damage: RowDamage,
        full_invalidation: bool,
    ) -> Result<PreparationResult, PrepareError> {
        display.validate()?;
        if !damage.valid_for_rows(display.rows) {
            return Err(PrepareError::InvalidDamage);
        }
        if self
            .generation
            .is_some_and(|generation| display.generation < generation)
        {
            return Err(PrepareError::StaleGeneration);
        }

        let initialized = self.generation.is_some();
        let geometry_changed = !initialized
            || self.rows != display.rows
            || self.columns != display.columns;
        let screen_changed = initialized && self.alternate_screen != display.alternate_screen;

        if geometry_changed || screen_changed || full_invalidation {
            damage = RowDamage::full(display.rows);
        } else if self.cursor != display.cursor {
            if self.cursor.visible && self.cursor.row < display.rows {
                damage.mark_row(self.cursor.row);
            }
            if display.cursor.visible {
                damage.mark_row(display.cursor.row);
            }
        }

        if geometry_changed {
            self.rebuild_row_storage(display.rows, display.columns);
        }

        let full_rebuild = damage.count() == display.rows as usize;
        let mut rebuilt_cell_count = 0usize;
        for row in 0..display.rows {
            if !damage.contains(row) {
                continue;
            }
            rebuilt_cell_count = rebuilt_cell_count
                .checked_add(self.rebuild_row(display, row)?)
                .ok_or(PrepareError::Overflow)?;
        }

        self.generation = Some(display.generation);
        self.rows = display.rows;
        self.columns = display.columns;
        self.cursor = display.cursor;
        self.alternate_screen = display.alternate_screen;

        Ok(PreparationResult {
            generation: display.generation,
            rebuilt_rows: damage,
            rebuilt_row_count: damage.count(),
            rebuilt_cell_count,
            full_rebuild,
        })
    }

    fn rebuild_row_storage(&mut self, rows: u16, columns: u16) {
        self.prepared_rows.clear();
        self.prepared_rows.reserve(rows as usize);
        for row in 0..rows {
            self.prepared_rows
                .push(PreparedRow::with_capacity(row, columns));
        }
    }

    fn rebuild_row(
        &mut self,
        display: CommittedDisplay<'_>,
        row: u16,
    ) -> Result<usize, PrepareError> {
        let columns = display.columns as usize;
        let first = (row as usize)
            .checked_mul(columns)
            .ok_or(PrepareError::Overflow)?;
        let last = first.checked_add(columns).ok_or(PrepareError::Overflow)?;
        let source = display
            .cells
            .get(first..last)
            .ok_or(PrepareError::InvalidCellCount)?;
        let target = self
            .prepared_rows
            .get_mut(row as usize)
            .ok_or(PrepareError::InvalidGeometry)?;

        target.cells.clear();
        if target.cells.capacity() < columns {
            target.cells.reserve(columns - target.cells.capacity());
        }
        for (column, cell) in source.iter().copied().enumerate() {
            let (foreground, background) = if cell.attributes.inverse {
                (cell.background, cell.foreground)
            } else {
                (cell.foreground, cell.background)
            };
            target.cells.push(PreparedCell {
                column: column as u16,
                scalar: cell.scalar,
                foreground,
                background,
                style: PreparedStyle {
                    bold: cell.attributes.bold,
                    underline: cell.attributes.underline,
                },
            });
        }
        Ok(source.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(scalar: char) -> RenderCell {
        RenderCell {
            scalar,
            ..RenderCell::default()
        }
    }

    fn display<'a>(
        generation: u64,
        rows: u16,
        columns: u16,
        cursor: CursorState,
        cells: &'a [RenderCell],
    ) -> CommittedDisplay<'a> {
        CommittedDisplay {
            generation,
            rows,
            columns,
            cursor,
            alternate_screen: false,
            cells,
        }
    }

    #[test]
    fn first_prepare_builds_every_visible_row() {
        let cells = vec![cell('a'), cell('b'), cell('c'), cell('d'), cell('e'), cell('f')];
        let mut surface = PreparedSurface::default();
        let result = surface
            .prepare(
                display(1, 2, 3, CursorState::default(), &cells),
                RowDamage::none(),
                false,
            )
            .unwrap();

        assert!(result.full_rebuild);
        assert_eq!(result.rebuilt_row_count, 2);
        assert_eq!(result.rebuilt_cell_count, 6);
        assert_eq!(surface.prepared_rows().len(), 2);
        assert_eq!(surface.prepared_row(0).unwrap().row(), 0);
        assert_eq!(surface.prepared_row(0).unwrap().cells()[0].column, 0);
        assert_eq!(surface.prepared_row(1).unwrap().cells()[2].scalar, 'f');
    }

    #[test]
    fn unchanged_generation_with_no_damage_does_no_cpu_rebuild() {
        let cells = vec![cell('a'), cell('b')];
        let mut surface = PreparedSurface::default();
        surface
            .prepare(
                display(7, 1, 2, CursorState::default(), &cells),
                RowDamage::none(),
                false,
            )
            .unwrap();

        let result = surface
            .prepare(
                display(7, 1, 2, CursorState::default(), &cells),
                RowDamage::none(),
                false,
            )
            .unwrap();

        assert!(!result.did_rebuild());
        assert_eq!(result.rebuilt_cell_count, 0);
    }

    #[test]
    fn partial_damage_rebuilds_only_the_marked_row() {
        let initial = vec![cell('a'), cell('b'), cell('c'), cell('d')];
        let changed = vec![cell('x'), cell('y'), cell('C'), cell('D')];
        let mut surface = PreparedSurface::default();
        surface
            .prepare(
                display(1, 2, 2, CursorState::default(), &initial),
                RowDamage::none(),
                false,
            )
            .unwrap();

        let damage = RowDamage::from_range(1, 1).unwrap();
        let result = surface
            .prepare(
                display(2, 2, 2, CursorState::default(), &changed),
                damage,
                false,
            )
            .unwrap();

        assert_eq!(result.rebuilt_row_count, 1);
        assert!(result.rebuilt_rows.contains(1));
        assert!(!result.rebuilt_rows.contains(0));
        assert_eq!(surface.prepared_row(0).unwrap().cells()[0].scalar, 'a');
        assert_eq!(surface.prepared_row(1).unwrap().cells()[0].scalar, 'C');
    }

    #[test]
    fn cursor_move_invalidates_old_and_new_rows_without_full_rebuild() {
        let cells = vec![cell('a'), cell('b'), cell('c'), cell('d')];
        let mut surface = PreparedSurface::default();
        surface
            .prepare(
                display(
                    1,
                    2,
                    2,
                    CursorState {
                        row: 0,
                        column: 0,
                        visible: true,
                    },
                    &cells,
                ),
                RowDamage::none(),
                false,
            )
            .unwrap();

        let result = surface
            .prepare(
                display(
                    2,
                    2,
                    2,
                    CursorState {
                        row: 1,
                        column: 1,
                        visible: true,
                    },
                    &cells,
                ),
                RowDamage::none(),
                false,
            )
            .unwrap();

        assert!(result.rebuilt_rows.contains(0));
        assert!(result.rebuilt_rows.contains(1));
        assert_eq!(result.rebuilt_row_count, 2);
    }

    #[test]
    fn inverse_is_resolved_without_changing_glyph_identity_inputs() {
        let cells = vec![RenderCell {
            scalar: 'Z',
            foreground: RenderColor::Indexed(1),
            background: RenderColor::Rgb { r: 2, g: 3, b: 4 },
            attributes: RenderAttributes {
                bold: true,
                underline: true,
                inverse: true,
            },
        }];
        let mut surface = PreparedSurface::default();
        surface
            .prepare(
                display(1, 1, 1, CursorState::default(), &cells),
                RowDamage::none(),
                false,
            )
            .unwrap();
        let prepared = surface.prepared_row(0).unwrap().cells()[0];

        assert_eq!(prepared.foreground, RenderColor::Rgb { r: 2, g: 3, b: 4 });
        assert_eq!(prepared.background, RenderColor::Indexed(1));
        assert_eq!(prepared.scalar, 'Z');
        assert!(prepared.style.bold);
        assert!(prepared.style.underline);
    }

    #[test]
    fn geometry_change_forces_full_rebuild() {
        let first = vec![cell('a'), cell('b')];
        let second = vec![cell('a'), cell('b'), cell('c'), cell('d')];
        let mut surface = PreparedSurface::default();
        surface
            .prepare(
                display(1, 1, 2, CursorState::default(), &first),
                RowDamage::none(),
                false,
            )
            .unwrap();
        let result = surface
            .prepare(
                display(2, 2, 2, CursorState::default(), &second),
                RowDamage::none(),
                false,
            )
            .unwrap();

        assert!(result.full_rebuild);
        assert_eq!(result.rebuilt_row_count, 2);
        assert_eq!(surface.rows(), 2);
        assert_eq!(surface.columns(), 2);
    }

    #[test]
    fn alternate_screen_transition_forces_full_rebuild() {
        let cells = vec![cell('a'), cell('b')];
        let mut surface = PreparedSurface::default();
        surface
            .prepare(
                display(1, 1, 2, CursorState::default(), &cells),
                RowDamage::none(),
                false,
            )
            .unwrap();
        let mut alternate = display(2, 1, 2, CursorState::default(), &cells);
        alternate.alternate_screen = true;
        let result = surface
            .prepare(alternate, RowDamage::none(), false)
            .unwrap();

        assert!(result.full_rebuild);
        assert_eq!(result.rebuilt_row_count, 1);
    }

    #[test]
    fn rejects_stale_generation_and_out_of_range_damage() {
        let cells = vec![cell('a')];
        let mut surface = PreparedSurface::default();
        surface
            .prepare(
                display(4, 1, 1, CursorState::default(), &cells),
                RowDamage::none(),
                false,
            )
            .unwrap();

        assert_eq!(
            surface.prepare(
                display(3, 1, 1, CursorState::default(), &cells),
                RowDamage::none(),
                false,
            ),
            Err(PrepareError::StaleGeneration)
        );

        let damage = RowDamage::from_range(1, 1).unwrap();
        assert_eq!(
            surface.prepare(
                display(5, 1, 1, CursorState::default(), &cells),
                damage,
                false,
            ),
            Err(PrepareError::InvalidDamage)
        );
    }

    #[test]
    fn rejects_invalid_geometry_cursor_and_cell_count() {
        let cells = vec![cell('a')];
        let mut surface = PreparedSurface::default();
        assert_eq!(
            surface.prepare(
                display(1, 0, 1, CursorState::default(), &[]),
                RowDamage::none(),
                false,
            ),
            Err(PrepareError::InvalidGeometry)
        );
        assert_eq!(
            surface.prepare(
                display(
                    1,
                    1,
                    1,
                    CursorState {
                        row: 1,
                        column: 0,
                        visible: true,
                    },
                    &cells,
                ),
                RowDamage::none(),
                false,
            ),
            Err(PrepareError::InvalidCursor)
        );
        assert_eq!(
            surface.prepare(
                display(1, 1, 2, CursorState::default(), &cells),
                RowDamage::none(),
                false,
            ),
            Err(PrepareError::InvalidCellCount)
        );
    }

    #[test]
    fn damage_union_is_fixed_size_and_allocation_free() {
        let mut damage = RowDamage::from_range(2, 2).unwrap();
        damage.union(RowDamage::from_range(5, 1).unwrap());

        assert_eq!(damage.count(), 3);
        assert!(damage.contains(2));
        assert!(damage.contains(3));
        assert!(damage.contains(5));
    }
}
