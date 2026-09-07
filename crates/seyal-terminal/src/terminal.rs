use crate::{
    active_grapheme::{
        append_payload, build_lead_cell, edge_decision_late_widen, edge_decision_new_unit,
        try_append_scalar, ActiveGrapheme, EdgeDecision,
    },
    damage::{DamageTracker, Mutation},
    grapheme_store::GraphemeStore,
    line::LineIdAllocator,
    parser::{Actions, Parser},
    presentation::{parse_osc_presentation, HostPresentationEvent, MAX_HOST_PRESENTATION_EVENTS},
    protocol_reply::{
        encode_decrqm_private, encode_dsr_cpr, encode_primary_da, ProtocolReply,
        MAX_PROTOCOL_REPLIES,
    },
    screen::{PreparedScreen, Screen},
    width::{grapheme_terminal_width, AmbiguousWidthPolicy},
    Cell, CursorState, Damage, HistoryAnchor, LineId, ModeState, ReflowRow, Style, TerminalError,
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
    pub grapheme_payload_overflow_count: u64,
    pub grapheme_store_capacity_fallback_count: u64,
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
        let mut diagnostics = self.core.diagnostics;
        diagnostics.grapheme_payload_overflow_count =
            self.core.grapheme_store.grapheme_payload_overflow_count;
        diagnostics.grapheme_store_capacity_fallback_count = self
            .core
            .grapheme_store
            .grapheme_store_capacity_fallback_count;
        diagnostics
    }

    pub fn ambiguous_width_policy(&self) -> AmbiguousWidthPolicy {
        self.core.ambiguous_width
    }

    pub fn set_ambiguous_width_policy(&mut self, policy: AmbiguousWidthPolicy) {
        if self.core.ambiguous_width != policy {
            self.core.ambiguous_width = policy;
            self.core.invalidate_active_grapheme();
        }
    }

    pub fn grapheme_store_live_bytes(&self) -> usize {
        self.core.grapheme_store.live_bytes()
    }

    pub fn pending_wrap(&self) -> bool {
        self.core.current().pending_wrap()
    }

    pub fn cell(&self, col: u16, row: u16) -> Option<Cell> {
        self.core.current().cell(col, row)
    }

    /// UTF-8 payload for a physical cell used by Candidate-D grapheme projection.
    ///
    /// Empty/Continuation return an empty slice. Lead returns store UTF-8 when
    /// present, otherwise the inline scalar encoding (overflow yields U+FFFD).
    pub fn lead_utf8(&self, col: u16, row: u16) -> Option<std::borrow::Cow<'_, [u8]>> {
        let cell = self.cell(col, row)?;
        match cell.role {
            crate::CellRole::Empty | crate::CellRole::Continuation => {
                Some(std::borrow::Cow::Borrowed(b""))
            }
            crate::CellRole::Lead => {
                if cell.overflow {
                    return Some(std::borrow::Cow::Borrowed("\u{FFFD}".as_bytes()));
                }
                if let Some(bytes) = self.core.grapheme_store.get(cell.store_id) {
                    Some(std::borrow::Cow::Borrowed(bytes))
                } else {
                    let mut buf = [0u8; 4];
                    let encoded = cell.character.encode_utf8(&mut buf);
                    Some(std::borrow::Cow::Owned(encoded.as_bytes().to_vec()))
                }
            }
        }
    }

    pub fn line_id(&self, row: u16) -> Option<LineId> {
        self.core.current().line_id(row)
    }

    /// Returns a bounded primary-screen history range. The returned rows are
    /// an explicit read-only projection of **primary** retained history plus
    /// primary visible rows. Alternate-screen cells are never included; the
    /// portable API still returns primary history while alternate screen is
    /// active so embedders (not VT) own Blocks/TUI presentation policy.
    ///
    /// Work is bounded by retained history plus visible rows, never by the
    /// numeric distance between `start` and `end` (LineIds may be sparse).
    pub fn primary_history_range(
        &self,
        start: LineId,
        end: LineId,
        max_lines: usize,
    ) -> Vec<(LineId, Vec<Cell>)> {
        if max_lines == 0 || end < start {
            return Vec::new();
        }
        let mut lines = Vec::new();
        for entry in self.core.primary.history_entries() {
            let id = entry.line_id;
            if id < start {
                continue;
            }
            if id > end {
                break;
            }
            lines.push((id, entry.cells()));
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

    /// Derives width-specific rows from canonical retained history. The
    /// projection is bounded by `max_rows` and does not rewrite source text.
    pub fn primary_history_reflow(&self, cols: u16, max_rows: usize) -> Vec<ReflowRow> {
        self.core.primary.history().reflow(cols, max_rows)
    }

    pub fn primary_history_resident_bytes(&self) -> usize {
        self.core.primary.history().resident_bytes()
    }

    pub fn primary_history_eviction_generation(&self) -> u64 {
        self.core.primary.history().eviction_generation()
    }

    pub fn primary_history_oldest_segment_age(&self) -> Option<u64> {
        self.core.primary.history().oldest_segment_age()
    }

    pub fn evict_oldest_primary_history_segment(&mut self) -> usize {
        self.core.primary.history_mut().evict_oldest_segment()
    }

    /// Resolves a retained source anchor to its canonical text unit. An
    /// absent result means the source was evicted or the offset is invalid.
    pub fn primary_history_unit(&self, anchor: HistoryAnchor) -> Option<(String, u8, Style)> {
        self.core
            .primary
            .history_entries()
            .find(|line| {
                line.line_id == anchor.line_id
                    && anchor.unit_offset >= line.start_offset
                    && usize::try_from(anchor.unit_offset - line.start_offset)
                        .ok()
                        .is_some_and(|offset| offset < line.units.len())
            })
            .and_then(|line| {
                let relative = anchor.unit_offset.checked_sub(line.start_offset)? as usize;
                line.units.get(relative)
            })
            .and_then(|unit| {
                String::from_utf8(unit.utf8.clone())
                    .ok()
                    .map(|text| (text, unit.width, unit.style))
            })
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

    /// Transfers one bounded, untrusted host-presentation event (OSC title/CWD/hyperlink).
    /// Embedders must treat payloads as display-only input, never as host authority.
    pub fn take_host_presentation_event(&mut self) -> Option<HostPresentationEvent> {
        self.core.presentation_events.pop_front()
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
    presentation_events: VecDeque<HostPresentationEvent>,
    protocol_replies: VecDeque<ProtocolReply>,
    grapheme_store: GraphemeStore,
    active_grapheme: Option<ActiveGrapheme>,
    ambiguous_width: AmbiguousWidthPolicy,
}

impl TerminalCore {
    fn new(cols: u16, rows: u16) -> Result<Self, TerminalError> {
        let mut line_ids = LineIdAllocator::new();
        let primary = Screen::new(cols, rows, &mut line_ids, true)?;
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
            presentation_events: VecDeque::with_capacity(MAX_HOST_PRESENTATION_EVENTS),
            protocol_replies: VecDeque::with_capacity(MAX_PROTOCOL_REPLIES),
            grapheme_store: GraphemeStore::default(),
            active_grapheme: None,
            ambiguous_width: AmbiguousWidthPolicy::default(),
        })
    }

    fn invalidate_active_grapheme(&mut self) {
        self.active_grapheme = None;
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

        let primary =
            self.primary
                .prepare_resize(cols, rows, &mut self.line_ids, &self.grapheme_store)?;
        let alternate = if let Some(screen) = &self.alternate {
            Some(screen.prepare_resize(cols, rows, &mut self.line_ids, &self.grapheme_store)?)
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
                7 => {
                    let status = if self.modes.wraparound { 1 } else { 2 };
                    if let Some(reply) = encode_decrqm_private(7, status) {
                        self.enqueue_protocol_reply(reply);
                    } else {
                        self.record_deferred();
                    }
                }
                25 => {
                    let status = if self.modes.cursor_visible { 1 } else { 2 };
                    if let Some(reply) = encode_decrqm_private(25, status) {
                        self.enqueue_protocol_reply(reply);
                    } else {
                        self.record_deferred();
                    }
                }
                1049 => {
                    let status = if self.modes.alternate_screen { 1 } else { 2 };
                    if let Some(reply) = encode_decrqm_private(1049, status) {
                        self.enqueue_protocol_reply(reply);
                    } else {
                        self.record_deferred();
                    }
                }
                2027 => {
                    let status = if self.modes.unicode_core { 1 } else { 2 };
                    if let Some(reply) = encode_decrqm_private(2027, status) {
                        self.enqueue_protocol_reply(reply);
                    } else {
                        self.record_deferred();
                    }
                }
                _ => self.record_deferred(),
            }
        }
    }

    fn reply_primary_da(&mut self) {
        if let Some(reply) = encode_primary_da() {
            self.enqueue_protocol_reply(reply);
        } else {
            self.record_deferred();
        }
    }

    fn enqueue_presentation(&mut self, event: HostPresentationEvent) {
        if self.presentation_events.len() == self.presentation_events.capacity() {
            self.record_deferred();
            return;
        }
        self.presentation_events.push_back(event);
    }

    fn editing_mutation<F>(&mut self, op: F) -> Mutation
    where
        F: FnOnce(
            &mut Screen,
            &mut LineIdAllocator,
            &mut GraphemeStore,
        ) -> Result<Mutation, TerminalError>,
    {
        let result = if self.modes.alternate_screen {
            if let Some(screen) = &mut self.alternate {
                op(screen, &mut self.line_ids, &mut self.grapheme_store)
            } else {
                op(
                    &mut self.primary,
                    &mut self.line_ids,
                    &mut self.grapheme_store,
                )
            }
        } else {
            op(
                &mut self.primary,
                &mut self.line_ids,
                &mut self.grapheme_store,
            )
        };
        match result {
            Ok(mutation) => mutation,
            Err(error) => {
                self.record_fault(error);
                Mutation::none()
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
        self.invalidate_active_grapheme();

        if enabled {
            let cols = self.primary.cols();
            let rows = self.primary.rows();
            let pen = self.primary.pen();
            let mut screen = Screen::new(cols, rows, &mut self.line_ids, false)?;
            screen.inherit_pen_for_clean_buffer(pen, &mut self.grapheme_store);
            self.alternate = Some(screen);
            self.modes.alternate_screen = true;
            self.apply(Mutation::full(rows));
        } else {
            if let Some(mut screen) = self.alternate.take() {
                screen.release_all_payloads(&mut self.grapheme_store);
            }
            self.modes.alternate_screen = false;
            self.apply(Mutation::full(self.primary.rows()));
        }
        Ok(())
    }

    fn print_current(&mut self, character: char) -> Result<Mutation, TerminalError> {
        self.print_scalar(character)
    }

    fn print_scalar(&mut self, character: char) -> Result<Mutation, TerminalError> {
        let unicode_core = self.modes.unicode_core;
        let wraparound = self.modes.wraparound;
        let ambiguous = self.ambiguous_width;
        let style = self.current().pen();

        // Append to active grapheme when eligible.
        if let Some(active) = self.active_grapheme.as_ref() {
            if try_append_scalar(active, character, unicode_core)
                || (!unicode_core
                    && grapheme_terminal_width(&character.to_string(), ambiguous) == 0)
            {
                return self.append_to_active(character, wraparound, ambiguous);
            }
            // Boundary: active unit stays committed; start a new one.
            self.active_grapheme = None;
        }

        // Legacy combining onto previous cell without active anchor.
        if !unicode_core {
            let width = grapheme_terminal_width(&character.to_string(), ambiguous);
            if width == 0 {
                return Ok(Mutation::none());
            }
        }

        let mut text = String::new();
        text.push(character);
        let width = if unicode_core {
            grapheme_terminal_width(&text, ambiguous)
        } else {
            grapheme_terminal_width(&text, ambiguous).max(1)
        };

        if width == 0 {
            // Isolated combining in Unicode-core with no active base: ignore.
            return Ok(Mutation::none());
        }

        let cursor = self.current().cursor(true);
        match edge_decision_new_unit(cursor.col, self.current().cols(), width, wraparound) {
            EdgeDecision::IgnoreUnit => {
                // SPEC-011 §8.4: ignore atomically; leave active unset.
                return Ok(Mutation::none());
            }
            EdgeDecision::RejectExtension | EdgeDecision::Place => {}
        }

        let lead = build_lead_cell(&text, width, style, false, &mut self.grapheme_store, None);
        let (mutation, soft_wrapped, lead_col, lead_row) = {
            let line_ids = &mut self.line_ids;
            let store = &mut self.grapheme_store;
            let screen = if self.modes.alternate_screen {
                self.alternate.as_mut().unwrap_or(&mut self.primary)
            } else {
                &mut self.primary
            };
            screen.place_new_unit(lead, wraparound, line_ids, store)?
        };
        let _ = soft_wrapped;

        self.active_grapheme = Some(ActiveGrapheme {
            col: lead_col,
            row: lead_row,
            utf8: text,
            width,
            style,
            store_id: self
                .current()
                .cell(lead_col, lead_row)
                .map(|c| c.store_id)
                .unwrap_or(crate::grapheme_store::INLINE_STORE_ID),
            overflow: false,
        });
        Ok(mutation)
    }

    fn append_to_active(
        &mut self,
        character: char,
        wraparound: bool,
        ambiguous: AmbiguousWidthPolicy,
    ) -> Result<Mutation, TerminalError> {
        let Some(mut active) = self.active_grapheme.take() else {
            return Ok(Mutation::none());
        };
        let previous_width = active.width;
        let result = append_payload(&mut active, character, &mut self.grapheme_store, ambiguous);

        if result.width_changed && result.width > previous_width {
            match edge_decision_late_widen(
                active.col,
                self.current().cols(),
                result.width,
                wraparound,
            ) {
                EdgeDecision::RejectExtension => {
                    // SPEC-011 §8.4: reject only the width-changing extension.
                    active.utf8.pop();
                    self.active_grapheme = Some(active);
                    return Ok(Mutation::none());
                }
                EdgeDecision::IgnoreUnit => {
                    active.utf8.pop();
                    self.active_grapheme = Some(active);
                    return Ok(Mutation::none());
                }
                EdgeDecision::Place => {
                    if active.col + 1 >= self.current().cols() && wraparound {
                        // Late widen that must soft-wrap: clear old, re-place on next row.
                        let style = active.style;
                        let text = active.utf8.clone();
                        let overflow = active.overflow;
                        let old_store = active.store_id;
                        let clear_mut = {
                            let store = &mut self.grapheme_store;
                            let screen = if self.modes.alternate_screen {
                                self.alternate.as_mut().unwrap_or(&mut self.primary)
                            } else {
                                &mut self.primary
                            };
                            screen.clear_unit_at(active.col, active.row, store)
                        };
                        // Soft-wrap lineage: mark previous row wrap by setting pending and LF.
                        {
                            let screen = if self.modes.alternate_screen {
                                self.alternate.as_mut().unwrap_or(&mut self.primary)
                            } else {
                                &mut self.primary
                            };
                            screen.set_pending_wrap(true);
                        }
                        let lead = build_lead_cell(
                            &text,
                            result.width,
                            style,
                            overflow,
                            &mut self.grapheme_store,
                            Some(old_store),
                        );
                        let (place_mut, _, lead_col, lead_row) = {
                            let line_ids = &mut self.line_ids;
                            let store = &mut self.grapheme_store;
                            let screen = if self.modes.alternate_screen {
                                self.alternate.as_mut().unwrap_or(&mut self.primary)
                            } else {
                                &mut self.primary
                            };
                            screen.place_new_unit(lead, wraparound, line_ids, store)?
                        };
                        active.col = lead_col;
                        active.row = lead_row;
                        active.width = result.width;
                        active.store_id = self
                            .current()
                            .cell(lead_col, lead_row)
                            .map(|c| c.store_id)
                            .unwrap_or(crate::grapheme_store::INLINE_STORE_ID);
                        self.active_grapheme = Some(active);
                        return Ok(clear_mut.merge(place_mut));
                    }
                }
            }
        }

        active.width = result.width;
        let release = if active.store_id != crate::grapheme_store::INLINE_STORE_ID {
            Some(active.store_id)
        } else {
            None
        };
        let lead = build_lead_cell(
            &active.utf8,
            active.width.max(1),
            active.style,
            active.overflow,
            &mut self.grapheme_store,
            release,
        );
        active.store_id = lead.store_id;
        active.overflow = lead.overflow;
        let mutation = {
            let store = &mut self.grapheme_store;
            let screen = if self.modes.alternate_screen {
                self.alternate.as_mut().unwrap_or(&mut self.primary)
            } else {
                &mut self.primary
            };
            screen.replace_active_lead(active.col, active.row, lead, previous_width, store)
        };
        // Adjust cursor after late widen occupying an extra cell.
        if result.width > previous_width && result.width >= 2 {
            let screen = if self.modes.alternate_screen {
                self.alternate.as_mut().unwrap_or(&mut self.primary)
            } else {
                &mut self.primary
            };
            let cursor = screen.cursor(true);
            if !screen.pending_wrap() && cursor.col == active.col + 1 {
                // Was width-1 with cursor after lead; now need cursor after continuation.
                let _ = screen; // cursor advance handled below
            }
        }
        if result.width > previous_width {
            let cols = self.current().cols();
            let screen = if self.modes.alternate_screen {
                self.alternate.as_mut().unwrap_or(&mut self.primary)
            } else {
                &mut self.primary
            };
            let next = active.col.saturating_add(u16::from(result.width));
            if next >= cols {
                // Move cursor to last col with pending wrap.
                let _ = screen.set_col(cols.saturating_sub(1));
                screen.set_pending_wrap(true);
            } else {
                let _ = screen.set_col(next);
            }
        }
        self.active_grapheme = Some(active);
        Ok(mutation)
    }

    fn execute_current(&mut self, byte: u8) -> Result<Mutation, TerminalError> {
        if let 0x08..=0x0d = byte {
            self.invalidate_active_grapheme();
        }
        if self.modes.alternate_screen
            && let Some(screen) = &mut self.alternate
        {
            return screen.execute(byte, &mut self.line_ids, &mut self.grapheme_store);
        }
        self.primary
            .execute(byte, &mut self.line_ids, &mut self.grapheme_store)
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
                        7 => {
                            if self.modes.wraparound != enabled {
                                self.modes.wraparound = enabled;
                                self.invalidate_active_grapheme();
                            }
                        }
                        25 => self.set_cursor_visible(enabled),
                        2027 => {
                            if self.modes.unicode_core != enabled {
                                self.modes.unicode_core = enabled;
                                self.invalidate_active_grapheme();
                            }
                        }
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
            b'A' => {
                self.invalidate_active_grapheme();
                self.current_mut().cursor_up(param_one(params, 0))
            }
            b'B' => {
                self.invalidate_active_grapheme();
                self.current_mut().cursor_down(param_one(params, 0))
            }
            b'C' => {
                self.invalidate_active_grapheme();
                self.current_mut().cursor_forward(param_one(params, 0))
            }
            b'D' => {
                self.invalidate_active_grapheme();
                self.current_mut().cursor_back(param_one(params, 0))
            }
            b'H' | b'f' => {
                self.invalidate_active_grapheme();
                self.current_mut().set_cursor(
                    param_one(params, 0).saturating_sub(1),
                    param_one(params, 1).saturating_sub(1),
                )
            }
            b'G' => {
                self.invalidate_active_grapheme();
                self.current_mut()
                    .set_col(param_one(params, 0).saturating_sub(1))
            }
            b'd' => {
                self.invalidate_active_grapheme();
                self.current_mut()
                    .set_row(param_one(params, 0).saturating_sub(1))
            }
            b'J' => {
                self.invalidate_active_grapheme();
                let mode = param_zero(params, 0);
                let store = &mut self.grapheme_store;
                if self.modes.alternate_screen {
                    if let Some(screen) = &mut self.alternate {
                        screen.erase_display(mode, store)
                    } else {
                        self.primary.erase_display(mode, store)
                    }
                } else {
                    self.primary.erase_display(mode, store)
                }
            }
            b'K' => {
                self.invalidate_active_grapheme();
                let mode = param_zero(params, 0);
                let store = &mut self.grapheme_store;
                if self.modes.alternate_screen {
                    if let Some(screen) = &mut self.alternate {
                        screen.erase_line(mode, store)
                    } else {
                        self.primary.erase_line(mode, store)
                    }
                } else {
                    self.primary.erase_line(mode, store)
                }
            }
            b's' => {
                self.current_mut().save_cursor();
                Mutation::none()
            }
            b'u' => {
                self.invalidate_active_grapheme();
                self.current_mut().restore_cursor()
            }
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
            b'c' => {
                match param_zero(params, 0) {
                    0 => self.reply_primary_da(),
                    _ => self.record_unknown(),
                }
                Mutation::none()
            }
            b'r' => {
                self.invalidate_active_grapheme();
                let top = param_zero(params, 0);
                let bottom = param_zero(params, 1);
                self.current_mut().set_scroll_region(top, bottom)
            }
            b'@' => {
                self.invalidate_active_grapheme();
                let count = param_one(params, 0);
                self.editing_mutation(|screen, _, store| Ok(screen.insert_characters(count, store)))
            }
            b'P' => {
                self.invalidate_active_grapheme();
                let count = param_one(params, 0);
                self.editing_mutation(|screen, _, store| Ok(screen.delete_characters(count, store)))
            }
            b'X' => {
                self.invalidate_active_grapheme();
                let count = param_one(params, 0);
                self.editing_mutation(|screen, _, store| Ok(screen.erase_characters(count, store)))
            }
            b'L' => {
                self.invalidate_active_grapheme();
                let count = param_one(params, 0);
                self.editing_mutation(|screen, line_ids, store| {
                    screen.insert_lines(count, line_ids, store)
                })
            }
            b'M' => {
                self.invalidate_active_grapheme();
                let count = param_one(params, 0);
                self.editing_mutation(|screen, line_ids, store| {
                    screen.delete_lines(count, line_ids, store)
                })
            }
            b'S' => {
                self.invalidate_active_grapheme();
                let count = param_one(params, 0);
                self.editing_mutation(|screen, line_ids, store| {
                    screen.scroll_up(
                        count,
                        line_ids,
                        Some(store),
                        crate::HistoryBreakAfter::HardBreak,
                    )
                })
            }
            b'T' => {
                self.invalidate_active_grapheme();
                let count = param_one(params, 0);
                self.editing_mutation(|screen, line_ids, store| {
                    screen.scroll_down(count, line_ids, Some(store))
                })
            }
            b'h' | b'l' => {
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
            b'D' => {
                self.invalidate_active_grapheme();
                self.editing_mutation(|screen, line_ids, store| screen.index_down(line_ids, store))
            }
            b'M' => {
                self.invalidate_active_grapheme();
                self.editing_mutation(|screen, line_ids, store| {
                    screen.reverse_index(line_ids, store)
                })
            }
            b'E' => {
                self.invalidate_active_grapheme();
                self.editing_mutation(|screen, line_ids, store| screen.next_line(line_ids, store))
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
        if truncated {
            self.record_deferred();
            return;
        }
        if let Some(event) = parse_osc_presentation(bytes) {
            self.enqueue_presentation(event);
            return;
        }
        if self.modes.alternate_screen {
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
    fn primary_history_range_remains_available_during_alternate_screen() {
        let mut terminal = TerminalState::new(4, 2).unwrap();
        terminal.feed(b"one\r\ntwo\r\nthree").unwrap();
        let last = terminal.line_id(1).unwrap();
        let before = terminal.primary_history_range(LineId(1), last, 8);
        assert!(!before.is_empty());
        terminal.feed(b"\x1b[?1049h").unwrap();
        assert!(terminal.modes().alternate_screen);
        let during = terminal.primary_history_range(LineId(1), last, 8);
        assert_eq!(
            during.len(),
            before.len(),
            "primary history must remain readable while alternate screen is active"
        );
        assert_eq!(
            during
                .iter()
                .map(|(id, cells)| (*id, cells.iter().map(|c| c.character).collect::<String>()))
                .collect::<Vec<_>>(),
            before
                .iter()
                .map(|(id, cells)| (*id, cells.iter().map(|c| c.character).collect::<String>()))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn retained_history_records_softwrap_and_hardbreak_lineage() {
        let mut terminal = TerminalState::new(2, 1).unwrap();
        terminal.feed(b"ab").unwrap();
        terminal.feed(b"c\n").unwrap();
        let history: Vec<_> = terminal
            .core
            .primary
            .history_entries()
            .map(|entry| {
                (
                    entry.line_id,
                    entry.break_after,
                    entry
                        .cells()
                        .iter()
                        .map(|cell| cell.character)
                        .collect::<String>(),
                )
            })
            .collect();

        assert!(
            history.iter().any(|(_, break_after, text)| {
                matches!(break_after, crate::HistoryBreakAfter::SoftWrap) && text == "ab"
            }),
            "soft-wrapped overflow row should be retained with SoftWrap lineage"
        );
        assert!(
            history.iter().any(|(_, break_after, text)| {
                matches!(break_after, crate::HistoryBreakAfter::HardBreak) && text == "c "
            }),
            "explicit line feed should be retained with HardBreak lineage"
        );
    }

    #[test]
    fn primary_resize_reflows_active_soft_wrapped_source() {
        let mut terminal = TerminalState::new(4, 2).unwrap();
        terminal.feed(b"abcdef").unwrap();
        terminal.resize(8, 2).unwrap();
        assert_eq!(terminal.row_text(0).as_deref(), Some("abcdef  "));
        terminal.resize(3, 2).unwrap();
        let text = format!(
            "{}{}",
            terminal.row_text(0).unwrap(),
            terminal.row_text(1).unwrap()
        );
        assert!(
            text.starts_with("abc"),
            "rows={:?} reflow text={text:?}",
            (terminal.row_text(0), terminal.row_text(1))
        );
        assert!(text.contains("def"), "reflow text={text:?}");
    }

    #[test]
    fn alternate_screen_never_adds_primary_history() {
        let mut terminal = TerminalState::new(4, 1).unwrap();
        terminal.feed(b"primary\r\n").unwrap();
        let before = terminal.primary_history_resident_bytes();
        terminal
            .feed(b"\x1b[?1049halternate\r\nalternate\r\n\x1b[?1049l")
            .unwrap();
        assert_eq!(terminal.primary_history_resident_bytes(), before);
        assert!(terminal.primary_history_eviction_generation() == 0);
    }

    #[test]
    fn retained_unit_preserves_canonical_multiscalar_payload_and_anchor() {
        let mut terminal = TerminalState::new(2, 1).unwrap();
        terminal.feed("界\u{301}\r\n".as_bytes()).unwrap();
        let (line_id, _) = terminal
            .primary_history_range(LineId(1), LineId(u64::MAX), 1)
            .into_iter()
            .next()
            .expect("wide source row is retained");
        let unit = terminal
            .primary_history_unit(HistoryAnchor {
                line_id,
                unit_offset: 0,
            })
            .expect("canonical source unit is addressable");
        assert_eq!(unit.0, "界\u{301}");
        assert_eq!(unit.1, 2);
    }

    #[test]
    fn history_reflow_is_invariant_under_input_chunking() {
        let input = "alpha界\u{301}xyz\r\nsecond-line\r\nthird";
        let mut one_shot = TerminalState::new(6, 1).unwrap();
        one_shot.feed(input.as_bytes()).unwrap();
        let mut bytewise = TerminalState::new(6, 1).unwrap();
        for byte in input.as_bytes() {
            bytewise.feed(std::slice::from_ref(byte)).unwrap();
        }
        assert_eq!(
            one_shot.primary_history_reflow(5, 64),
            bytewise.primary_history_reflow(5, 64)
        );
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

        // Mode 2027 defaults to set and is queryable (SPEC-011 §3).
        terminal.feed(b"\x1b[?2027$p").unwrap();
        assert_eq!(
            terminal.take_protocol_reply().unwrap().as_bytes(),
            b"\x1b[?2027;1$y"
        );

        let deferred_before = terminal.diagnostics().deferred_sequences;
        terminal.feed(b"\x1b[0n\x1b[?999$p").unwrap();
        assert!(terminal.take_protocol_reply().is_none());
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
