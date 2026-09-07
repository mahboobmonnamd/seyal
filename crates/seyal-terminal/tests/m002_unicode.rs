//! M002.2 (#816) Unicode grapheme/width TerminalState fixtures.

use seyal_terminal::{CellRole, TerminalState, UNICODE_SEMANTIC_VERSION};

fn feed_hex(terminal: &mut TerminalState, hex: &str) {
    let bytes = parse_hex(hex);
    terminal.feed(&bytes).unwrap();
}

fn parse_hex(hex: &str) -> Vec<u8> {
    hex.split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).expect("hex byte"))
        .collect()
}

fn row_text(terminal: &TerminalState, row: u16) -> String {
    (0..terminal.cols())
        .map(|col| {
            let cell = terminal.cell(col, row).unwrap();
            if cell.role == CellRole::Continuation {
                '\0'
            } else {
                cell.character
            }
        })
        .filter(|c| *c != '\0')
        .collect()
}

#[test]
fn pinned_unicode_version() {
    assert_eq!(UNICODE_SEMANTIC_VERSION, "17.0.0");
}

#[test]
fn ascii_and_combining_and_chunk_equivalence() {
    let mut once = TerminalState::new(20, 4).unwrap();
    once.feed("e\u{0301}X".as_bytes()).unwrap();
    assert_eq!(once.cell(0, 0).unwrap().character, 'e');
    assert_eq!(once.cell(1, 0).unwrap().character, 'X');

    let bytes = "e\u{0301}X".as_bytes().to_vec();
    let mut chunked = TerminalState::new(20, 4).unwrap();
    for byte in &bytes {
        chunked.feed(&[*byte]).unwrap();
    }
    assert_eq!(chunked.row_text(0), once.row_text(0));
}

#[test]
fn cjk_width_two_occupies_lead_and_continuation() {
    let mut terminal = TerminalState::new(8, 2).unwrap();
    terminal.feed("世".as_bytes()).unwrap();
    let lead = terminal.cell(0, 0).unwrap();
    let cont = terminal.cell(1, 0).unwrap();
    assert_eq!(lead.role, CellRole::Lead);
    assert_eq!(lead.width, 2);
    assert_eq!(lead.character, '世');
    assert_eq!(cont.role, CellRole::Continuation);
    assert_eq!(terminal.cursor().col, 2);
}

#[test]
fn overwrite_wide_lead_or_continuation_clears_unit() {
    let mut terminal = TerminalState::new(8, 2).unwrap();
    terminal.feed("世".as_bytes()).unwrap();
    terminal.feed(b"\x1b[1;1H").unwrap();
    terminal.feed(b"Z").unwrap();
    assert_eq!(terminal.cell(0, 0).unwrap().character, 'Z');
    assert_ne!(terminal.cell(1, 0).unwrap().role, CellRole::Continuation);

    let mut terminal = TerminalState::new(8, 2).unwrap();
    terminal.feed("世".as_bytes()).unwrap();
    terminal.feed(b"\x1b[1;2H").unwrap();
    terminal.feed(b"Q").unwrap();
    assert_eq!(terminal.cell(1, 0).unwrap().character, 'Q');
    assert_ne!(terminal.cell(0, 0).unwrap().role, CellRole::Lead);
}

#[test]
fn mode_2027_query_set_reset() {
    let mut terminal = TerminalState::new(40, 4).unwrap();
    assert!(terminal.modes().unicode_core);
    terminal.feed(b"\x1b[?2027$p").unwrap();
    assert_eq!(
        terminal.take_protocol_reply().unwrap().as_bytes(),
        b"\x1b[?2027;1$y"
    );
    terminal.feed(b"\x1b[?2027l\x1b[?2027$p").unwrap();
    assert!(!terminal.modes().unicode_core);
    assert_eq!(
        terminal.take_protocol_reply().unwrap().as_bytes(),
        b"\x1b[?2027;2$y"
    );
}

#[test]
fn decawm_reset_wide_final_column_ignored() {
    let hex =
        include_str!("../../../tests/fixtures/m002-unicode/decawm-reset-wide-final.input.hex");
    let mut terminal = TerminalState::new(3, 1).unwrap();
    feed_hex(&mut terminal, hex.trim());
    assert_eq!(row_text(&terminal, 0), "ab ");
    assert_eq!(terminal.cursor().col, 2);
    assert!(!terminal.pending_wrap());
    assert!(!terminal.modes().wraparound);
}

#[test]
fn decawm_reset_late_widen_rejects_vs16() {
    let hex =
        include_str!("../../../tests/fixtures/m002-unicode/decawm-reset-late-widen.input.hex");
    let mut terminal = TerminalState::new(3, 1).unwrap();
    feed_hex(&mut terminal, hex.trim());
    assert_eq!(row_text(&terminal, 0), "ab❤");
    assert_eq!(terminal.cell(2, 0).unwrap().character, '❤');
    assert_eq!(terminal.cell(2, 0).unwrap().width, 1);
    assert!(!terminal.pending_wrap());
}

#[test]
fn grapheme_payload_overflow_is_bounded() {
    let mut terminal = TerminalState::new(40, 4).unwrap();
    let mut payload = String::from("a");
    for _ in 0..4096 {
        payload.push('\u{0301}');
    }
    assert!(payload.len() > 8192);
    terminal.feed(payload.as_bytes()).unwrap();
    assert!(terminal.diagnostics().grapheme_payload_overflow_count >= 1);
    terminal.feed(b"Z").unwrap();
    // Recovers on next grapheme boundary.
    let texts: String = (0..terminal.cols())
        .filter_map(|c| terminal.cell(c, 0).map(|cell| cell.character))
        .collect();
    assert!(texts.contains('Z'));
}

#[test]
fn cursor_move_invalidates_active_anchor() {
    let mut terminal = TerminalState::new(20, 4).unwrap();
    terminal.feed(b"e").unwrap();
    terminal.feed(b"\x1b[1;1H").unwrap();
    terminal.feed("\u{0301}".as_bytes()).unwrap();
    // After invalidation, combining mark does not attach to prior 'e'.
    assert_eq!(terminal.cell(0, 0).unwrap().character, 'e');
}
