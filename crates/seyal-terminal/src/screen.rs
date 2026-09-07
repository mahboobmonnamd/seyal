use crate::{
    cursor::Cursor, damage::Mutation, grapheme_store::GraphemeStore, line::LineIdAllocator, Cell,
    CellRole, Color, CursorState, LineId, Style, TerminalError,
};
use std::collections::VecDeque;

const MAX_HISTORY_LINES: usize = 8_192;

#[derive(Clone, Copy, Debug)]
struct SavedCursor {
    cursor: Cursor,
    style: Style,
}

pub(crate) struct Screen {
    cols: u16,
    rows: u16,
    cells: Vec<Cell>,
    line_ids: Vec<LineId>,
    cursor: Cursor,
    pen: Style,
    saved_cursor: Option<SavedCursor>,
    history: VecDeque<(LineId, Vec<Cell>)>,
}

impl Screen {
    pub(crate) fn new(
        cols: u16,
        rows: u16,
        line_ids: &mut LineIdAllocator,
    ) -> Result<Self, TerminalError> {
        if cols == 0 || rows == 0 {
            return Err(TerminalError::InvalidSize);
        }
        if cols > crate::terminal::MAX_TERMINAL_COLUMNS || rows > crate::terminal::MAX_TERMINAL_ROWS
        {
            return Err(TerminalError::InvalidSize);
        }
        if !line_ids.can_allocate(usize::from(rows)) {
            return Err(TerminalError::LineIdentityExhausted);
        }

        let mut row_ids = Vec::with_capacity(usize::from(rows));
        for _ in 0..rows {
            row_ids.push(line_ids.allocate()?);
        }

        Ok(Self {
            cols,
            rows,
            cells: vec![Cell::default(); usize::from(cols) * usize::from(rows)],
            line_ids: row_ids,
            cursor: Cursor::default(),
            pen: Style::default(),
            saved_cursor: None,
            history: VecDeque::new(),
        })
    }

    pub(crate) fn cols(&self) -> u16 {
        self.cols
    }

    pub(crate) fn rows(&self) -> u16 {
        self.rows
    }

    pub(crate) fn pen(&self) -> Style {
        self.pen
    }

    pub(crate) fn inherit_pen_for_clean_buffer(&mut self, pen: Style, store: &mut GraphemeStore) {
        self.pen = pen;
        self.release_all_payloads(store);
        self.cells.fill(Cell::blank(pen.bg));
    }

    pub(crate) fn release_all_payloads(&mut self, store: &mut GraphemeStore) {
        for cell in &self.cells {
            Self::release_cell(*cell, store);
        }
        for (_, row) in &self.history {
            for cell in row {
                Self::release_cell(*cell, store);
            }
        }
    }

    fn release_cell(cell: Cell, store: &mut GraphemeStore) {
        if cell.role == CellRole::Lead {
            store.release(cell.store_id);
        }
    }

    /// Clears a lead or continuation so no orphan half remains (SPEC-011 §7.3).
    pub(crate) fn clear_unit_at(
        &mut self,
        col: u16,
        row: u16,
        store: &mut GraphemeStore,
    ) -> Mutation {
        if col >= self.cols || row >= self.rows {
            return Mutation::none();
        }
        let index = self.index(col, row);
        let cell = self.cells[index];
        let (lead_col, lead_row) = match cell.role {
            CellRole::Continuation if col > 0 => (col - 1, row),
            CellRole::Lead | CellRole::Empty => (col, row),
            CellRole::Continuation => (col, row),
        };
        let lead_index = self.index(lead_col, lead_row);
        let lead = self.cells[lead_index];
        if lead.role == CellRole::Lead {
            Self::release_cell(lead, store);
            let width = lead.width.max(1);
            self.cells[lead_index] = Cell::blank(self.pen.bg);
            if width >= 2 && lead_col + 1 < self.cols {
                let cont = self.index(lead_col + 1, lead_row);
                if self.cells[cont].role == CellRole::Continuation {
                    self.cells[cont] = Cell::blank(self.pen.bg);
                }
            }
        } else {
            self.cells[index] = Cell::blank(self.pen.bg);
        }
        Mutation::row(row)
    }

    pub(crate) fn cursor(&self, visible: bool) -> CursorState {
        CursorState {
            col: self.cursor.col,
            row: self.cursor.row,
            visible,
        }
    }

