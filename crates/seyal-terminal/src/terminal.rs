use crate::{
    Cell, CursorState, Damage, LineId, ModeState, TerminalError,
    damage::{DamageTracker, Mutation},
    parser::{Actions, Parser},
    screen::Screen,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Diagnostics {
    pub deferred_sequences: u64,
    pub unknown_sequences: u64,
    pub malformed_sequences: u64,
}

pub struct TerminalState {
    parser: Parser,
    core: TerminalCore,
}

impl TerminalState {
    pub fn new(cols: u16, rows: u16) -> Result<Self, TerminalError> {
        Ok(Self {
            parser: Parser::new(),
            core: TerminalCore::new(cols, rows)?,
        })
    }

    pub fn feed(&mut self, bytes: &[u8]) {
        self.parser.feed(bytes, &mut self.core);
        self.core.damage.commit();
    }

    pub fn finish_input(&mut self) {
        self.parser.finish(&mut self.core);
        self.core.damage.commit();
    }

    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<(), TerminalError> {
        self.core.resize(cols, rows)
    }

    pub fn cols(&self) -> u16 {
        self.core.current().cols()
    }

    pub fn rows(&self) -> u16 {
        self.core.current().rows()
    }

    pub fn cursor(&self) -> CursorState {
        self.core.current().cursor(self.core.modes.cursor_visible)
    }

    pub fn modes(&self) -> ModeState {
        self.core.modes
    }

    pub fn diagnostics(&self) -> Diagnostics {
        self.core.diagnostics
    }

    pub fn cell(&self, col: u16, row: u16) -> Option<Cell> {
        self.core.current().cell(col, row)
    }

    pub fn line_id(&self, row: u16) -> Option<LineId> {
        self.core.current().line_id(row)
    }

    pub fn row_text(&self, row: u16) -> Option<String> {
        if row >= self.rows() {
            return None;
        }
        Some(
            (0..self.cols())
                .filter_map(|col| self.cell(col, row))
                .map(|cell| cell.character)
                .collect(),
        )
    }

    pub fn damage_generation(&self) -> u64 {
        self.core.damage.generation()
    }

    pub fn take_damage(&mut self) -> Option<Damage> {
        self.core.damage.take()
    }
}

struct TerminalCore {
    primary: Screen,
    alternate: Option<Screen>,
    modes: ModeState,
    next_screen_namespace: u32,
    damage: DamageTracker,
    diagnostics: Diagnostics,
}

impl TerminalCore {
    fn new(cols: u16, rows: u16) -> Result<Self, TerminalError> {
        let primary = Screen::new(cols, rows, 1)?;
        let mut damage = DamageTracker::default();
        damage.mark(Mutation::full(rows));
        damage.commit();
        Ok(Self {
            primary,
            alternate: None,
            modes: ModeState::default(),
            next_screen_namespace: 2,
            damage,
            diagnostics: Diagnostics::default(),
        })
    }

    fn current(&self) -> &Screen {
        if self.modes.alternate_screen {
            self.alternate.as_ref().unwrap_or(&self.primary)
        } else {
            &self.primary
        }
    }

    fn current_mut(&mut self) -> &mut Screen {
        if self.modes.alternate_screen {
            if let Some(screen) = &mut self.alternate {
                return screen;
            }
        }
        &mut self.primary
    }

    fn apply(&mut self, mutation: Mutation) {
        self.damage.mark(mutation);
    }

    fn resize(&mut self, cols: u16, rows: u16) -> Result<(), TerminalError> {
        if cols == 0 || rows == 0 {
            return Err(TerminalError::InvalidSize);
        }
        let primary = self.primary.resize(cols, rows)?;
        let alternate = if let Some(screen) = &mut self.alternate {
            screen.resize(cols, rows)?
        } else {
            Mutation::none()
        };
        self.apply(primary.merge(alternate).merge(Mutation::full(rows)));
        self.damage.commit();
        Ok(())
    }

    fn set_cursor_visible(&mut self, visible: bool) {
        if self.modes.cursor_visible == visible {
            return;
        }
        self.modes.cursor_visible = visible;
        let row = self.current().cursor(visible).row;
        self.apply(Mutation::row(row));
    }

    fn set_alternate_screen(&mut self, enabled: bool) {
        if enabled == self.modes.alternate_screen {
            return;
        }

        if enabled {
            let cols = self.primary.cols();
            let rows = self.primary.rows();
            let namespace = self.next_screen_namespace;
            self.next_screen_namespace = self.next_screen_namespace.saturating_add(1);
            match Screen::new(cols, rows, namespace) {
                Ok(screen) => {
                    self.alternate = Some(screen);
                    self.modes.alternate_screen = true;
                    self.apply(Mutation::full(rows));
                }
                Err(_) => self.record_malformed(),
            }
        } else {
            self.alternate = None;
            self.modes.alternate_screen = false;
            self.apply(Mutation::full(self.primary.rows()));
        }
    }

    fn record_deferred(&mut self) {
        self.diagnostics.deferred_sequences = self.diagnostics.deferred_sequences.saturating_add(1);
    }

    fn record_unknown(&mut self) {
        self.diagnostics.unknown_sequences = self.diagnostics.unknown_sequences.saturating_add(1);
    }

    fn record_malformed(&mut self) {
        self.diagnostics.malformed_sequences =
            self.diagnostics.malformed_sequences.saturating_add(1);
    }
}

impl Actions for TerminalCore {
    fn print(&mut self, character: char) {
        let mutation = self.current_mut().print(character);
        self.apply(mutation);
    }

    fn execute(&mut self, byte: u8) {
        let mutation = self.current_mut().execute(byte);
        self.apply(mutation);
    }

    fn csi(&mut self, params: &[u16], private: Option<u8>, ignored: bool, final_byte: u8) {
        if ignored {
            self.record_deferred();
            return;
        }

        if private.is_some() {
            if private == Some(b'?') && matches!(final_byte, b'h' | b'l') {
                let enabled = final_byte == b'h';
                for mode in params {
                    match *mode {
                        25 => self.set_cursor_visible(enabled),
                        1049 => self.set_alternate_screen(enabled),
                        _ => self.record_deferred(),
                    }
                }
            } else {
                self.record_deferred();
            }
            return;
        }

        let mutation = match final_byte {
            b'A' => self.current_mut().cursor_up(param_one(params, 0)),
            b'B' => self.current_mut().cursor_down(param_one(params, 0)),
            b'C' => self.current_mut().cursor_forward(param_one(params, 0)),
            b'D' => self.current_mut().cursor_back(param_one(params, 0)),
            b'H' | b'f' => self.current_mut().set_cursor(
                param_one(params, 0).saturating_sub(1),
                param_one(params, 1).saturating_sub(1),
            ),
            b'G' => self
                .current_mut()
                .set_col(param_one(params, 0).saturating_sub(1)),
            b'd' => self
                .current_mut()
                .set_row(param_one(params, 0).saturating_sub(1)),
            b'J' => self.current_mut().erase_display(param_zero(params, 0)),
            b'K' => self.current_mut().erase_line(param_zero(params, 0)),
            b's' => {
                self.current_mut().save_cursor();
                Mutation::none()
            }
            b'u' => self.current_mut().restore_cursor(),
            b'm' => {
                if self.current_mut().apply_sgr(params) {
                    self.record_deferred();
                }
                Mutation::none()
            }
            b'@' | b'P' | b'X' | b'L' | b'M' | b'S' | b'T' | b'r' | b'h' | b'l' => {
                self.record_deferred();
                Mutation::none()
            }
            _ => {
                self.record_unknown();
                Mutation::none()
            }
        };
        self.apply(mutation);
    }

    fn esc(&mut self, final_byte: u8, had_intermediate: bool) {
        if had_intermediate {
            self.record_deferred();
            return;
        }
        let mutation = match final_byte {
            b'7' => {
                self.current_mut().save_cursor();
                Mutation::none()
            }
            b'8' => self.current_mut().restore_cursor(),
            b'D' | b'E' | b'M' => {
                self.record_deferred();
                Mutation::none()
            }
            _ => {
                self.record_unknown();
                Mutation::none()
            }
        };
        self.apply(mutation);
    }

    fn deferred_string(&mut self) {
        self.record_deferred();
    }

    fn malformed(&mut self) {
        self.record_malformed();
    }
}

fn param_one(params: &[u16], index: usize) -> u16 {
    match params.get(index).copied().unwrap_or(0) {
        0 => 1,
        value => value,
    }
}

fn param_zero(params: &[u16], index: usize) -> u16 {
    params.get(index).copied().unwrap_or(0)
}
