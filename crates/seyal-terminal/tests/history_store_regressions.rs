use seyal_terminal::{
    CellRole, Color, HistoryAnchor, HistoryAnchorResolution, HistoryRangeError, LineId,
    TerminalState,
};

#[test]
fn reflow_at_one_column_never_emits_an_orphan_wide_unit() {
    let mut terminal = TerminalState::new(2, 1).expect("terminal");
    terminal.feed("界\r\n".as_bytes()).expect("feed");

    let rows = terminal.primary_history_reflow(1, 8);
    assert!(rows.iter().all(|row| row.cells.len() <= 1));
    assert!(rows.iter().all(|row| {
        row.cells
            .first()
            .is_none_or(|cell| cell.role != CellRole::Lead || cell.width < 2)
    }));
}

#[test]
fn narrowing_active_wide_unit_to_one_column_preserves_source_without_an_orphan() {
    let mut terminal = TerminalState::new(2, 1).expect("terminal");
    let source_id = terminal.line_id(0).expect("source line");
    terminal.feed("界\u{301}".as_bytes()).expect("feed");

    terminal.resize(1, 1).expect("resize");

    let visible = terminal.cell(0, 0).expect("cell");
    assert_ne!(visible.role, CellRole::Lead);
    assert!(terminal
        .primary_history_units_range(source_id, source_id, 8)
        .iter()
        .any(|unit| unit.text == "界\u{301}" && unit.width == 2));
}

#[test]
fn shrinking_rows_preserves_every_active_source_line() {
    let mut terminal = TerminalState::new(4, 4).expect("terminal");
    terminal.feed(b"a\r\nb\r\nc\r\nd").expect("feed");
    let source_ids = (0..4)
        .map(|row| terminal.line_id(row).expect("line id"))
        .collect::<Vec<_>>();

    terminal.resize(4, 2).expect("resize");

    for (line_id, expected) in source_ids.into_iter().zip(["a", "b", "c", "d"]) {
        let text = terminal
            .primary_history_units_range(line_id, line_id, 8)
            .into_iter()
            .map(|unit| unit.text)
            .collect::<String>();
        assert_eq!(text, expected, "source line {line_id:?} was lost");
    }
}

#[test]
fn resize_does_not_join_rows_separated_by_cursor_motion() {
    let mut terminal = TerminalState::new(4, 3).expect("terminal");
    terminal.feed(b"a\x1b[3;1Hb").expect("feed");
    let first = terminal.line_id(0).expect("first line");
    let third = terminal.line_id(2).expect("third line");

    terminal.resize(8, 3).expect("resize");

    assert_eq!(terminal.row_text(0).as_deref(), Some("a       "));
    assert_eq!(terminal.row_text(2).as_deref(), Some("b       "));
    assert_eq!(terminal.line_id(0), Some(first));
    assert_eq!(terminal.line_id(2), Some(third));
}

#[test]
fn resize_oscillation_preserves_multiscalar_payload_and_source_anchor() {
    let mut terminal = TerminalState::new(8, 2).expect("terminal");
    terminal
        .feed("ab界\u{301}cd".as_bytes())
        .expect("unicode feed");
    let source_id = terminal.line_id(0).expect("source line");

    for cols in [4, 8, 3, 8, 1, 8] {
        terminal.resize(cols, 2).expect("resize");
        let units = terminal.primary_history_units_range(source_id, source_id, 32);
        assert_eq!(
            units
                .iter()
                .map(|unit| unit.text.as_str())
                .collect::<String>(),
            "ab界\u{301}cd",
            "canonical payload changed at {cols} columns"
        );
        assert_eq!(
            units.first().map(|unit| unit.anchor.line_id),
            Some(source_id)
        );
    }
}

#[test]
fn output_and_scroll_after_resize_preserve_source_ids_offsets_lineage_and_style() {
    let mut terminal = TerminalState::new(4, 2).expect("terminal");
    terminal
        .feed(b"\x1b[31mabcdef")
        .expect("soft-wrapped styled input");
    let first = terminal.line_id(0).expect("first source id");
    let second = terminal.line_id(1).expect("second source id");

    terminal.resize(8, 2).expect("widen");
    terminal.feed(b"g\r\nh\r\n").expect("output after resize");

    let first_units = terminal.primary_history_units_range(first, first, 16);
    assert_eq!(
        first_units
            .iter()
            .map(|unit| unit.text.as_str())
            .collect::<String>(),
        "abcd"
    );
    assert_eq!(
        first_units
            .iter()
            .map(|unit| unit.anchor.unit_offset)
            .collect::<Vec<_>>(),
        [0, 1, 2, 3]
    );
    assert!(first_units
        .iter()
        .all(|unit| unit.style.fg == Color::Indexed(1)));

    let second_units = terminal.primary_history_units_range(second, second, 16);
    assert_eq!(
        second_units
            .iter()
            .map(|unit| unit.text.as_str())
            .collect::<String>(),
        "efg"
    );
    assert_eq!(
        second_units
            .iter()
            .map(|unit| unit.anchor.unit_offset)
            .collect::<Vec<_>>(),
        [0, 1, 2]
    );
    assert!(second_units
        .iter()
        .all(|unit| unit.style.fg == Color::Indexed(1)));
}

#[test]
fn evicted_range_and_anchor_are_explicitly_unavailable() {
    let mut terminal = TerminalState::new(512, 1).expect("terminal");
    let line = format!("{}\r\n", "a".repeat(512));
    for _ in 0..128 {
        terminal.feed(line.as_bytes()).expect("history feed");
    }
    let anchor = HistoryAnchor {
        line_id: LineId(1),
        unit_offset: 0,
    };
    assert!(matches!(
        terminal.primary_history_unit(anchor),
        HistoryAnchorResolution::Resolved { .. }
    ));

    assert!(terminal.evict_oldest_primary_history_segment() > 0);

    assert_eq!(
        terminal.primary_history_unit(anchor),
        HistoryAnchorResolution::Unavailable
    );
    assert_eq!(
        terminal.primary_history_range(LineId(1), LineId(1), 8),
        Err(HistoryRangeError::Stale)
    );
    assert_eq!(
        terminal.primary_history_unit(HistoryAnchor {
            line_id: LineId(u64::MAX),
            unit_offset: 0,
        }),
        HistoryAnchorResolution::Invalid
    );
}
