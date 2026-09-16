use seyal_terminal::{
    CopyModeMotion, HistoryBreakAfter, HistoryRangeError, PasteError, SelectionKind, TerminalState,
    VisualPos,
};

fn feed(terminal: &mut TerminalState, bytes: &[u8]) {
    terminal.feed(bytes).expect("feed");
}

#[test]
fn copy_preserves_hard_newline_and_not_soft_wrap() {
    let mut terminal = TerminalState::new(4, 1).unwrap();
    feed(&mut terminal, b"abcd\r\nefgh\r\n");
    let source = terminal.primary_history_units_range(
        seyal_terminal::LineId(1),
        seyal_terminal::LineId(u64::MAX),
        32,
    );
    let start = source[0].anchor;
    let end = source[source.len() - 1].anchor;
    let text = terminal.copy_history_range(start, end).unwrap();
    assert_eq!(text, "abcd\nefgh");

    let mut wrapped = TerminalState::new(4, 1).unwrap();
    feed(&mut wrapped, b"abcdefgh\r\n");
    let units = wrapped.primary_history_units_range(
        seyal_terminal::LineId(1),
        seyal_terminal::LineId(u64::MAX),
        32,
    );
    assert!(units
        .iter()
        .any(|u| u.break_after == HistoryBreakAfter::SoftWrap));
    let start = units[0].anchor;
    let end = units[units.len() - 1].anchor;
    let text = wrapped.copy_history_range(start, end).unwrap();
    assert_eq!(text, "abcdefgh");
}

#[test]
fn linear_visual_copy_does_not_split_graphemes() {
    let mut terminal = TerminalState::new(8, 1).unwrap();
    feed(&mut terminal, "a e\u{301} z".as_bytes());
    terminal.set_linear_selection(VisualPos { col: 2, row: 0 }, VisualPos { col: 2, row: 0 });
    let text = terminal.copy_selection_text().unwrap();
    assert_eq!(text, "e\u{301}");
}

#[test]
fn rectangular_copy_uses_visual_row_separators() {
    let mut terminal = TerminalState::new(4, 2).unwrap();
    feed(&mut terminal, b"abcd\r\nefgh");
    terminal.set_rectangular_selection(VisualPos { col: 1, row: 0 }, VisualPos { col: 2, row: 1 });
    assert_eq!(
        terminal.selection_session().kind,
        SelectionKind::Rectangular
    );
    let text = terminal.copy_selection_text().unwrap();
    assert_eq!(text, "bc\nfg");
}

#[test]
fn search_next_and_prev_cycle_source_matches() {
    let mut terminal = TerminalState::new(8, 1).unwrap();
    feed(&mut terminal, b"one two\r\none\r\n");
    let first = terminal.search_and_select("one", true).expect("first");
    let second = terminal.search_and_select("one", true).expect("second");
    assert_ne!(first.start.line_id, second.start.line_id);
    let wrapped = terminal.search_and_select("one", true).expect("wrap");
    assert_eq!(wrapped.start, first.start);
    let prev = terminal.search_and_select("one", false).expect("prev");
    assert_eq!(prev.start, second.start);
}

#[test]
fn keyboard_copy_mode_yanks_linear_range_without_mouse() {
    let mut terminal = TerminalState::new(8, 1).unwrap();
    feed(&mut terminal, b"abcdef");
    terminal.enter_copy_mode();
    assert!(terminal.copy_mode().active);
    // Cursor is at the cell after 'f' or on 'f' depending on wrap; move to col 1
    // and mark, then move to col 3 and yank "bcd" or similar.
    for _ in 0..8 {
        terminal.copy_mode_motion(CopyModeMotion::Left);
    }
    terminal.copy_mode_motion(CopyModeMotion::Right); // col 1
    terminal.copy_mode_toggle_anchor();
    terminal.copy_mode_motion(CopyModeMotion::Right);
    terminal.copy_mode_motion(CopyModeMotion::Right);
    let yanked = terminal.yank_selection().unwrap();
    assert!(
        yanked.contains('b') && yanked.contains('c'),
        "keyboard yank should copy a contiguous source span, got {yanked:?}"
    );
    assert!(!terminal.copy_mode().active);
    assert_eq!(
        terminal.take_copy_buffer().as_deref(),
        Some(yanked.as_str())
    );
}

#[test]
fn bracketed_paste_mode_is_canonical_and_wraps_host_bytes() {
    let mut terminal = TerminalState::new(4, 1).unwrap();
    assert!(!terminal.modes().bracketed_paste);
    feed(&mut terminal, b"\x1b[?2004h");
    assert!(terminal.modes().bracketed_paste);
    feed(&mut terminal, b"\x1b[?2004$p");
    let reply = terminal.take_protocol_reply().expect("decrqm");
    assert_eq!(reply.as_bytes(), b"\x1b[?2004;1$y");
    let wrapped = terminal.encode_host_paste(b"ab\x00c\x1b[201~d").unwrap();
    assert_eq!(wrapped, b"\x1b[200~abcd\x1b[201~");
    feed(&mut terminal, b"\x1b[?2004l");
    assert_eq!(terminal.encode_host_paste(b"hi").unwrap(), b"hi");
}

