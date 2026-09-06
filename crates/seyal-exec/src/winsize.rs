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
        if columns > seyal_terminal::MAX_TERMINAL_COLUMNS
            || rows > seyal_terminal::MAX_TERMINAL_ROWS
        {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_geometry_above_terminal_caps() {
        assert!(matches!(
            WindowSize::cells(seyal_terminal::MAX_TERMINAL_COLUMNS + 1, 24),
            Err(ExecError::InvalidWindowSize)
        ));
        assert!(matches!(
            WindowSize::cells(80, seyal_terminal::MAX_TERMINAL_ROWS + 1),
            Err(ExecError::InvalidWindowSize)
        ));
        assert!(WindowSize::cells(
            seyal_terminal::MAX_TERMINAL_COLUMNS,
            seyal_terminal::MAX_TERMINAL_ROWS
        )
        .is_ok());
    }
}
