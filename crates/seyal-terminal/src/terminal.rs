use crate::{
    Cell, CursorState, Damage, LineId, ModeState, TerminalError,
    damage::{DamageTracker, Mutation},
    line::LineIdAllocator,
    parser::{Actions, Parser},
    screen::Screen,
};
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Diagnostics {
    pub deferred_sequences: u64,
    pub unknown_sequences: u64,
    pub malformed_sequences: u64,
}

/// Bounded shell-integration metadata emitted by the canonical VT parser.
/// Terminal cells and arbitrary OSC payloads are never exposed through this
/// interface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShellIntegrationEvent {
    CommandStarted {
        token: ShellIntegrationToken,
    },
    CommandFinished {
        token: ShellIntegrationToken,
        exit_status: i32,
    },
}

/// Runtime-issued nonce carried by the shell integration marker. A marker is
/// only meaningful when it matches a command currently pending in Runtime;
/// arbitrary OSC 133 traffic is ignored by the block timeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShellIntegrationToken([u8; 16]);

impl ShellIntegrationToken {
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(self) -> [u8; 16] {
        self.0
    }

    pub(crate) fn from_hex(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != 32 {
            return None;
        }
        let mut token = [0u8; 16];
        for (index, pair) in bytes.chunks_exact(2).enumerate() {
            token[index] = (hex(pair[0])? << 4) | hex(pair[1])?;
        }
        Some(Self(token))
    }

    pub fn write_hex(self, out: &mut String) {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        for byte in self.0 {
            out.push(HEX[(byte >> 4) as usize] as char);
            out.push(HEX[(byte & 0xf) as usize] as char);
        }
    }
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
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

    pub fn feed(&mut self, bytes: &[u8]) -> Result<(), TerminalError> {
        if let Some(error) = self.core.fault {
            return Err(error);
        }
        self.parser.feed(bytes, &mut self.core);
        self.core.damage.commit();
        self.core.fault.map_or(Ok(()), Err)
    }

