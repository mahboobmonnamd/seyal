use crate::{
    damage::{DamageTracker, Mutation},
    line::LineIdAllocator,
    parser::{Actions, Parser},
    protocol_reply::{encode_decrqm_private, encode_dsr_cpr, ProtocolReply, MAX_PROTOCOL_REPLIES},
    screen::{PreparedScreen, Screen},
    Cell, CursorState, Damage, LineId, ModeState, TerminalError,
};
use std::collections::VecDeque;

/// Soft ceiling aligned with Candidate-D display maxima. Larger geometries are
/// rejected cheaply so embedders cannot force multi-gigabyte grid allocations.
pub const MAX_TERMINAL_COLUMNS: u16 = 512;
pub const MAX_TERMINAL_ROWS: u16 = 256;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Diagnostics {
    pub deferred_sequences: u64,
    pub unknown_sequences: u64,
    pub malformed_sequences: u64,
}

/// Bounded shell-integration metadata emitted by the canonical VT parser.
/// Terminal cells and arbitrary OSC payloads are never exposed through this
/// interface. Correlation of tokens to workspace/Block lifecycle is owned by
/// Runtime/application integration, not by this crate.
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
/// only meaningful when an external integrator correlates it with pending
/// work; arbitrary OSC 133 traffic remains bounded and is otherwise ignored.
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
        let (pairs, remainder) = bytes.as_chunks::<2>();
        if !remainder.is_empty() {
            return None;
        }
        for (index, pair) in pairs.iter().enumerate() {
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

    /// Convenience prepare+commit for VT-only consumers. Prefer
    /// [`prepare_resize`] / [`commit_resize`] when coordinating with a PTY.
    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<(), TerminalError> {
        let prepared = self.prepare_resize(cols, rows)?;
        self.commit_resize(prepared);
        Ok(())
    }

    /// Fallible canonical resize preparation. Does not mutate live geometry or
    /// damage. Must be completed with [`commit_resize`] or dropped.
    pub fn prepare_resize(
        &mut self,
        cols: u16,
        rows: u16,
    ) -> Result<PreparedResize, TerminalError> {
        self.core.prepare_resize(cols, rows)
    }

    /// Infallible commit of a prepared resize. Damage/projection become
    /// observable only after this returns.
    pub fn commit_resize(&mut self, prepared: PreparedResize) {
        self.core.commit_resize(prepared);
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
    ///
    /// Work is bounded by retained history plus visible rows, never by the
    /// numeric distance between `start` and `end` (LineIds may be sparse).
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
        for (id, cells) in self.core.primary.history_entries() {
            if id < start {
                continue;
            }
            if id > end {
                break;
            }
            lines.push((id, cells.to_vec()));
            if lines.len() >= max_lines {
                return lines;
            }
        }
        for row in 0..self.core.primary.rows() {
            let Some(id) = self.core.primary.line_id(row) else {
                continue;
            };
            if id < start || id > end {
                continue;
            }
            if lines.iter().any(|(existing, _)| *existing == id) {
                continue;
            }
            let Some(cells) = self.core.primary.cell_row(row) else {
                continue;
            };
            lines.push((id, cells.to_vec()));
            if lines.len() >= max_lines {
                break;
            }
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

    /// Transfers one bounded terminal-generated protocol reply. Transport
    /// layers write these opaque bytes to the child PTY without interpreting
    /// query semantics.
    pub fn take_protocol_reply(&mut self) -> Option<ProtocolReply> {
        self.core.protocol_replies.pop_front()
    }
}

/// Opaque prepared resize held until [`TerminalState::commit_resize`].
pub struct PreparedResize {
    rows: u16,
    primary: PreparedScreen,
    alternate: Option<PreparedScreen>,
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
    protocol_replies: VecDeque<ProtocolReply>,
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
            protocol_replies: VecDeque::with_capacity(MAX_PROTOCOL_REPLIES),
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

    fn prepare_resize(&mut self, cols: u16, rows: u16) -> Result<PreparedResize, TerminalError> {
        if let Some(error) = self.fault {
            return Err(error);
        }
        if cols == 0 || rows == 0 {
            return Err(TerminalError::InvalidSize);
        }
        if cols > MAX_TERMINAL_COLUMNS || rows > MAX_TERMINAL_ROWS {
            return Err(TerminalError::InvalidSize);
        }

        #[cfg(feature = "test-fault-injection")]
        if crate::test_fault::take(crate::test_fault::FaultPoint::ResizePrepare) {
            return Err(TerminalError::LineIdentityExhausted);
        }

        let mut required_ids = usize::from(rows.saturating_sub(self.primary.rows()));
        if let Some(screen) = &self.alternate {
            required_ids += usize::from(rows.saturating_sub(screen.rows()));
        }
        if !self.line_ids.can_allocate(required_ids) {
            return Err(TerminalError::LineIdentityExhausted);
        }

        let primary = self
            .primary
            .prepare_resize(cols, rows, &mut self.line_ids)?;
        let alternate = if let Some(screen) = &self.alternate {
            Some(screen.prepare_resize(cols, rows, &mut self.line_ids)?)
        } else {
            None
        };
        Ok(PreparedResize {
            rows,
            primary,
            alternate,
        })
    }

    fn commit_resize(&mut self, prepared: PreparedResize) {
        let primary = self.primary.commit_prepared(prepared.primary);
        let alternate = if let Some(prepared_alt) = prepared.alternate {
            if let Some(screen) = &mut self.alternate {
                screen.commit_prepared(prepared_alt)
            } else {
                Mutation::none()
            }
        } else {
            Mutation::none()
        };
        self.apply(
            primary
                .merge(alternate)
                .merge(Mutation::full(prepared.rows)),
        );
        self.damage.commit();
    }

    fn enqueue_protocol_reply(&mut self, reply: ProtocolReply) {
        if self.protocol_replies.len() == self.protocol_replies.capacity() {
            self.record_deferred();
            return;
        }
        self.protocol_replies.push_back(reply);
    }

    fn reply_dsr_cpr(&mut self) {
        let cursor = self.current().cursor(self.modes.cursor_visible);
        if let Some(reply) = encode_dsr_cpr(cursor.row, cursor.col) {
            self.enqueue_protocol_reply(reply);
        } else {
            self.record_deferred();
        }
    }

    fn reply_decrqm(&mut self, params: &[u16]) {
        for mode in params {
            match *mode {
                25 => {
                    let status = if self.modes.cursor_visible { 1 } else { 2 };
                    if let Some(reply) = encode_decrqm_private(25, status) {
                        self.enqueue_protocol_reply(reply);
                    } else {
                        self.record_deferred();
                    }
                }
                _ => self.record_deferred(),
            }
        }
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
            // DECRQM uses intermediate `$` which the ECMA-48 parser marks as
            // ignored; handle the known private-mode query without advertising
            // unsupported modes.
            if private == Some(b'?') && final_byte == b'p' {
                self.reply_decrqm(params);
            } else {
                self.record_deferred();
            }
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
            b'n' => {
                match param_zero(params, 0) {
                    6 => self.reply_dsr_cpr(),
                    _ => self.record_unknown(),
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

    #[test]
    fn prepare_resize_leaves_geometry_and_damage_unchanged_until_commit() {
        let mut terminal = TerminalState::new(4, 2).unwrap();
        let _ = terminal.take_damage();
        let generation = terminal.damage_generation();
        let prepared = terminal.prepare_resize(8, 4).expect("prepare");
        assert_eq!((terminal.cols(), terminal.rows()), (4, 2));
        assert_eq!(terminal.damage_generation(), generation);
        assert!(terminal.take_damage().is_none());

        terminal.commit_resize(prepared);
        assert_eq!((terminal.cols(), terminal.rows()), (8, 4));
        let damage = terminal.take_damage().expect("damage after commit");
        assert!(damage.full);
        assert!(terminal.damage_generation() > generation);
    }

    #[test]
    fn dsr_cpr_emits_ordered_replies_with_chunk_equivalence() {
        let mut one_shot = TerminalState::new(80, 24).unwrap();
        one_shot.feed(b"\x1b[10;20H\x1b[6n").unwrap();
        let expected = one_shot.take_protocol_reply().expect("cpr reply");
        assert_eq!(expected.as_bytes(), b"\x1b[10;20R");
        assert!(one_shot.take_protocol_reply().is_none());

        let mut chunked = TerminalState::new(80, 24).unwrap();
        for byte in b"\x1b[10;20H\x1b[6n" {
            chunked.feed(std::slice::from_ref(byte)).unwrap();
        }
        assert_eq!(
            chunked.take_protocol_reply().map(|r| r.as_bytes().to_vec()),
            Some(expected.as_bytes().to_vec())
        );
    }

    #[test]
    fn multiple_queries_preserve_reply_order_and_bound() {
        let mut terminal = TerminalState::new(80, 24).unwrap();
        terminal.feed(b"\x1b[1;1H\x1b[6n\x1b[2;3H\x1b[6n").unwrap();
        assert_eq!(
            terminal.take_protocol_reply().unwrap().as_bytes(),
            b"\x1b[1;1R"
        );
        assert_eq!(
            terminal.take_protocol_reply().unwrap().as_bytes(),
            b"\x1b[2;3R"
        );

        let mut flood = TerminalState::new(80, 24).unwrap();
        let before = flood.diagnostics().deferred_sequences;
        for _ in 0..(MAX_PROTOCOL_REPLIES + 4) {
            flood.feed(b"\x1b[6n").unwrap();
        }
        let mut drained = 0usize;
        while flood.take_protocol_reply().is_some() {
            drained += 1;
        }
        assert_eq!(drained, MAX_PROTOCOL_REPLIES);
        assert!(flood.diagnostics().deferred_sequences > before);
    }

    #[test]
    fn decrqm_mode_25_and_unknown_queries_are_safe() {
        let mut terminal = TerminalState::new(80, 24).unwrap();
        terminal.feed(b"\x1b[?25l\x1b[?25$p").unwrap();
        assert_eq!(
            terminal.take_protocol_reply().unwrap().as_bytes(),
            b"\x1b[?25;2$y"
        );

        let unknown_before = terminal.diagnostics().unknown_sequences;
        let deferred_before = terminal.diagnostics().deferred_sequences;
        terminal.feed(b"\x1b[0n\x1b[?2027$p\x1b[?999$p").unwrap();
        assert!(terminal.take_protocol_reply().is_none());
        assert!(terminal.diagnostics().unknown_sequences > unknown_before);
        assert!(terminal.diagnostics().deferred_sequences > deferred_before);
    }

    #[test]
    fn replies_enqueued_before_feed_fault_remain_takeable() {
        let mut terminal = TerminalState::new(2, 1).expect("valid terminal");
        terminal.core.line_ids = LineIdAllocator::with_next(Some(u64::MAX));
        terminal
            .feed(b"A\r\n")
            .expect("consume final available line id");
        assert_eq!(
            terminal.feed(b"\x1b[6n\r\n"),
            Err(TerminalError::LineIdentityExhausted)
        );
        assert_eq!(
            terminal.take_protocol_reply().unwrap().as_bytes(),
            b"\x1b[1;1R"
        );
        assert!(terminal.take_protocol_reply().is_none());
    }

    #[test]
    fn huge_sparse_history_span_is_bounded_by_retained_storage() {
        use std::time::{Duration, Instant};

        let mut terminal = TerminalState::new(4, 2).unwrap();
        terminal.feed(b"one\r\ntwo\r\nthree\r\nfour").unwrap();
        // Alternate screen burns LineIds, creating gaps in primary identity space.
        terminal.feed(b"\x1b[?1049h\x1b[?1049l").unwrap();
        terminal.feed(b"five\r\nsix").unwrap();

        let started = Instant::now();
        let rows = terminal.primary_history_range(LineId(1), LineId(u64::MAX), 512);
        assert!(
            started.elapsed() < Duration::from_millis(100),
            "history lookup must not scale with numeric LineId distance"
        );
        assert!(rows.len() <= 512);
        assert!(!rows.is_empty());

        let absent = terminal.primary_history_range(LineId(u64::MAX - 10), LineId(u64::MAX), 8);
        assert!(absent.is_empty());
    }

    #[test]
    fn oversized_geometry_is_rejected_without_mutating_state() {
        assert!(matches!(
            TerminalState::new(MAX_TERMINAL_COLUMNS, MAX_TERMINAL_ROWS + 1),
            Err(TerminalError::InvalidSize)
        ));
        assert!(matches!(
            TerminalState::new(MAX_TERMINAL_COLUMNS + 1, MAX_TERMINAL_ROWS),
            Err(TerminalError::InvalidSize)
        ));
        let mut terminal = TerminalState::new(80, 24).unwrap();
        let generation = terminal.damage_generation();
        assert!(matches!(
            terminal.prepare_resize(u16::MAX, u16::MAX),
            Err(TerminalError::InvalidSize)
        ));
        assert_eq!((terminal.cols(), terminal.rows()), (80, 24));
        assert_eq!(terminal.damage_generation(), generation);
        terminal
            .prepare_resize(MAX_TERMINAL_COLUMNS, MAX_TERMINAL_ROWS)
            .expect("max accepted geometry prepares");
    }
}
