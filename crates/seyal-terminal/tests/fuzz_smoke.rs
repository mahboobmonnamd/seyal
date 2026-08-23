use std::{env, fs, path::PathBuf};

use seyal_terminal::TerminalState;

fn input() -> Vec<u8> {
    let path =
        PathBuf::from(env::var_os("SEYAL_FUZZ_INPUT").expect("SEYAL_FUZZ_INPUT is required"));
    fs::read(path).expect("read retained fuzz seed")
}

fn new_terminal() -> TerminalState {
    let mut terminal = TerminalState::new(80, 24).expect("valid fuzz terminal");
    let _ = terminal.take_damage();
    terminal
}

fn assert_same_state(left: &TerminalState, right: &TerminalState) {
    assert_eq!(left.cols(), right.cols());
    assert_eq!(left.rows(), right.rows());
    assert_eq!(left.cursor(), right.cursor());
    assert_eq!(left.modes(), right.modes());
    assert_eq!(left.diagnostics(), right.diagnostics());
    for row in 0..left.rows() {
        assert_eq!(left.row_text(row), right.row_text(row));
        assert_eq!(left.line_id(row), right.line_id(row));
    }
}

#[test]
fn vt_byte_parser_seed() {
    let bytes = input();
    let mut terminal = new_terminal();
    terminal.feed(&bytes);
    terminal.finish_input();

    // Exercise state mutation after arbitrary parser input. Any panic, invalid
    // dimension transition or unsafe parser state fails the retained seed.
    terminal.resize(132, 43).expect("valid grow resize");
    terminal.resize(80, 24).expect("valid shrink resize");
    for row in 0..terminal.rows() {
        let _ = terminal.row_text(row).expect("row remains addressable");
        let _ = terminal.line_id(row).expect("row retains logical identity");
    }
}

#[test]
fn parser_state_mutation_seed() {
    let bytes = input();

    let mut one_shot = new_terminal();
    one_shot.feed(&bytes);
    one_shot.finish_input();

    let mut bytewise = new_terminal();
    for byte in &bytes {
        bytewise.feed(&[*byte]);
    }
    bytewise.finish_input();

    assert_same_state(&one_shot, &bytewise);

    one_shot.resize(100, 30).expect("one-shot grow resize");
    bytewise.resize(100, 30).expect("bytewise grow resize");
    assert_same_state(&one_shot, &bytewise);
}