    pub(crate) fn cell(&self, col: u16, row: u16) -> Option<Cell> {
        if col >= self.cols || row >= self.rows {
            return None;
        }
        Some(self.cells[self.index(col, row)])
    }

    pub(crate) fn line_id(&self, row: u16) -> Option<LineId> {
        self.line_ids.get(usize::from(row)).copied()
    }

    /// Oldest-to-newest retained primary history entries (storage order).
    pub(crate) fn history_entries(&self) -> impl Iterator<Item = (LineId, &[Cell])> {
        self.history
            .iter()
            .map(|(id, cells)| (*id, cells.as_slice()))
    }

    pub(crate) fn cell_row(&self, row: u16) -> Option<&[Cell]> {
        if row >= self.rows {
            return None;
        }
        let start = usize::from(row) * usize::from(self.cols);
        Some(&self.cells[start..start + usize::from(self.cols)])
    }

    /// Builds the next screen buffers and allocates any new line identities
    /// without mutating the live screen. Dropping the result burns allocated
    /// identities but leaves observable geometry/damage unchanged.
    pub(crate) fn prepare_resize(
        &self,
        cols: u16,
        rows: u16,
        line_ids: &mut LineIdAllocator,
    ) -> Result<PreparedScreen, TerminalError> {
        if cols == 0 || rows == 0 {
            return Err(TerminalError::InvalidSize);
        }
        if cols > crate::terminal::MAX_TERMINAL_COLUMNS || rows > crate::terminal::MAX_TERMINAL_ROWS
        {
            return Err(TerminalError::InvalidSize);
        }
        if cols == self.cols && rows == self.rows {
            return Ok(PreparedScreen::noop());
        }

        let old_cols = self.cols;
        let old_rows = self.rows;
        let new_row_count = usize::from(rows.saturating_sub(old_rows));
        if !line_ids.can_allocate(new_row_count) {
            return Err(TerminalError::LineIdentityExhausted);
        }

        let mut next = vec![Cell::default(); usize::from(cols) * usize::from(rows)];
        let copy_cols = old_cols.min(cols);
        let copy_rows = old_rows.min(rows);
        for row in 0..copy_rows {
            let old_start = usize::from(row) * usize::from(old_cols);
            let new_start = usize::from(row) * usize::from(cols);
            let count = usize::from(copy_cols);
            next[new_start..new_start + count]
                .copy_from_slice(&self.cells[old_start..old_start + count]);
        }

        let mut next_line_ids = Vec::with_capacity(usize::from(rows));
        for row in 0..rows {
            if row < old_rows {
                next_line_ids.push(self.line_ids[usize::from(row)]);
            } else {
                next_line_ids.push(line_ids.allocate()?);
            }
        }

        let mut cursor = self.cursor;
        cursor.clamp(cols, rows);
        let mut saved_cursor = self.saved_cursor;
        if let Some(saved) = &mut saved_cursor {
            saved.cursor.clamp(cols, rows);
        }

        Ok(PreparedScreen {
            cols,
            rows,
            cells: next,
            line_ids: next_line_ids,
            cursor,
            saved_cursor,
            unchanged: false,
        })
    }

    /// Infallible swap of a prepared resize into the live screen.
    pub(crate) fn commit_prepared(&mut self, prepared: PreparedScreen) -> Mutation {
        if prepared.unchanged {
            return Mutation::none();
        }
        let rows = prepared.rows;
        self.cols = prepared.cols;
        self.rows = prepared.rows;
        self.cells = prepared.cells;
        self.line_ids = prepared.line_ids;
        self.cursor = prepared.cursor;
        self.saved_cursor = prepared.saved_cursor;
        Mutation::full(rows)
    }

