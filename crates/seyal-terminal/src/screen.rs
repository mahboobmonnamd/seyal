use crate::{
    Cell, Color, CursorState, LineId, Style, TerminalError, cursor::Cursor, damage::Mutation,
    line::LineIdAllocator,
};

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

    pub(crate) fn inherit_pen_for_clean_buffer(&mut self, pen: Style) {
        self.pen = pen;
        self.cells.fill(Cell::blank(pen.bg));
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

    pub(crate) fn resize(
        &mut self,
        cols: u16,
        rows: u16,
        line_ids: &mut LineIdAllocator,
    ) -> Result<Mutation, TerminalError> {
        if cols == 0 || rows == 0 {
            return Err(TerminalError::InvalidSize);
        }
        if cols == self.cols && rows == self.rows {
            return Ok(Mutation::none());
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

        self.cols = cols;
        self.rows = rows;
        self.cells = next;
        self.line_ids = next_line_ids;
        self.cursor.clamp(cols, rows);
        if let Some(saved) = &mut self.saved_cursor {
            saved.cursor.clamp(cols, rows);
        }
        Ok(Mutation::full(rows))
    }

    pub(crate) fn print(
        &mut self,
        character: char,
        line_ids: &mut LineIdAllocator,
    ) -> Result<Mutation, TerminalError> {
        let mut mutation = Mutation::none();
        if self.cursor.pending_wrap {
            self.cursor.pending_wrap = false;
            self.cursor.col = 0;
            mutation = mutation.merge(self.line_feed(line_ids)?);
        }

        let row = self.cursor.row;
        let index = self.index(self.cursor.col, row);
        self.cells[index] = Cell {
            character,
            style: self.pen,
        };
        mutation = mutation.merge(Mutation::row(row));

        if self.cursor.col == self.cols - 1 {
            self.cursor.pending_wrap = true;
        } else {
            self.cursor.col += 1;
        }
        Ok(mutation)
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

    pub(crate) fn erase_display(&mut self, mode: u16) -> Mutation {
        let blank = Cell::blank(self.pen.bg);
        let cursor_index = self.index(self.cursor.col, self.cursor.row);
        match mode {
            0 => {
                self.cells[cursor_index..].fill(blank);
                Mutation::rows(self.cursor.row, self.rows - 1)
            }
            1 => {
                self.cells[..=cursor_index].fill(blank);
                Mutation::rows(0, self.cursor.row)
            }
            2 => {
                self.cells.fill(blank);
                Mutation::full(self.rows)
            }
            _ => Mutation::none(),
        }
    }

    pub(crate) fn erase_line(&mut self, mode: u16) -> Mutation {
        let blank = Cell::blank(self.pen.bg);
        let row = self.cursor.row;
        let start = usize::from(row) * usize::from(self.cols);
        let end = start + usize::from(self.cols);
        let col = usize::from(self.cursor.col);
        match mode {
            0 => self.cells[start + col..end].fill(blank),
            1 => self.cells[start..=start + col].fill(blank),
            2 => self.cells[start..end].fill(blank),
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

    fn line_feed(
        &mut self,
        line_ids: &mut LineIdAllocator,
    ) -> Result<Mutation, TerminalError> {
        let old = self.cursor.row;
        self.cursor.pending_wrap = false;
        if self.cursor.row < self.rows - 1 {
            self.cursor.row += 1;
            return Ok(Mutation::rows(old, self.cursor.row));
        }

        let new_line_id = line_ids.allocate()?;
        let row_width = usize::from(self.cols);
        self.cells.copy_within(row_width.., 0);
        let last_row_start = self.cells.len() - row_width;
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
