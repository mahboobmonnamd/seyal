//! Deterministic, platform-neutral preparation for Seyal's permanent terminal renderer.
//!
//! This crate owns no PTY, VT parser, canonical terminal state, native font object,
//! or GPU resource. It consumes only an already-committed disposable client display
//! state and maintains a reusable contiguous prepared-cell cache. Native renderers
//! consume that cache in one coarse transfer; there is no per-cell language callback.

pub const MAX_RENDER_ROWS: u16 = 256;
pub const MAX_RENDER_COLUMNS: u16 = 512;
const DAMAGE_WORDS: usize = MAX_RENDER_ROWS as usize / u64::BITS as usize;

pub const PREPARED_FLAG_BOLD: u16 = 1 << 0;
pub const PREPARED_FLAG_UNDERLINE: u16 = 1 << 1;

const COLOR_TAG_INDEXED: u32 = 0x0100_0000;
const COLOR_TAG_RGB: u32 = 0x0200_0000;

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
#[repr(u8)]
pub enum RenderCellRole {
    Empty = 0,
    Lead = 1,
    Continuation = 2,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderCell {
    pub scalar: char,
    pub role: RenderCellRole,
    pub width: u8,
    pub text: std::sync::Arc<[u8]>,
    pub foreground: RenderColor,
    pub background: RenderColor,
    pub attributes: RenderAttributes,
}

impl Default for RenderCell {
    fn default() -> Self {
        Self {
            scalar: ' ',
            role: RenderCellRole::Empty,
            width: 0,
            text: std::sync::Arc::from([]),
            foreground: RenderColor::Default,
            background: RenderColor::Default,
            attributes: RenderAttributes::default(),
        }
    }
}

pub const PREPARED_ROLE_EMPTY: u16 = 0;
pub const PREPARED_ROLE_LEAD: u16 = 1;
pub const PREPARED_ROLE_CONTINUATION: u16 = 2;
pub const PREPARED_FLAG_HAS_GRAPHEME: u16 = 1 << 4;

/// Read-only cell source used by the preparation engine.
///
/// Candidate-D adapters implement this trait over the already committed client
/// cache. Conversion therefore occurs only for rows selected by damage; the
/// renderer does not maintain a second full display cache merely to translate
/// cell types.
pub trait CellSource {
    fn len(&self) -> usize;
    fn cell(&self, index: usize) -> Option<RenderCell>;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl CellSource for [RenderCell] {
    fn len(&self) -> usize {
        <[RenderCell]>::len(self)
    }

    fn cell(&self, index: usize) -> Option<RenderCell> {
        self.get(index).cloned()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct CursorState {
    pub row: u16,
    pub column: u16,
    pub visible: bool,
    pub reserved: [u8; 3],
}

impl CursorState {
    pub const fn new(row: u16, column: u16, visible: bool) -> Self {
        Self {
            row,
            column,
            visible,
            reserved: [0; 3],
        }
    }
}

pub struct CommittedDisplay<'a, S: CellSource + ?Sized> {
    pub generation: u64,
    pub rows: u16,
    pub columns: u16,
    pub cursor: CursorState,
    pub alternate_screen: bool,
    pub cells: &'a S,
}

impl<S: CellSource + ?Sized> CommittedDisplay<'_, S> {
    fn validate(&self) -> Result<(), PrepareError> {
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

/// Row-granular invalidation matching Candidate-D's current complete-row delta
/// contract. The representation is fixed-size so generation coalescing never
/// allocates on the presentation path.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
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

    pub const fn words(self) -> [u64; DAMAGE_WORDS] {
        self.words
    }

    fn valid_for_rows(self, rows: u16) -> bool {
        (rows..MAX_RENDER_ROWS).all(|row| !self.contains(row))
    }
}

/// Stable internal native-facing cell representation.
///
/// This is not a public plugin ABI. The layout is deliberately fixed so one
/// coarse pointer/length transfer can expose the prepared current surface to
/// the Swift/Metal layer without one call per cell or per glyph.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct PreparedCell {
    pub scalar: u32,
    pub foreground: u32,
    pub background: u32,
    pub flags: u16,
    pub reserved: u16,
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
/// Geometry changes allocate one contiguous `rows × columns` cache. Ordinary
/// damage rewrites only selected row ranges in place. A fresh Metal drawable may
/// still re-issue the complete cache, but unchanged rows are not re-shaped,
/// converted, allocated or rebuilt merely because a new drawable was acquired.
#[derive(Debug, Default)]
pub struct PreparedSurface {
    generation: Option<u64>,
    rows: u16,
    columns: u16,
    cursor: CursorState,
    alternate_screen: bool,
    prepared_cells: Vec<PreparedCell>,
    grapheme_bytes: Vec<u8>,
    grapheme_row_ranges: Vec<std::ops::Range<usize>>,
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

    pub fn alternate_screen(&self) -> bool {
        self.alternate_screen
    }

    pub fn prepared_cells(&self) -> &[PreparedCell] {
        &self.prepared_cells
    }

    pub fn grapheme_bytes(&self) -> &[u8] {
        &self.grapheme_bytes
    }

    pub fn prepared_row(&self, row: u16) -> Option<&[PreparedCell]> {
        if row >= self.rows {
            return None;
        }
        let columns = self.columns as usize;
        let first = (row as usize).checked_mul(columns)?;
        let last = first.checked_add(columns)?;
        self.prepared_cells.get(first..last)
    }

    pub fn prepare<S: CellSource + ?Sized>(
        &mut self,
        display: CommittedDisplay<'_, S>,
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
        let geometry_changed =
            !initialized || self.rows != display.rows || self.columns != display.columns;
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
            let cell_count = (display.rows as usize)
                .checked_mul(display.columns as usize)
                .ok_or(PrepareError::Overflow)?;
            self.prepared_cells.clear();
            self.prepared_cells
                .resize(cell_count, PreparedCell::default());
            self.grapheme_bytes.clear();
            self.grapheme_row_ranges.clear();
            self.grapheme_row_ranges
                .resize(usize::from(display.rows), 0..0);
        }

        let rebuilt_row_count = damage.count();
        let full_rebuild = rebuilt_row_count == display.rows as usize;
        let mut rebuilt_cell_count = 0usize;
        for row in 0..display.rows {
            if !damage.contains(row) {
                continue;
            }
            rebuilt_cell_count = rebuilt_cell_count
                .checked_add(self.rebuild_row(&display, row)?)
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
            rebuilt_row_count,
            rebuilt_cell_count,
            full_rebuild,
        })
    }