    /// Places a completed canonical unit at the cursor (after wrap handling).
    /// Returns `(mutation, soft_wrapped, lead_col, lead_row)`.
    pub(crate) fn place_new_unit(
        &mut self,
        lead: Cell,
        wraparound: bool,
        line_ids: &mut LineIdAllocator,
        store: &mut GraphemeStore,
    ) -> Result<(Mutation, bool, u16, u16), TerminalError> {
        let width = lead.width.max(1);
        let mut mutation = Mutation::none();
        let mut soft_wrapped = false;

        if self.cursor.pending_wrap {
            if wraparound {
                let wrap_mutation = self.line_feed(line_ids)?;
                self.cursor.col = 0;
                mutation = mutation.merge(wrap_mutation);
                soft_wrapped = true;
            } else {
                self.cursor.pending_wrap = false;
            }
        }

        if width >= 2
            && self.cursor.col + 1 >= self.cols
            && (self.cursor.col == self.cols - 1 || self.cursor.col + 1 > self.cols - 1)
        {
            if wraparound {
                let wrap_mutation = self.line_feed(line_ids)?;
                self.cursor.col = 0;
                mutation = mutation.merge(wrap_mutation);
                soft_wrapped = true;
            } else {
                return Ok((mutation, soft_wrapped, self.cursor.col, self.cursor.row));
            }
        }

        let col = self.cursor.col;
        let row = self.cursor.row;
        mutation = mutation.merge(self.clear_unit_at(col, row, store));
        if width >= 2 && col + 1 < self.cols {
            mutation = mutation.merge(self.clear_unit_at(col + 1, row, store));
        }

        let index = self.index(col, row);
        self.cells[index] = lead;
        if width >= 2 {
            if col + 1 >= self.cols {
                Self::release_cell(lead, store);
                self.cells[index] = Cell::blank(self.pen.bg);
                return Ok((mutation, soft_wrapped, col, row));
            }
            let cont_index = self.index(col + 1, row);
            self.cells[cont_index] = Cell::continuation();
        }
        mutation = mutation.merge(Mutation::row(row));

        let advance = u16::from(width);
        let next_col = col.saturating_add(advance);
        if next_col >= self.cols {
            self.cursor.col = self.cols - 1;
            self.cursor.pending_wrap = wraparound;
        } else {
            self.cursor.col = next_col;
            self.cursor.pending_wrap = false;
        }
        Ok((mutation, soft_wrapped, col, row))
    }

    /// Overwrites an existing active lead in place (append / late width change).
    pub(crate) fn replace_active_lead(
        &mut self,
        col: u16,
        row: u16,
        lead: Cell,
        previous_width: u8,
        store: &mut GraphemeStore,
    ) -> Mutation {
        let mut mutation = Mutation::none();
        if previous_width >= 2 && col + 1 < self.cols {
            let cont = self.index(col + 1, row);
            if self.cells[cont].role == CellRole::Continuation {
                self.cells[cont] = Cell::blank(self.pen.bg);
            }
        }
        // Release previous store via clear of current lead only if different id.
        let index = self.index(col, row);
        let old = self.cells[index];
        if old.role == CellRole::Lead && old.store_id != lead.store_id {
            Self::release_cell(old, store);
        }
        self.cells[index] = lead;
        if lead.width >= 2 && col + 1 < self.cols {
            mutation = mutation.merge(self.clear_unit_at(col + 1, row, store));
            let cont_index = self.index(col + 1, row);
            self.cells[cont_index] = Cell::continuation();
        }
        mutation.merge(Mutation::row(row))
    }

    pub(crate) fn pending_wrap(&self) -> bool {
        self.cursor.pending_wrap
    }

    pub(crate) fn set_pending_wrap(&mut self, pending: bool) {
        self.cursor.pending_wrap = pending;
    }

    pub(crate) fn execute(
        &mut self,
        byte: u8,
        line_ids: &mut LineIdAllocator,
    ) -> Result<Mutation, TerminalError> {
        Ok(match byte {
            0x08 => self.backspace(),
            0x09 => self.tab(),
            0x0a..=0x0c => return self.line_feed(line_ids),
            0x0d => self.carriage_return(),
            _ => Mutation::none(),
        })
    }

    pub(crate) fn cursor_up(&mut self, count: u16) -> Mutation {
        let old = self.cursor.row;
        self.cursor.row = self.cursor.row.saturating_sub(count);
        self.cursor.pending_wrap = false;
        Mutation::rows(old, self.cursor.row)
    }

    pub(crate) fn cursor_down(&mut self, count: u16) -> Mutation {
        let old = self.cursor.row;
        self.cursor.row = self
            .cursor
            .row
            .saturating_add(count)
            .min(self.rows.saturating_sub(1));
        self.cursor.pending_wrap = false;
        Mutation::rows(old, self.cursor.row)
    }