    pub fn finish_input(&mut self) -> Result<(), TerminalError> {
        if let Some(error) = self.core.fault {
            return Err(error);
        }
        self.parser.finish(&mut self.core);
        self.core.damage.commit();
        self.core.fault.map_or(Ok(()), Err)
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

    /// Returns a bounded primary-screen history range. The returned rows are
    /// an explicit read-only projection; alternate-screen content is never
    /// treated as command output history.
    pub fn primary_history_range(
        &self,
        start: LineId,
        end: LineId,
        max_lines: usize,
    ) -> Vec<(LineId, Vec<Cell>)> {
        if self.core.modes.alternate_screen || max_lines == 0 || end < start {
            return Vec::new();
        }
        let mut lines = Vec::new();
        let mut id = start;
        while id <= end && lines.len() < max_lines {
            if let Some(cells) = self.core.primary.history_line(id) {
                lines.push((id, cells.to_vec()));
            }
            let Some(next) = id.0.checked_add(1) else {
                break;
            };
            id = LineId(next);
        }
        lines
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

    pub fn take_shell_integration_event(&mut self) -> Option<ShellIntegrationEvent> {
        self.core.shell_events.pop_front()
    }
}

struct TerminalCore {
    primary: Screen,
    alternate: Option<Screen>,
    line_ids: LineIdAllocator,
    modes: ModeState,
    damage: DamageTracker,
    diagnostics: Diagnostics,
    fault: Option<TerminalError>,
    shell_events: VecDeque<ShellIntegrationEvent>,
}

impl TerminalCore {
    fn new(cols: u16, rows: u16) -> Result<Self, TerminalError> {
        let mut line_ids = LineIdAllocator::new();
        let primary = Screen::new(cols, rows, &mut line_ids)?;
        let mut damage = DamageTracker::default();
        damage.mark(Mutation::full(rows));
        damage.commit();
        Ok(Self {
            primary,
            alternate: None,
            line_ids,
            modes: ModeState::default(),
            damage,
            diagnostics: Diagnostics::default(),
            fault: None,
            shell_events: VecDeque::with_capacity(16),
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
        if self.modes.alternate_screen
            && let Some(screen) = &mut self.alternate
        {
            return screen;
        }
        &mut self.primary
    }

    fn apply(&mut self, mutation: Mutation) {
        self.damage.mark(mutation);
    }

    fn resize(&mut self, cols: u16, rows: u16) -> Result<(), TerminalError> {
        if let Some(error) = self.fault {
            return Err(error);
        }
        if cols == 0 || rows == 0 {
            return Err(TerminalError::InvalidSize);
        }

        let mut required_ids = usize::from(rows.saturating_sub(self.primary.rows()));
        if let Some(screen) = &self.alternate {
            required_ids += usize::from(rows.saturating_sub(screen.rows()));
        }
        if !self.line_ids.can_allocate(required_ids) {
            return Err(TerminalError::LineIdentityExhausted);
        }

        let primary = self.primary.resize(cols, rows, &mut self.line_ids)?;
        let alternate = if let Some(screen) = &mut self.alternate {
            screen.resize(cols, rows, &mut self.line_ids)?
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

    fn set_alternate_screen(&mut self, enabled: bool) -> Result<(), TerminalError> {
        if enabled == self.modes.alternate_screen {
            return Ok(());
        }

        if enabled {
            let cols = self.primary.cols();
            let rows = self.primary.rows();
            let pen = self.primary.pen();
            let mut screen = Screen::new(cols, rows, &mut self.line_ids)?;
            screen.inherit_pen_for_clean_buffer(pen);
            self.alternate = Some(screen);
            self.modes.alternate_screen = true;
            self.apply(Mutation::full(rows));
        } else {
            self.alternate = None;
            self.modes.alternate_screen = false;
            self.apply(Mutation::full(self.primary.rows()));
        }
        Ok(())
    }

    fn print_current(&mut self, character: char) -> Result<Mutation, TerminalError> {
        if self.modes.alternate_screen
            && let Some(screen) = &mut self.alternate
        {
            return screen.print(character, &mut self.line_ids);
        }
        self.primary.print(character, &mut self.line_ids)
    }

    fn execute_current(&mut self, byte: u8) -> Result<Mutation, TerminalError> {
        if self.modes.alternate_screen
            && let Some(screen) = &mut self.alternate
        {
            return screen.execute(byte, &mut self.line_ids);
        }
        self.primary.execute(byte, &mut self.line_ids)
    }

    fn record_fault(&mut self, error: TerminalError) {
        if self.fault.is_none() {
            self.fault = Some(error);
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
        if self.fault.is_some() {
            return;
        }
        match self.print_current(character) {
            Ok(mutation) => self.apply(mutation),
            Err(error) => self.record_fault(error),
        }
    }

    fn execute(&mut self, byte: u8) {
        if self.fault.is_some() {
            return;
        }
        match self.execute_current(byte) {
            Ok(mutation) => self.apply(mutation),
            Err(error) => self.record_fault(error),
        }
    }

    fn csi(&mut self, params: &[u16], private: Option<u8>, ignored: bool, final_byte: u8) {
        if self.fault.is_some() {
            return;
        }
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
                        1049 => {
                            if let Err(error) = self.set_alternate_screen(enabled) {
                                self.record_fault(error);
                                break;
                            }
                        }
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
        if self.fault.is_some() {
            return;
        }
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

    fn osc(&mut self, bytes: &[u8], truncated: bool) {
        if self.fault.is_some() {
            return;
        }
        if truncated || self.modes.alternate_screen {
            self.record_deferred();
            return;
        }
        let event = match bytes.strip_prefix(b"133;") {
            Some(payload) => {
                let mut fields = payload.split(|byte| *byte == b';');
                match (fields.next(), fields.next(), fields.next()) {
                    (Some(b"C"), Some(token), None) => ShellIntegrationToken::from_hex(token)
                        .map(|token| ShellIntegrationEvent::CommandStarted { token }),
                    (Some(b"D"), Some(token), Some(status)) => {
                        ShellIntegrationToken::from_hex(token).and_then(|token| {
                            std::str::from_utf8(status)
                                .ok()
                                .and_then(|status| status.parse::<i32>().ok())
                                .map(|exit_status| ShellIntegrationEvent::CommandFinished {
                                    token,
                                    exit_status,
                                })
                        })
                    }
                    _ => None,
                }
            }
            _ => None,
        };
        let Some(event) = event else {
            self.record_deferred();
            return;
        };
        if self.shell_events.len() == self.shell_events.capacity() {
            self.record_deferred();
            return;
        }
        self.shell_events.push_back(event);
    }

    fn deferred_string(&mut self) {
        if self.fault.is_none() {
            self.record_deferred();
        }
    }

    fn malformed(&mut self) {
        if self.fault.is_none() {
            self.record_malformed();
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_identity_exhaustion_is_explicit_and_does_not_duplicate_scroll_id() {
        let mut terminal = TerminalState::new(2, 1).expect("valid terminal");
        terminal.core.line_ids = LineIdAllocator::with_next(Some(u64::MAX));

        terminal
            .feed(b"A\r\n")
            .expect("last available line id may be allocated once");
        let last = terminal.line_id(0).expect("visible line has id");
        assert_eq!(last, LineId(u64::MAX));

        assert_eq!(
            terminal.feed(b"\r\n"),
            Err(TerminalError::LineIdentityExhausted)
        );
        assert_eq!(terminal.line_id(0), Some(last));
        assert_eq!(
            terminal.feed(b"ignored after fault"),
            Err(TerminalError::LineIdentityExhausted)
        );
        assert_eq!(terminal.line_id(0), Some(last));
    }

    #[test]
    fn resize_preflights_line_identity_for_primary_and_alternate_atomically() {
        let mut terminal = TerminalState::new(2, 1).expect("valid terminal");
        terminal.core.line_ids = LineIdAllocator::with_next(Some(u64::MAX));
        terminal
            .feed(b"\x1b[?1049h")
            .expect("alternate consumes final available id");
        assert!(terminal.modes().alternate_screen);

        assert_eq!(
            terminal.resize(2, 2),
            Err(TerminalError::LineIdentityExhausted)
        );
        assert_eq!((terminal.cols(), terminal.rows()), (2, 1));
        terminal
            .feed(b"\x1b[?1049l")
            .expect("leaving alternate needs no new id");
        assert_eq!((terminal.cols(), terminal.rows()), (2, 1));
    }

    #[test]
    fn exposes_bounded_trusted_shell_events_without_exposing_osc_payload() {
        let mut terminal = TerminalState::new(80, 24).unwrap();
        terminal
            .feed(b"\x1b]133;C;00112233445566778899aabbccddeeff\x07\x1b]133;D;00112233445566778899aabbccddeeff;17\x1b\\")
            .unwrap();
        let token = ShellIntegrationToken::from_bytes([
            0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee,
            0xff,
        ]);

        assert_eq!(
            terminal.take_shell_integration_event(),
            Some(ShellIntegrationEvent::CommandStarted { token })
        );
        assert_eq!(
            terminal.take_shell_integration_event(),
            Some(ShellIntegrationEvent::CommandFinished {
                token,
                exit_status: 17,
            })
        );
        assert_eq!(terminal.take_shell_integration_event(), None);
    }

    #[test]
    fn unbound_or_malformed_markers_are_not_lifecycle_events() {
        let mut terminal = TerminalState::new(80, 24).unwrap();
        terminal
            .feed(b"\x1b]133;C\x07\x1b]133;C;short\x07\x1b]133;D;00112233445566778899aabbccddeeff;bad\x07")
            .unwrap();
        assert_eq!(terminal.take_shell_integration_event(), None);
    }

    #[test]
    fn primary_history_range_returns_scrolled_rows_by_line_id() {
        let mut terminal = TerminalState::new(4, 2).unwrap();
        terminal.feed(b"one\r\ntwo\r\nthree").unwrap();
        let first = terminal.line_id(0).unwrap();
        let last = terminal.line_id(1).unwrap();
        let rows = terminal.primary_history_range(LineId(1), last, 8);
        assert_eq!(rows.first().map(|(id, _)| *id), Some(LineId(1)));
        assert!(rows.iter().any(|(_, cells)| {
            cells
                .iter()
                .map(|cell| cell.character)
                .collect::<String>()
                .starts_with("one")
        }));
        assert!(terminal.primary_history_range(first, last, 0).is_empty());
    }
}
