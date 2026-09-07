#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModeState {
    pub alternate_screen: bool,
    pub cursor_visible: bool,
    /// DEC private mode 2027 — Unicode-core grapheme semantics when set.
    pub unicode_core: bool,
    /// DEC private mode 7 (DECAWM) — autowrap when set.
    pub wraparound: bool,
    /// DEC private mode 1 (DECCKM) — application cursor keys.
    pub application_cursor: bool,
    /// DEC private mode 66 (DECNKM) — application keypad.
    pub application_keypad: bool,
    /// Negotiated Kitty progressive keyboard flags, masked to 1|2.
    pub keyboard_flags: u8,
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
            application_cursor: false,
            application_keypad: false,
            keyboard_flags: 0,
        }
    }
}