    pub(crate) fn cursor_forward(&mut self, count: u16) -> Mutation {
        let row = self.cursor.row;
        self.cursor.col = self
            .cursor
            .col
            .saturating_add(count)
            .min(self.cols.saturating_sub(1));
        self.cursor.pending_wrap = false;
        Mutation::row(row)
    }

    pub(crate) fn cursor_back(&mut self, count: u16) -> Mutation {
        let row = self.cursor.row;
        self.cursor.col = self.cursor.col.saturating_sub(count);
        self.cursor.pending_wrap = false;
        Mutation::row(row)
    }

    pub(crate) fn set_cursor(&mut self, row: u16, col: u16) -> Mutation {
        let old = self.cursor.row;
        self.cursor.row = row.min(self.rows.saturating_sub(1));
        self.cursor.col = col.min(self.cols.saturating_sub(1));
        self.cursor.pending_wrap = false;
        Mutation::rows(old, self.cursor.row)
    }

    pub(crate) fn set_col(&mut self, col: u16) -> Mutation {
        let row = self.cursor.row;
        self.cursor.col = col.min(self.cols.saturating_sub(1));
        self.cursor.pending_wrap = false;
        Mutation::row(row)
    }

    pub(crate) fn set_row(&mut self, row: u16) -> Mutation {
        let old = self.cursor.row;
        self.cursor.row = row.min(self.rows.saturating_sub(1));
        self.cursor.pending_wrap = false;
        Mutation::rows(old, self.cursor.row)
    }

    pub(crate) fn erase_display(&mut self, mode: u16, store: &mut GraphemeStore) -> Mutation {
        let blank = Cell::blank(self.pen.bg);
        let cursor_index = self.index(self.cursor.col, self.cursor.row);
        match mode {
            0 => {
                for cell in &self.cells[cursor_index..] {
                    Self::release_cell(*cell, store);
                }
                self.cells[cursor_index..].fill(blank);
                Mutation::rows(self.cursor.row, self.rows - 1)
            }
            1 => {
                for cell in &self.cells[..=cursor_index] {
                    Self::release_cell(*cell, store);
                }
                self.cells[..=cursor_index].fill(blank);
                Mutation::rows(0, self.cursor.row)
            }
            2 => {
                for cell in &self.cells {
                    Self::release_cell(*cell, store);
                }
                self.cells.fill(blank);
                Mutation::full(self.rows)
            }
            _ => Mutation::none(),
        }
    }

    pub(crate) fn erase_line(&mut self, mode: u16, store: &mut GraphemeStore) -> Mutation {
        let blank = Cell::blank(self.pen.bg);
        let row = self.cursor.row;
        let start = usize::from(row) * usize::from(self.cols);
        let end = start + usize::from(self.cols);
        let col = usize::from(self.cursor.col);
        match mode {
            0 => {
                for cell in &self.cells[start + col..end] {
                    Self::release_cell(*cell, store);
                }
                self.cells[start + col..end].fill(blank);
            }
            1 => {
                for cell in &self.cells[start..=start + col] {
                    Self::release_cell(*cell, store);
                }
                self.cells[start..=start + col].fill(blank);
            }
            2 => {
                for cell in &self.cells[start..end] {
                    Self::release_cell(*cell, store);
                }
                self.cells[start..end].fill(blank);
            }
            _ => return Mutation::none(),
        }
        Mutation::row(row)
    }

    pub(crate) fn save_cursor(&mut self) {
        self.saved_cursor = Some(SavedCursor {
            cursor: self.cursor,
            style: self.pen,
        });
    }

    pub(crate) fn restore_cursor(&mut self) -> Mutation {
        let Some(saved) = self.saved_cursor else {
            return Mutation::none();
        };
        let old = self.cursor.row;
        self.cursor = saved.cursor;
        self.cursor.clamp(self.cols, self.rows);
        self.pen = saved.style;
        Mutation::rows(old, self.cursor.row)
    }

