use crate::{Color, Style};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub character: char,
    pub style: Style,
}

impl Default for Cell {
    fn default() -> Self {
        Self::blank(Color::Default)
    }
}

impl Cell {
    pub(crate) fn blank(background: Color) -> Self {
        Self {
            character: ' ',
            style: Style {
                bg: background,
                ..Style::default()
            },
        }
    }
}
