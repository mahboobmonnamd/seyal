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

    terminal.feed(input);

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

    terminal.feed(input);

    assert_eq!(render(&terminal), expected);
    assert_eq!(terminal.diagnostics().deferred_sequences, 1);
    assert_eq!(terminal.diagnostics().malformed_sequences, 0);
}
