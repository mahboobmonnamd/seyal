#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModeState {
    pub alternate_screen: bool,
    pub cursor_visible: bool,
    /// DEC private mode 2027 — Unicode-core grapheme semantics when set.
    pub unicode_core: bool,
    /// DEC private mode 7 (DECAWM) — autowrap when set.
    pub wraparound: bool,
}

impl Default for ModeState {
    fn default() -> Self {
        Self {
            alternate_screen: false,
            cursor_visible: true,
            // SPEC-011 §3.1: mode 2027 defaults to set.
            unicode_core: true,
            // DECAWM defaults to set (wrap); fixtures explicitly reset it.
            wraparound: true,
        }
    }
}
