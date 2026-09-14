use seyal_terminal::{HostPresentationEvent, TerminalState, MAX_HOST_PRESENTATION_EVENTS};

fn feed(terminal: &mut TerminalState, bytes: &[u8]) {
    terminal.feed(bytes).expect("feed succeeds");
}

fn row(terminal: &TerminalState, row: u16) -> String {
    terminal.row_text(row).expect("row")
}

#[test]
fn decstbm_il_dl_and_su_sd_preserve_outside_region() {
    let mut terminal = TerminalState::new(4, 4).unwrap();
    feed(&mut terminal, b"AAAA\r\nBBBB\r\nCCCC\r\nDDDD");
    feed(&mut terminal, b"\x1b[2;3r\x1b[2;1H\x1b[M");
    // DL at row2 within region rows 2-3: delete BBBB, pull CCCC up, blank bottom of region
    assert_eq!(row(&terminal, 0), "AAAA");
    assert_eq!(row(&terminal, 1), "CCCC");
    assert_eq!(row(&terminal, 2), "    ");
    assert_eq!(row(&terminal, 3), "DDDD");

    feed(
        &mut terminal,
        b"\x1b[r\x1b[1;1HXXXX\r\nYYYY\r\nZZZZ\r\nWWWW",
    );
    feed(&mut terminal, b"\x1b[2;4r\x1b[1T");
    // SD 1 inside rows 2-4: insert blank at top of region
    assert_eq!(row(&terminal, 0), "XXXX");
    assert_eq!(row(&terminal, 1), "    ");
    assert_eq!(row(&terminal, 2), "YYYY");
    assert_eq!(row(&terminal, 3), "ZZZZ");
}

#[test]
fn partial_region_scroll_does_not_enter_primary_history() {
    let mut terminal = TerminalState::new(4, 3).unwrap();
    feed(&mut terminal, b"AAAA\r\nBBBB\r\nCCCC");
    let before = terminal
        .primary_history_range(
            seyal_terminal::LineId(1),
            seyal_terminal::LineId(u64::MAX),
            8,
        )
        .unwrap();
    feed(&mut terminal, b"\x1b[2;3r\x1b[2S");
    let after = terminal
        .primary_history_range(
            seyal_terminal::LineId(1),
            seyal_terminal::LineId(u64::MAX),
            8,
        )
        .unwrap();
    assert_eq!(before.len(), after.len());
    assert_eq!(row(&terminal, 0), "AAAA");
}

#[test]
fn reverse_index_at_top_margin_scrolls_region_down() {
    let mut terminal = TerminalState::new(3, 3).unwrap();
    feed(&mut terminal, b"AAA\r\nBBB\r\nCCC");
    feed(&mut terminal, b"\x1b[2;3r\x1b[2;1H\x1bM");
    assert_eq!(row(&terminal, 0), "AAA");
    assert_eq!(row(&terminal, 1), "   ");
    assert_eq!(row(&terminal, 2), "BBB");
}

#[test]
fn ich_dch_ech_edit_current_line() {
    let mut terminal = TerminalState::new(8, 1).unwrap();
    feed(
        &mut terminal,
        b"ABCDEFGH\x1b[1;3H\x1b[2@\x1b[1;6H\x1b[2P\x1b[1;1H\x1b[2X",
    );
    assert_eq!(row(&terminal, 0), "    CF  ");
}

#[test]
fn osc_presentation_events_are_bounded_and_do_not_execute() {
    let mut terminal = TerminalState::new(8, 2).unwrap();
    feed(&mut terminal, b"\x1b]0;Title\x07");
    match terminal.take_host_presentation_event() {
        Some(HostPresentationEvent::IconAndWindowTitle(payload)) => {
            assert_eq!(payload.as_bytes(), b"Title");
        }
        other => panic!("unexpected event: {other:?}"),
    }
    feed(&mut terminal, b"\x1b]2;Win\x07");
    match terminal.take_host_presentation_event() {
        Some(HostPresentationEvent::WindowTitle(payload)) => {
            assert_eq!(payload.as_bytes(), b"Win");
        }
        other => panic!("unexpected event: {other:?}"),
    }
    feed(&mut terminal, b"\x1b]7;file:///tmp/work\x07");
    match terminal.take_host_presentation_event() {
        Some(HostPresentationEvent::WorkingDirectory(payload)) => {
            assert_eq!(payload.as_bytes(), b"file:///tmp/work");
        }
        other => panic!("unexpected event: {other:?}"),
    }
    feed(&mut terminal, b"\x1b]8;id=1;https://example.com\x07");
    match terminal.take_host_presentation_event() {
        Some(HostPresentationEvent::Hyperlink { id, uri }) => {
            assert_eq!(id.as_bytes(), b"1");
            assert_eq!(uri.as_bytes(), b"https://example.com");
        }
        other => panic!("unexpected event: {other:?}"),
    }
    feed(&mut terminal, b"\x1b]8;;\x07");
    match terminal.take_host_presentation_event() {
        Some(HostPresentationEvent::Hyperlink { id, uri }) => {
            assert!(id.as_bytes().is_empty());
            assert!(uri.as_bytes().is_empty());
        }
        other => panic!("unexpected event: {other:?}"),
    }
    assert!(terminal.take_host_presentation_event().is_none());
}

#[test]
fn host_presentation_queue_is_bounded() {
    let mut terminal = TerminalState::new(8, 2).unwrap();
    let before = terminal.diagnostics().deferred_sequences;
    for _ in 0..(MAX_HOST_PRESENTATION_EVENTS + 3) {
        feed(&mut terminal, b"\x1b]2;x\x07");
    }
    let mut count = 0;
    while terminal.take_host_presentation_event().is_some() {
        count += 1;
    }
    assert_eq!(count, MAX_HOST_PRESENTATION_EVENTS);
    assert!(terminal.diagnostics().deferred_sequences > before);
}

#[test]
fn primary_da_and_decrqm_1049_reply_through_protocol_seam() {
    let mut terminal = TerminalState::new(80, 24).unwrap();
    feed(&mut terminal, b"\x1b[c\x1b[0c");
    assert_eq!(
        terminal.take_protocol_reply().unwrap().as_bytes(),
        b"\x1b[?1;2c"
    );
    assert_eq!(
        terminal.take_protocol_reply().unwrap().as_bytes(),
        b"\x1b[?1;2c"
    );

    feed(&mut terminal, b"\x1b[?1049h\x1b[?1049$p");
    assert_eq!(
        terminal.take_protocol_reply().unwrap().as_bytes(),
        b"\x1b[?1049;1$y"
    );
    feed(&mut terminal, b"\x1b[?1049l\x1b[?1049$p");
    assert_eq!(
        terminal.take_protocol_reply().unwrap().as_bytes(),
        b"\x1b[?1049;2$y"
    );
}