    fn rebuild_row<S: CellSource + ?Sized>(
        &mut self,
        display: &CommittedDisplay<'_, S>,
        row: u16,
    ) -> Result<usize, PrepareError> {
        let columns = display.columns as usize;
        let first = (row as usize)
            .checked_mul(columns)
            .ok_or(PrepareError::Overflow)?;
        let last = first.checked_add(columns).ok_or(PrepareError::Overflow)?;
        if last > self.prepared_cells.len() {
            return Err(PrepareError::InvalidCellCount);
        }

        let mut row_graphemes = Vec::new();
        for offset in 0..columns {
            let cell = display
                .cells
                .cell(first + offset)
                .ok_or(PrepareError::InvalidCellCount)?;
            let (foreground, background) = if cell.attributes.inverse {
                (cell.background, cell.foreground)
            } else {
                (cell.foreground, cell.background)
            };
            let mut flags = 0u16;
            if cell.attributes.bold {
                flags |= PREPARED_FLAG_BOLD;
            }
            if cell.attributes.underline {
                flags |= PREPARED_FLAG_UNDERLINE;
            }
            let role = match cell.role {
                RenderCellRole::Empty => PREPARED_ROLE_EMPTY,
                RenderCellRole::Lead => PREPARED_ROLE_LEAD,
                RenderCellRole::Continuation => PREPARED_ROLE_CONTINUATION,
            };
            let width = cell.width.min(2) as u16;
            let mut reserved = role | (width << 2);
            let scalar = if cell.role == RenderCellRole::Continuation {
                0
            } else {
                cell.scalar as u32
            };
            let has_grapheme = cell.role == RenderCellRole::Lead
                && !cell.text.is_empty()
                && std::str::from_utf8(&cell.text)
                    .ok()
                    .is_some_and(|s| s.chars().count() > 1);
            if has_grapheme {
                reserved |= PREPARED_FLAG_HAS_GRAPHEME;
                let len = u16::try_from(cell.text.len()).map_err(|_| PrepareError::Overflow)?;
                row_graphemes
                    .try_reserve(usize::from(len).saturating_add(2))
                    .map_err(|_| PrepareError::Overflow)?;
                row_graphemes.extend_from_slice(&len.to_le_bytes());
                row_graphemes.extend_from_slice(&cell.text);
            }
            self.prepared_cells[first + offset] = PreparedCell {
                scalar,
                foreground: pack_color(foreground),
                background: pack_color(background),
                flags,
                reserved,
            };
        }
        self.replace_row_graphemes(row, &row_graphemes)?;
        Ok(columns)
    }