    pub(crate) fn apply_sgr(&mut self, params: &[u16]) -> bool {
        if params.is_empty() {
            self.pen = Style::default();
            return false;
        }

        let mut deferred = false;
        let mut index = 0;
        while index < params.len() {
            match params[index] {
                0 => self.pen = Style::default(),
                1 => self.pen.bold = true,
                22 => self.pen.bold = false,
                4 => self.pen.underline = true,
                24 => self.pen.underline = false,
                7 => self.pen.inverse = true,
                27 => self.pen.inverse = false,
                30..=37 => self.pen.fg = Color::Indexed((params[index] - 30) as u8),
                39 => self.pen.fg = Color::Default,
                40..=47 => self.pen.bg = Color::Indexed((params[index] - 40) as u8),
                49 => self.pen.bg = Color::Default,
                90..=97 => self.pen.fg = Color::Indexed((params[index] - 90 + 8) as u8),
                100..=107 => self.pen.bg = Color::Indexed((params[index] - 100 + 8) as u8),
                38 | 48 => {
                    let foreground = params[index] == 38;
                    match params.get(index + 1).copied() {
                        Some(5) if index + 2 < params.len() => {
                            let color = Color::Indexed(params[index + 2].min(255) as u8);
                            if foreground {
                                self.pen.fg = color;
                            } else {
                                self.pen.bg = color;
                            }
                            index += 2;
                        }
                        Some(2) if index + 4 < params.len() => {
                            let color = Color::Rgb {
                                r: params[index + 2].min(255) as u8,
                                g: params[index + 3].min(255) as u8,
                                b: params[index + 4].min(255) as u8,
                            };
                            if foreground {
                                self.pen.fg = color;
                            } else {
                                self.pen.bg = color;
                            }
                            index += 4;
                        }
                        _ => deferred = true,
                    }
                }
                _ => deferred = true,
            }
            index += 1;
        }
        deferred
    }

    fn backspace(&mut self) -> Mutation {
        let row = self.cursor.row;
        self.cursor.col = self.cursor.col.saturating_sub(1);
        self.cursor.pending_wrap = false;
        Mutation::row(row)
    }

    fn tab(&mut self) -> Mutation {
        let row = self.cursor.row;
        let next = (self.cursor.col / 8).saturating_add(1).saturating_mul(8);
        self.cursor.col = next.min(self.cols.saturating_sub(1));
        self.cursor.pending_wrap = false;
        Mutation::row(row)
    }

    fn carriage_return(&mut self) -> Mutation {
        let row = self.cursor.row;
        self.cursor.col = 0;
        self.cursor.pending_wrap = false;
        Mutation::row(row)
    }

    fn line_feed(&mut self, line_ids: &mut LineIdAllocator) -> Result<Mutation, TerminalError> {
        let old = self.cursor.row;
        if self.cursor.row < self.rows - 1 {
            self.cursor.pending_wrap = false;
            self.cursor.row += 1;
            return Ok(Mutation::rows(old, self.cursor.row));
        }

        let new_line_id = line_ids.allocate()?;
        let evicted_id = self.line_ids[0];
        let evicted = self.cells[..usize::from(self.cols)].to_vec();
        if self.history.len() == MAX_HISTORY_LINES {
            self.history.pop_front();
        }
        self.history.push_back((evicted_id, evicted));
        self.cursor.pending_wrap = false;
        let row_width = usize::from(self.cols);
        self.cells.copy_within(row_width.., 0);
        let last_row_start = self.cells.len() - row_width;
        // Last row was shifted up; blank without releasing shifted cells.
        self.cells[last_row_start..].fill(Cell::blank(self.pen.bg));
        self.line_ids.copy_within(1.., 0);
        let last = self.line_ids.len() - 1;
        self.line_ids[last] = new_line_id;
        Ok(Mutation::full(self.rows))
    }

    fn index(&self, col: u16, row: u16) -> usize {
        usize::from(row) * usize::from(self.cols) + usize::from(col)
    }
}

/// Fallible resize preparation held until canonical commit.
pub(crate) struct PreparedScreen {
    cols: u16,
    rows: u16,
    cells: Vec<Cell>,
    line_ids: Vec<LineId>,
    cursor: Cursor,
    saved_cursor: Option<SavedCursor>,
    unchanged: bool,
}

impl PreparedScreen {
    fn noop() -> Self {
        Self {
            cols: 0,
            rows: 0,
            cells: Vec::new(),
            line_ids: Vec::new(),
            cursor: Cursor::default(),
            saved_cursor: None,
            unchanged: true,
        }
    }
}
