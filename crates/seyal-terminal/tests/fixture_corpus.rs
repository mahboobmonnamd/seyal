use seyal_terminal::{Color, TerminalState};

fn render(terminal: &TerminalState) -> String {
    let mut output = String::new();
    for row in 0..terminal.rows() {
        output.push_str(&terminal.row_text(row).expect("fixture row exists"));
        output.push('\n');
    }
    output
}

#[test]
fn retained_m001_basic_fixture_matches_canonical_state() {
    let input = include_bytes!("../../../tests/fixtures/vt/m001-basic.input");
    let expected = include_str!("../../../tests/fixtures/vt/m001-basic.expected.txt");
    let mut terminal = TerminalState::new(8, 3).expect("fixture dimensions are valid");

    terminal.feed(input).expect("fixture feed succeeds");

    assert_eq!(render(&terminal), expected);
    assert_eq!(terminal.cell(0, 1).unwrap().style.fg, Color::Indexed(1));
    assert_eq!(terminal.cursor().row, 1);
    assert_eq!(terminal.cursor().col, 4);
}

#[test]
fn retained_deferred_osc_fixture_preserves_parser_continuity() {
    let input = include_bytes!("../../../tests/fixtures/vt/m001-deferred-osc.input");
    let expected = include_str!("../../../tests/fixtures/vt/m001-deferred-osc.expected.txt");
    let mut terminal = TerminalState::new(8, 2).expect("fixture dimensions are valid");

    terminal.feed(input).expect("fixture feed succeeds");

    assert_eq!(render(&terminal), expected);
    assert_eq!(terminal.diagnostics().deferred_sequences, 1);
    assert_eq!(terminal.diagnostics().malformed_sequences, 0);
}

#[test]
fn retained_ecma48_core_fixture_matches_canonical_state_and_style() {
    let input = include_bytes!("../../../tests/fixtures/vt/m001-ecma48-core.input");
    let expected = include_str!("../../../tests/fixtures/vt/m001-ecma48-core.expected.txt");
    let mut terminal = TerminalState::new(8, 3).expect("fixture dimensions are valid");

    terminal.feed(input).expect("fixture feed succeeds");

    assert_eq!(render(&terminal), expected);
    let styled = terminal.cell(4, 0).expect("styled fixture cell exists");
    assert_eq!(styled.style.fg, Color::Indexed(1));
    assert_eq!(styled.style.bg, Color::Indexed(4));
    assert!(styled.style.bold);
    assert!(styled.style.underline);
}

#[test]
fn retained_ecma48_erase_save_fixture_matches_canonical_state() {
    let input = include_bytes!("../../../tests/fixtures/vt/m001-ecma48-erase-save.input");
    let expected = include_str!("../../../tests/fixtures/vt/m001-ecma48-erase-save.expected.txt");
    let mut terminal = TerminalState::new(5, 2).expect("fixture dimensions are valid");

    terminal.feed(input).expect("fixture feed succeeds");

    assert_eq!(render(&terminal), expected);
    assert_eq!(terminal.cursor().row, 1);
    assert_eq!(terminal.cursor().col, 3);
}

#[test]
fn retained_xterm_private_fixture_preserves_primary_and_cursor_visibility() {
    let input = include_bytes!("../../../tests/fixtures/vt/m001-xterm-private.input");
    let expected = include_str!("../../../tests/fixtures/vt/m001-xterm-private.expected.txt");
    let mut terminal = TerminalState::new(4, 2).expect("fixture dimensions are valid");

    terminal.feed(input).expect("fixture feed succeeds");

    assert_eq!(render(&terminal), expected);
    assert!(!terminal.modes().alternate_screen);
    assert!(!terminal.modes().cursor_visible);
    assert!(!terminal.cursor().visible);
}

#[test]
fn retained_utf8_fixture_is_chunk_boundary_independent() {
    let input = include_bytes!("../../../tests/fixtures/vt/m001-utf8.input");
    let expected = include_str!("../../../tests/fixtures/vt/m001-utf8.expected.txt");

    let mut one_shot = TerminalState::new(8, 2).expect("fixture dimensions are valid");
    one_shot.feed(input).expect("fixture feed succeeds");
    assert_eq!(render(&one_shot), expected);

    let mut bytewise = TerminalState::new(8, 2).expect("fixture dimensions are valid");
    for byte in input {
        bytewise
            .feed(&[*byte])
            .expect("bytewise fixture feed succeeds");
    }

    assert_eq!(render(&bytewise), expected);
    assert_eq!(one_shot.cursor(), bytewise.cursor());
    assert_eq!(one_shot.diagnostics(), bytewise.diagnostics());
}