    fn replace_row_graphemes(&mut self, row: u16, replacement: &[u8]) -> Result<(), PrepareError> {
        let row = usize::from(row);
        let old = self
            .grapheme_row_ranges
            .get(row)
            .cloned()
            .ok_or(PrepareError::InvalidGeometry)?;
        if old.end > self.grapheme_bytes.len() || old.start > old.end {
            return Err(PrepareError::InvalidCellCount);
        }
        let old_len = old.end - old.start;
        if replacement.len() > old_len {
            self.grapheme_bytes
                .try_reserve(replacement.len() - old_len)
                .map_err(|_| PrepareError::Overflow)?;
        }
        self.grapheme_bytes
            .splice(old.clone(), replacement.iter().copied());
        let new_end = old
            .start
            .checked_add(replacement.len())
            .ok_or(PrepareError::Overflow)?;
        self.grapheme_row_ranges[row] = old.start..new_end;

        if replacement.len() >= old_len {
            let shift = replacement.len() - old_len;
            for range in &mut self.grapheme_row_ranges[row + 1..] {
                range.start = range
                    .start
                    .checked_add(shift)
                    .ok_or(PrepareError::Overflow)?;
                range.end = range.end.checked_add(shift).ok_or(PrepareError::Overflow)?;
            }
        } else {
            let shift = old_len - replacement.len();
            for range in &mut self.grapheme_row_ranges[row + 1..] {
                range.start = range
                    .start
                    .checked_sub(shift)
                    .ok_or(PrepareError::Overflow)?;
                range.end = range.end.checked_sub(shift).ok_or(PrepareError::Overflow)?;
            }
        }
        Ok(())
    }
}

pub const fn pack_color(color: RenderColor) -> u32 {
    match color {
        RenderColor::Default => 0,
        RenderColor::Indexed(index) => COLOR_TAG_INDEXED | index as u32,
        RenderColor::Rgb { r, g, b } => {
            COLOR_TAG_RGB | ((r as u32) << 16) | ((g as u32) << 8) | b as u32
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    struct CountingSource {
        cells: Vec<RenderCell>,
        reads: Cell<usize>,
    }

    impl CountingSource {
        fn new(cells: Vec<RenderCell>) -> Self {
            Self {
                cells,
                reads: Cell::new(0),
            }
        }

        fn reset_reads(&self) {
            self.reads.set(0);
        }

        fn reads(&self) -> usize {
            self.reads.get()
        }
    }

    impl CellSource for CountingSource {
        fn len(&self) -> usize {
            self.cells.len()
        }

        fn cell(&self, index: usize) -> Option<RenderCell> {
            self.reads.set(self.reads.get() + 1);
            self.cells.get(index).cloned()
        }
    }

    fn cell(scalar: char) -> RenderCell {
        RenderCell {
            scalar,
            role: RenderCellRole::Lead,
            width: 1,
            text: {
                let mut buf = [0u8; 4];
                let encoded = scalar.encode_utf8(&mut buf);
                std::sync::Arc::from(encoded.as_bytes().to_vec())
            },
            ..RenderCell::default()
        }
    }

    fn display<'a>(
        generation: u64,
        rows: u16,
        columns: u16,
        cursor: CursorState,
        cells: &'a [RenderCell],
    ) -> CommittedDisplay<'a, [RenderCell]> {
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
    fn first_prepare_builds_every_visible_row_into_one_contiguous_cache() {
        let cells = vec![
            cell('a'),
            cell('b'),
            cell('c'),
            cell('d'),
            cell('e'),
            cell('f'),
        ];
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
        assert_eq!(surface.prepared_cells().len(), 6);
        assert_eq!(surface.prepared_row(0).unwrap()[0].scalar, 'a' as u32);
        assert_eq!(surface.prepared_row(1).unwrap()[2].scalar, 'f' as u32);
        assert_eq!(
            surface.prepared_cells().as_ptr(),
            surface.prepared_row(0).unwrap().as_ptr()
        );
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
    fn preparation_reads_and_copies_only_rebuilt_rows() {
        let mut grapheme = cell('\u{1f469}');
        grapheme.text = std::sync::Arc::from("\u{1f469}\u{200d}\u{1f4bb}".as_bytes());
        let source = CountingSource::new(vec![
            grapheme,
            cell('b'),
            cell('c'),
            cell('d'),
            cell('e'),
            cell('f'),
        ]);
        let mut surface = PreparedSurface::default();
        let initial_cursor = CursorState::new(0, 0, true);
        let initial = CommittedDisplay {
            generation: 7,
            rows: 3,
            columns: 2,
            cursor: initial_cursor,
            alternate_screen: false,
            cells: &source,
        };
        let result = surface.prepare(initial, RowDamage::none(), false).unwrap();
        assert_eq!(result.rebuilt_cell_count, 6);
        assert_eq!(source.reads(), result.rebuilt_cell_count);

        source.reset_reads();
        let sidecar_before = surface.grapheme_bytes().to_vec();
        let sidecar_ptr_before = surface.grapheme_bytes().as_ptr();
        let unchanged = CommittedDisplay {
            generation: 7,
            rows: 3,
            columns: 2,
            cursor: initial_cursor,
            alternate_screen: false,
            cells: &source,
        };
        let result = surface
            .prepare(unchanged, RowDamage::none(), false)
            .unwrap();
        assert_eq!(result.rebuilt_cell_count, 0);
        assert_eq!(source.reads(), 0, "no-damage prepare read source cells");
        assert_eq!(surface.grapheme_bytes(), sidecar_before);
        assert_eq!(surface.grapheme_bytes().as_ptr(), sidecar_ptr_before);

        source.reset_reads();
        let cursor_only = CommittedDisplay {
            generation: 8,
            rows: 3,
            columns: 2,
            cursor: CursorState::new(1, 0, true),
            alternate_screen: false,
            cells: &source,
        };
        let result = surface
            .prepare(cursor_only, RowDamage::none(), false)
            .unwrap();
        assert_eq!(result.rebuilt_row_count, 2);
        assert_eq!(result.rebuilt_cell_count, 4);
        assert_eq!(source.reads(), result.rebuilt_cell_count);
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
        assert_eq!(surface.prepared_row(0).unwrap()[0].scalar, 'a' as u32);
        assert_eq!(surface.prepared_row(1).unwrap()[0].scalar, 'C' as u32);
    }

    #[test]
    fn partial_row_grapheme_replacement_preserves_physical_sidecar_order() {
        fn grapheme(text: &str) -> RenderCell {
            RenderCell {
                scalar: text.chars().next().unwrap(),
                role: RenderCellRole::Lead,
                width: 1,
                text: std::sync::Arc::from(text.as_bytes()),
                ..RenderCell::default()
            }
        }

        fn encoded(texts: &[&str]) -> Vec<u8> {
            let mut bytes = Vec::new();
            for text in texts {
                bytes.extend_from_slice(&(text.len() as u16).to_le_bytes());
                bytes.extend_from_slice(text.as_bytes());
            }
            bytes
        }

        let initial = vec![
            grapheme("a\u{301}"),
            cell('b'),
            grapheme("c\u{327}"),
            grapheme("d\u{308}"),
        ];
        let changed = vec![
            grapheme("\u{1f469}\u{200d}\u{1f4bb}"),
            cell('B'),
            initial[2].clone(),
            initial[3].clone(),
        ];
        let mut surface = PreparedSurface::default();
        surface
            .prepare(
                display(1, 2, 2, CursorState::default(), &initial),
                RowDamage::none(),
                false,
            )
            .unwrap();
        assert_eq!(
            surface.grapheme_bytes(),
            encoded(&["a\u{301}", "c\u{327}", "d\u{308}"])
        );

        surface
            .prepare(
                display(2, 2, 2, CursorState::default(), &changed),
                RowDamage::from_range(0, 1).unwrap(),
                false,
            )
            .unwrap();
        assert_eq!(
            surface.grapheme_bytes(),
            encoded(&["\u{1f469}\u{200d}\u{1f4bb}", "c\u{327}", "d\u{308}"])
        );
        assert_ne!(
            surface.prepared_cells()[0].reserved & PREPARED_FLAG_HAS_GRAPHEME,
            0
        );
        assert_ne!(
            surface.prepared_cells()[2].reserved & PREPARED_FLAG_HAS_GRAPHEME,
            0
        );
    }

    #[test]
    fn coalesced_damage_rebuilds_union_once_against_latest_state() {
        let initial = vec![
            cell('a'),
            cell('b'),
            cell('c'),
            cell('d'),
            cell('e'),
            cell('f'),
        ];
        let latest = vec![
            cell('A'),
            cell('B'),
            cell('C'),
            cell('D'),
            cell('E'),
            cell('F'),
        ];
        let mut surface = PreparedSurface::default();
        surface
            .prepare(
                display(1, 3, 2, CursorState::default(), &initial),
                RowDamage::none(),
                false,
            )
            .unwrap();

        let mut damage = RowDamage::from_range(0, 1).unwrap();
        damage.union(RowDamage::from_range(2, 1).unwrap());
        let result = surface
            .prepare(
                display(3, 3, 2, CursorState::default(), &latest),
                damage,
                false,
            )
            .unwrap();

        assert_eq!(result.rebuilt_row_count, 2);
        assert_eq!(surface.prepared_row(0).unwrap()[0].scalar, 'A' as u32);
        assert_eq!(surface.prepared_row(1).unwrap()[0].scalar, 'c' as u32);
        assert_eq!(surface.prepared_row(2).unwrap()[0].scalar, 'E' as u32);
    }

    #[test]
    fn cursor_move_invalidates_old_and_new_rows_without_full_rebuild() {
        let cells = vec![cell('a'), cell('b'), cell('c'), cell('d')];
        let mut surface = PreparedSurface::default();
        surface
            .prepare(
                display(1, 2, 2, CursorState::new(0, 0, true), &cells),
                RowDamage::none(),
                false,
            )
            .unwrap();

        let result = surface
            .prepare(
                display(2, 2, 2, CursorState::new(1, 1, true), &cells),
                RowDamage::none(),
                false,
            )
            .unwrap();

        assert!(result.rebuilt_rows.contains(0));
        assert!(result.rebuilt_rows.contains(1));
        assert_eq!(result.rebuilt_row_count, 2);
    }

    #[test]
    fn inverse_is_resolved_without_baking_color_into_glyph_identity() {
        let cells = vec![RenderCell {
            scalar: 'Z',
            role: RenderCellRole::Lead,
            width: 1,
            text: std::sync::Arc::from(b"Z".as_slice()),
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
        let prepared = surface.prepared_cells()[0];

        assert_eq!(
            prepared.foreground,
            pack_color(RenderColor::Rgb { r: 2, g: 3, b: 4 })
        );
        assert_eq!(prepared.background, pack_color(RenderColor::Indexed(1)));
        assert_eq!(prepared.scalar, 'Z' as u32);
        assert_eq!(prepared.flags, PREPARED_FLAG_BOLD | PREPARED_FLAG_UNDERLINE);
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
        assert_eq!(surface.prepared_cells().len(), 4);
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
        let alternate = CommittedDisplay {
            generation: 2,
            rows: 1,
            columns: 2,
            cursor: CursorState::default(),
            alternate_screen: true,
            cells: cells.as_slice(),
        };
        let result = surface
            .prepare(alternate, RowDamage::none(), false)
            .unwrap();

        assert!(result.full_rebuild);
        assert_eq!(result.rebuilt_row_count, 1);
        assert!(surface.alternate_screen());
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
                display(1, 1, 1, CursorState::new(1, 0, true), &cells),
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
        assert_eq!(damage.words().len(), DAMAGE_WORDS);
    }

    #[test]
    fn packed_colors_keep_default_indexed_and_rgb_domains_distinct() {
        assert_eq!(pack_color(RenderColor::Default), 0);
        assert_eq!(pack_color(RenderColor::Indexed(7)), COLOR_TAG_INDEXED | 7);
        assert_eq!(
            pack_color(RenderColor::Rgb {
                r: 0x12,
                g: 0x34,
                b: 0x56,
            }),
            COLOR_TAG_RGB | 0x0012_3456
        );
    }
}
