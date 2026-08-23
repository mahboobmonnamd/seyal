#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CursorState {
    pub col: u16,
    pub row: u16,
    pub visible: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Cursor {
    pub(crate) col: u16,
    pub(crate) row: u16,
    pub(crate) pending_wrap: bool,
}

impl Cursor {
    pub(crate) fn clamp(&mut self, cols: u16, rows: u16) {
        self.col = self.col.min(cols.saturating_sub(1));
        self.row = self.row.min(rows.saturating_sub(1));
        self.pending_wrap = false;
    }
}
