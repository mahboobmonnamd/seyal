#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModeState {
    pub alternate_screen: bool,
    pub cursor_visible: bool,
}

impl Default for ModeState {
    fn default() -> Self {
        Self {
            alternate_screen: false,
            cursor_visible: true,
        }
    }
}