#[test]
fn paste_rejects_empty_and_oversized_payloads() {
    let terminal = TerminalState::new(4, 1).unwrap();
    assert_eq!(
        terminal.encode_host_paste(b"").unwrap_err(),
        PasteError::Empty
    );
    let huge = vec![b'x'; seyal_terminal::MAX_PASTE_BYTES + 1];
    assert_eq!(
        terminal.encode_host_paste(&huge).unwrap_err(),
        PasteError::TooLarge
    );
}

#[test]
fn alternate_screen_clears_host_selection_and_copy_mode() {
    let mut terminal = TerminalState::new(4, 2).unwrap();
    feed(&mut terminal, b"abcd");
    terminal.set_linear_selection(VisualPos { col: 0, row: 0 }, VisualPos { col: 1, row: 0 });
    terminal.enter_copy_mode();
    feed(&mut terminal, b"\x1b[?1049h");
    assert!(terminal.selection_session().start.is_none());
    assert!(!terminal.copy_mode().active);
}

#[test]
fn selection_mutations_commit_display_damage() {
    let mut terminal = TerminalState::new(4, 1).unwrap();
    feed(&mut terminal, b"abcd");
    let _ = terminal.take_damage();
    terminal.set_linear_selection(VisualPos { col: 0, row: 0 }, VisualPos { col: 1, row: 0 });
    let damage = terminal.take_damage().expect("selection damage");
    assert!(damage.full);
    terminal.enter_copy_mode();
    assert!(terminal.take_damage().is_some());
}

#[test]
fn linear_selection_copy_survives_scroll_as_source_anchors() {
    // SPEC-010 §11: selection endpoints are source anchors. Scrolling the
    // selected text into retained history must not silently retarget copy to
    // whatever now occupies the original visual cells.
    let mut terminal = TerminalState::new(8, 2).unwrap();
    feed(&mut terminal, b"SECRET03");
    terminal.set_linear_selection(VisualPos { col: 0, row: 0 }, VisualPos { col: 7, row: 0 });
    assert_eq!(terminal.copy_selection_text().unwrap(), "SECRET03");
    feed(&mut terminal, b"\r\nKEEPKEEP\r\nNEWNEW01");
    assert_eq!(
        terminal.copy_selection_text().unwrap(),
        "SECRET03",
        "scroll must not retarget a linear selection to new visual occupants"
    );
    // After scroll, SECRET03 is off the 2-row viewport; highlight on the new
    // occupants must not claim the stale visual corners.
    let row0 = terminal.row_text(0).unwrap_or_default();
    assert!(
        row0.starts_with("KEEP") || row0.starts_with("NEW") || row0.starts_with("SECRET"),
        "expected scrolled viewport content, got {row0:?}"
    );
    if !row0.starts_with("SECRET") {
        assert!(
            !terminal.selection_covers_cell(0, 0),
            "viewport cell with different source must not stay highlighted"
        );
    }
}

#[test]
fn linear_selection_endpoints_stale_after_eviction() {
    // SPEC-010 §11: eviction of either endpoint is explicit; do not fall
    // through to unrelated visual cells.
    let mut terminal = TerminalState::new(8, 1).unwrap();
    feed(&mut terminal, b"EVICTME!");
    terminal.set_linear_selection(VisualPos { col: 0, row: 0 }, VisualPos { col: 7, row: 0 });
    let start = terminal
        .selection_session()
        .start_anchor
        .expect("selection stores source start");
    let end = terminal
        .selection_session()
        .end_anchor
        .expect("selection stores source end");
    assert_eq!(terminal.copy_selection_text().unwrap(), "EVICTME!");

    // Push the selected line into sealed history, then evict that segment.
    feed(&mut terminal, b"\r\n");
    let line = format!("{}\r\n", "a".repeat(512));
    for _ in 0..128 {
        feed(&mut terminal, line.as_bytes());
    }
    assert!(terminal.evict_oldest_primary_history_segment() > 0);
    match terminal.copy_selection_text() {
        Err(HistoryRangeError::Stale) => {}
        Ok(text) => panic!("evicted selection must be Stale, got {text:?}"),
        Err(other) => panic!("expected Stale after eviction, got {other:?}"),
    }
    assert_eq!(terminal.selection_session().start_anchor, Some(start));
    assert_eq!(terminal.selection_session().end_anchor, Some(end));
}

#[test]
fn linear_selection_anchors_stable_across_resize_oscillation() {
    let mut terminal = TerminalState::new(8, 2).unwrap();
    feed(&mut terminal, "ab\u{301}cdef".as_bytes());
    terminal.set_linear_selection(VisualPos { col: 1, row: 0 }, VisualPos { col: 3, row: 0 });
    let before = terminal.copy_selection_text().unwrap();
    assert!(
        before.contains('b') && before.contains('\u{301}'),
        "pre-resize copy should include the combining grapheme, got {before:?}"
    );
    terminal.resize(4, 2).unwrap();
    terminal.resize(12, 3).unwrap();
    terminal.resize(8, 2).unwrap();
    assert_eq!(
        terminal.copy_selection_text().unwrap(),
        before,
        "resize/reflow must keep the same source-anchored selection text"
    );
}
