use crate::{
    cursor::Cursor,
    damage::Mutation,
    grapheme_store::GraphemeStore,
    history::{HistoryBreakAfter, HistoryLine, HistoryStore},
    line::LineIdAllocator,
    Cell, CellRole, Color, CursorState, LineId, Style, TerminalError,
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
    row_breaks: Vec<Option<HistoryBreakAfter>>,
    cursor: Cursor,
    pen: Style,
    saved_cursor: Option<SavedCursor>,
    history: HistoryStore,
    retain_history: bool,
    /// Inclusive 0-based DECSTBM top margin.
    scroll_top: u16,
    /// Inclusive 0-based DECSTBM bottom margin.
    scroll_bottom: u16,
}

impl Screen {
    pub(crate) fn new(
        cols: u16,
        rows: u16,
        line_ids: &mut LineIdAllocator,
        retain_history: bool,
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
            row_breaks: vec![None; usize::from(rows)],
            cursor: Cursor::default(),
            pen: Style::default(),
            saved_cursor: None,
            history: HistoryStore::default(),
            retain_history,
            scroll_top: 0,
            scroll_bottom: rows.saturating_sub(1),
        })
    }

    /// DECSTBM. Parameters are 1-based inclusive margins; `0` means default.
    /// Invalid ranges (top >= bottom after defaults) leave the region unchanged.
    /// Successful set moves the cursor to the absolute origin (1,1).
    pub(crate) fn set_scroll_region(&mut self, top: u16, bottom: u16) -> Mutation {
        let top = if top == 0 { 1 } else { top };
        let bottom = if bottom == 0 { self.rows } else { bottom };
        if top < 1 || bottom > self.rows || top >= bottom {
            return Mutation::none();
        }
        self.scroll_top = top - 1;
        self.scroll_bottom = bottom - 1;
        self.set_cursor(0, 0)
    }

    fn region_is_full_screen(&self) -> bool {
        self.scroll_top == 0 && self.scroll_bottom + 1 == self.rows
    }

    fn clamp_scroll_region_to_geometry(&mut self) {
        if self.rows == 0 {
            self.scroll_top = 0;
            self.scroll_bottom = 0;
            return;
        }
        let max_row = self.rows - 1;
        if self.scroll_top > max_row {
            self.scroll_top = 0;
        }
        if self.scroll_bottom > max_row || self.scroll_bottom <= self.scroll_top {
            self.scroll_bottom = max_row;
            if self.scroll_top >= self.scroll_bottom {
                self.scroll_top = 0;
            }
        }
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
    pub(crate) fn history_entries(&self) -> impl Iterator<Item = &HistoryLine> {
        self.history.entries()
    }

    pub(crate) fn history(&self) -> &HistoryStore {
        &self.history
    }

    pub(crate) fn history_mut(&mut self) -> &mut HistoryStore {
        &mut self.history
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
        store: &GraphemeStore,
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
        let mut next_breaks = vec![None; usize::from(rows)];
        if self.retain_history && cols != old_cols {
            let mut source = HistoryStore::default();
            let last_content_row = (0..old_rows)
                .rev()
                .find(|row| {
                    let start = usize::from(*row) * usize::from(old_cols);
                    self.cells[start..start + usize::from(old_cols)]
                        .iter()
                        .any(|cell| cell.role != CellRole::Empty)
                })
                .unwrap_or(0);
            for row in 0..=last_content_row {
                let row_start = usize::from(row) * usize::from(old_cols);
                let break_after = self.row_breaks[usize::from(row)].unwrap_or_else(|| {
                    if row + 1 == old_rows {
                        HistoryBreakAfter::HardBreak
                    } else {
                        HistoryBreakAfter::SoftWrap
                    }
                });
                let mut content_end = usize::from(old_cols);
                if row == last_content_row {
                    content_end = self.cells[row_start..row_start + usize::from(old_cols)]
                        .iter()
                        .rposition(|cell| cell.role != CellRole::Empty)
                        .map_or(1, |index| index + 1);
                }
                source.append_row(
                    self.line_ids[usize::from(row)],
                    break_after,
                    &self.cells[row_start..row_start + content_end],
                    store,
                );
            }
            let reflowed = source.reflow(cols, usize::MAX);
            let first = reflowed.len().saturating_sub(usize::from(rows));
            for (row, projection) in reflowed.iter().skip(first).enumerate() {
                let start = row * usize::from(cols);
                let count = projection.cells.len().min(usize::from(cols));
                next[start..start + count].copy_from_slice(&projection.cells[..count]);
            }
        } else {
            let copy_cols = old_cols.min(cols);
            let copy_rows = old_rows.min(rows);
            for row in 0..copy_rows {
                let old_start = usize::from(row) * usize::from(old_cols);
                let new_start = usize::from(row) * usize::from(cols);
                let count = usize::from(copy_cols);
                next[new_start..new_start + count]
                    .copy_from_slice(&self.cells[old_start..old_start + count]);
                next_breaks[usize::from(row)] = self.row_breaks[usize::from(row)];
            }
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
            row_breaks: next_breaks,
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
        self.row_breaks = prepared.row_breaks;
        self.cursor = prepared.cursor;
        self.saved_cursor = prepared.saved_cursor;
        self.clamp_scroll_region_to_geometry();
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
                let wrap_mutation =
                    self.line_feed(line_ids, HistoryBreakAfter::SoftWrap, Some(&mut *store))?;
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
                let wrap_mutation =
                    self.line_feed(line_ids, HistoryBreakAfter::SoftWrap, Some(&mut *store))?;
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
        store: &mut GraphemeStore,
    ) -> Result<Mutation, TerminalError> {
        Ok(match byte {
            0x08 => self.backspace(),
            0x09 => self.tab(),
            0x0a..=0x0c => {
                return self.line_feed(line_ids, HistoryBreakAfter::HardBreak, Some(store));
            }
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

    /// IND / LF within the scroll region: scroll at the bottom margin.
    fn line_feed(
        &mut self,
        line_ids: &mut LineIdAllocator,
        break_after: HistoryBreakAfter,
        store: Option<&mut GraphemeStore>,
    ) -> Result<Mutation, TerminalError> {
        self.cursor.pending_wrap = false;
        let old = self.cursor.row;
        self.row_breaks[usize::from(self.cursor.row)] = Some(break_after);
        if self.cursor.row == self.scroll_bottom {
            return self.scroll_up(1, line_ids, store, break_after);
        }
        if self.cursor.row < self.rows.saturating_sub(1) {
            self.cursor.row += 1;
            return Ok(Mutation::rows(old, self.cursor.row));
        }
        Ok(Mutation::row(old))
    }

    /// ESC D — Index (same scroll rules as LF, without carriage return).
    pub(crate) fn index_down(
        &mut self,
        line_ids: &mut LineIdAllocator,
        store: &mut GraphemeStore,
    ) -> Result<Mutation, TerminalError> {
        self.line_feed(line_ids, HistoryBreakAfter::HardBreak, Some(store))
    }

    /// ESC M — Reverse Index.
    pub(crate) fn reverse_index(
        &mut self,
        line_ids: &mut LineIdAllocator,
        store: &mut GraphemeStore,
    ) -> Result<Mutation, TerminalError> {
        self.cursor.pending_wrap = false;
        let old = self.cursor.row;
        if self.cursor.row == self.scroll_top {
            return self.scroll_down(1, line_ids, Some(store));
        }
        if self.cursor.row > 0 {
            self.cursor.row -= 1;
            return Ok(Mutation::rows(old, self.cursor.row));
        }
        Ok(Mutation::row(old))
    }

    /// ESC E — Next Line (CR + Index).
    pub(crate) fn next_line(
        &mut self,
        line_ids: &mut LineIdAllocator,
        store: &mut GraphemeStore,
    ) -> Result<Mutation, TerminalError> {
        let cr = self.carriage_return();
        Ok(cr.merge(self.index_down(line_ids, store)?))
    }

    /// CSI S — Scroll Up (SU) inside the current region.
    pub(crate) fn scroll_up(
        &mut self,
        count: u16,
        line_ids: &mut LineIdAllocator,
        store: Option<&mut GraphemeStore>,
        break_after: HistoryBreakAfter,
    ) -> Result<Mutation, TerminalError> {
        let count = count.max(1);
        let region_height = self
            .scroll_bottom
            .saturating_sub(self.scroll_top)
            .saturating_add(1);
        let n = count.min(region_height);
        if n == 0 {
            return Ok(Mutation::none());
        }
        self.shift_region_rows_up(
            self.scroll_top,
            self.scroll_bottom,
            n,
            line_ids,
            store,
            break_after,
        )
    }

    /// CSI T — Scroll Down (SD) inside the current region.
    pub(crate) fn scroll_down(
        &mut self,
        count: u16,
        line_ids: &mut LineIdAllocator,
        store: Option<&mut GraphemeStore>,
    ) -> Result<Mutation, TerminalError> {
        let count = count.max(1);
        let region_height = self
            .scroll_bottom
            .saturating_sub(self.scroll_top)
            .saturating_add(1);
        let n = count.min(region_height);
        if n == 0 {
            return Ok(Mutation::none());
        }
        self.shift_region_rows_down(self.scroll_top, self.scroll_bottom, n, line_ids, store)
    }

    /// CSI L — Insert Lines at the cursor row within the scroll region.
    pub(crate) fn insert_lines(
        &mut self,
        count: u16,
        line_ids: &mut LineIdAllocator,
        store: &mut GraphemeStore,
    ) -> Result<Mutation, TerminalError> {
        if self.cursor.row < self.scroll_top || self.cursor.row > self.scroll_bottom {
            return Ok(Mutation::none());
        }
        let count = count.max(1);
        let available = self
            .scroll_bottom
            .saturating_sub(self.cursor.row)
            .saturating_add(1);
        let n = count.min(available);
        self.cursor.pending_wrap = false;
        self.shift_region_rows_down(
            self.cursor.row,
            self.scroll_bottom,
            n,
            line_ids,
            Some(store),
        )
    }

    /// CSI M — Delete Lines at the cursor row within the scroll region.
    pub(crate) fn delete_lines(
        &mut self,
        count: u16,
        line_ids: &mut LineIdAllocator,
        store: &mut GraphemeStore,
    ) -> Result<Mutation, TerminalError> {
        if self.cursor.row < self.scroll_top || self.cursor.row > self.scroll_bottom {
            return Ok(Mutation::none());
        }
        let count = count.max(1);
        let available = self
            .scroll_bottom
            .saturating_sub(self.cursor.row)
            .saturating_add(1);
        let n = count.min(available);
        self.cursor.pending_wrap = false;
        self.shift_region_rows_up(
            self.cursor.row,
            self.scroll_bottom,
            n,
            line_ids,
            Some(store),
            HistoryBreakAfter::HardBreak,
        )
    }

    /// CSI @ — Insert Characters at the cursor.
    pub(crate) fn insert_characters(&mut self, count: u16, store: &mut GraphemeStore) -> Mutation {
        let count = count.max(1);
        let row = self.cursor.row;
        let col = self.cursor.col;
        if col >= self.cols {
            return Mutation::none();
        }
        self.cursor.pending_wrap = false;
        let cols = usize::from(self.cols);
        let start = usize::from(row) * cols;
        let insert_at = start + usize::from(col);
        let n = usize::from(count).min(cols - usize::from(col));
        if n == 0 {
            return Mutation::none();
        }
        // Release cells that will fall off the right edge.
        for cell in &self.cells[start + cols - n..start + cols] {
            Self::release_cell(*cell, store);
        }
        self.cells
            .copy_within(insert_at..start + cols - n, insert_at + n);
        let blank = Cell::blank(self.pen.bg);
        self.cells[insert_at..insert_at + n].fill(blank);
        self.sanitize_row(row, store);
        Mutation::row(row)
    }

    /// CSI P — Delete Characters at the cursor.
    pub(crate) fn delete_characters(&mut self, count: u16, store: &mut GraphemeStore) -> Mutation {
        let count = count.max(1);
        let row = self.cursor.row;
        let col = self.cursor.col;
        if col >= self.cols {
            return Mutation::none();
        }
        self.cursor.pending_wrap = false;
        let cols = usize::from(self.cols);
        let start = usize::from(row) * cols;
        let delete_at = start + usize::from(col);
        let n = usize::from(count).min(cols - usize::from(col));
        if n == 0 {
            return Mutation::none();
        }
        for cell in &self.cells[delete_at..delete_at + n] {
            Self::release_cell(*cell, store);
        }
        self.cells
            .copy_within(delete_at + n..start + cols, delete_at);
        let blank = Cell::blank(self.pen.bg);
        self.cells[start + cols - n..start + cols].fill(blank);
        self.sanitize_row(row, store);
        Mutation::row(row)
    }

    /// CSI X — Erase Characters at the cursor (no shift).
    pub(crate) fn erase_characters(&mut self, count: u16, store: &mut GraphemeStore) -> Mutation {
        let count = count.max(1);
        let row = self.cursor.row;
        let col = self.cursor.col;
        if col >= self.cols {
            return Mutation::none();
        }
        self.cursor.pending_wrap = false;
        let end_col = col.saturating_add(count).min(self.cols);
        let mut mutation = Mutation::none();
        let mut c = col;
        while c < end_col {
            mutation = mutation.merge(self.clear_unit_at(c, row, store));
            c = c.saturating_add(1);
        }
        mutation
    }

    fn shift_region_rows_up(
        &mut self,
        top: u16,
        bottom: u16,
        count: u16,
        line_ids: &mut LineIdAllocator,
        mut store: Option<&mut GraphemeStore>,
        break_after: HistoryBreakAfter,
    ) -> Result<Mutation, TerminalError> {
        let cols = usize::from(self.cols);
        let top_i = usize::from(top);
        let bottom_i = usize::from(bottom);
        let n = usize::from(count);
        let region_rows = bottom_i - top_i + 1;
        if n == 0 || n > region_rows {
            return Ok(Mutation::none());
        }

        // Evict scrolled-away rows: full-screen primary-compatible retention only.
        if self.retain_history && self.region_is_full_screen() && top == 0 {
            for row in 0..n {
                let row_start = (top_i + row) * cols;
                let evicted_id = self.line_ids[top_i + row];
                let source_break = self.row_breaks[top_i + row].unwrap_or(break_after);
                if let Some(store) = store.as_deref() {
                    self.history.append_row(
                        evicted_id,
                        source_break,
                        &self.cells[row_start..row_start + cols],
                        store,
                    );
                }
                if let Some(store) = store.as_deref_mut() {
                    for cell in &self.cells[row_start..row_start + cols] {
                        Self::release_cell(*cell, store);
                    }
                }
            }
        } else if let Some(store) = store.as_mut() {
            for row in 0..n {
                let row_start = (top_i + row) * cols;
                for cell in &self.cells[row_start..row_start + cols] {
                    Self::release_cell(*cell, store);
                }
            }
        }

        let keep = region_rows - n;
        if keep > 0 {
            let src = (top_i + n) * cols;
            let dst = top_i * cols;
            let len = keep * cols;
            self.cells.copy_within(src..src + len, dst);
            self.line_ids
                .copy_within(top_i + n..top_i + n + keep, top_i);
            self.row_breaks
                .copy_within(top_i + n..top_i + n + keep, top_i);
        }

        let blank = Cell::blank(self.pen.bg);
        for row in 0..n {
            let row_index = bottom_i + 1 - n + row;
            let row_start = row_index * cols;
            // Bottom rows still hold original content after the upward move.
            if let Some(store) = store.as_mut() {
                for cell in &self.cells[row_start..row_start + cols] {
                    Self::release_cell(*cell, store);
                }
            }
            self.cells[row_start..row_start + cols].fill(blank);
            self.line_ids[row_index] = line_ids.allocate()?;
            self.row_breaks[row_index] = None;
        }
        Ok(Mutation::rows(top, bottom))
    }

    fn shift_region_rows_down(
        &mut self,
        top: u16,
        bottom: u16,
        count: u16,
        line_ids: &mut LineIdAllocator,
        mut store: Option<&mut GraphemeStore>,
    ) -> Result<Mutation, TerminalError> {
        let cols = usize::from(self.cols);
        let top_i = usize::from(top);
        let bottom_i = usize::from(bottom);
        let n = usize::from(count);
        let region_rows = bottom_i - top_i + 1;
        if n == 0 || n > region_rows {
            return Ok(Mutation::none());
        }

        if let Some(store) = store.as_mut() {
            for row in 0..n {
                let row_index = bottom_i + 1 - n + row;
                let row_start = row_index * cols;
                for cell in &self.cells[row_start..row_start + cols] {
                    Self::release_cell(*cell, store);
                }
            }
        }

        let keep = region_rows - n;
        if keep > 0 {
            let src = top_i * cols;
            let len = keep * cols;
            let dst = (top_i + n) * cols;
            self.cells.copy_within(src..src + len, dst);
            for row in (0..keep).rev() {
                self.line_ids[top_i + row + n] = self.line_ids[top_i + row];
                self.row_breaks[top_i + row + n] = self.row_breaks[top_i + row];
            }
        }

        let blank = Cell::blank(self.pen.bg);
        for row in 0..n {
            let row_index = top_i + row;
            let row_start = row_index * cols;
            // Top rows are leftovers of the memmove-down source; payloads now
            // live in the shifted rows, so blank without releasing.
            self.cells[row_start..row_start + cols].fill(blank);
            self.line_ids[row_index] = line_ids.allocate()?;
            self.row_breaks[row_index] = None;
        }
        Ok(Mutation::rows(top, bottom))
    }

    /// After in-row cell shifts, blank orphan lead/continuation halves.
    fn sanitize_row(&mut self, row: u16, store: &mut GraphemeStore) {
        let cols = usize::from(self.cols);
        let start = usize::from(row) * cols;
        let mut col = 0usize;
        while col < cols {
            let cell = self.cells[start + col];
            match cell.role {
                CellRole::Lead if cell.width >= 2 => {
                    if col + 1 >= cols || self.cells[start + col + 1].role != CellRole::Continuation
                    {
                        Self::release_cell(cell, store);
                        self.cells[start + col] = Cell::blank(self.pen.bg);
                    } else {
                        col += 1;
                    }
                }
                CellRole::Continuation
                    if col == 0 || self.cells[start + col - 1].role != CellRole::Lead =>
                {
                    self.cells[start + col] = Cell::blank(self.pen.bg);
                }
                _ => {}
            }
            col += 1;
        }
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
    row_breaks: Vec<Option<HistoryBreakAfter>>,
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
            row_breaks: Vec::new(),
            cursor: Cursor::default(),
            saved_cursor: None,
            unchanged: true,
        }
    }
}
