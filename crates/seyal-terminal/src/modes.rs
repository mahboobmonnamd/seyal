/// Xterm mouse reporting level. 1000/1002/1003 are mutually exclusive.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MouseReporting {
    #[default]
    Off,
    /// DECSET 1000 — press and release.
    Button,
    /// DECSET 1002 — press, release, and motion while a button is down.
    ButtonDrag,
    /// DECSET 1003 — press, release, and all motion.
    Any,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModeState {
    pub alternate_screen: bool,
    pub cursor_visible: bool,
    /// DEC private mode 2027 — Unicode-core grapheme semantics when set.
    pub unicode_core: bool,
    /// DEC private mode 7 (DECAWM) — autowrap when set.
    pub wraparound: bool,
    /// DEC private mode 2004 — bracketed paste when set.
    pub bracketed_paste: bool,
    /// DEC private mode 1 (DECCKM) — application cursor keys.
    pub application_cursor: bool,
    /// DEC private mode 66 (DECNKM) — application keypad.
    pub application_keypad: bool,
    /// Negotiated Kitty progressive keyboard flags, masked to 1|2.
    pub keyboard_flags: u8,
    /// DECSET 1000/1002/1003 reporting level.
    pub mouse_reporting: MouseReporting,
    /// DECSET 1006 — SGR mouse encoding when set; otherwise X10.
    pub mouse_sgr: bool,
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
            // Bracketed paste starts reset; shells/TUIs enable it explicitly.
            bracketed_paste: false,
            application_cursor: false,
            application_keypad: false,
            keyboard_flags: 0,
            mouse_reporting: MouseReporting::Off,
            mouse_sgr: false,
        }
    }
}
