use crate::ExecError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowSize {
    columns: u16,
    rows: u16,
    pixel_width: u16,
    pixel_height: u16,
}

impl WindowSize {
    pub fn cells(columns: u16, rows: u16) -> Result<Self, ExecError> {
        Self::new(columns, rows, 0, 0)
    }

    pub fn new(
        columns: u16,
        rows: u16,
        pixel_width: u16,
        pixel_height: u16,
    ) -> Result<Self, ExecError> {
        if columns == 0 || rows == 0 {
            return Err(ExecError::InvalidWindowSize);
        }
        Ok(Self {
            columns,
            rows,
            pixel_width,
            pixel_height,
        })
    }

    pub fn columns(self) -> u16 {
        self.columns
    }

    pub fn rows(self) -> u16 {
        self.rows
    }

    pub fn pixel_width(self) -> u16 {
        self.pixel_width
    }

    pub fn pixel_height(self) -> u16 {
        self.pixel_height
    }
}

impl Default for WindowSize {
    fn default() -> Self {
        Self {
            columns: 80,
            rows: 24,
            pixel_width: 0,
            pixel_height: 0,
        }
    }
}
