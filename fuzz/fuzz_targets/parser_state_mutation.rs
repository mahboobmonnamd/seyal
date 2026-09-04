#![no_main]

use libfuzzer_sys::fuzz_target;
use seyal_terminal::TerminalState;

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

fuzz_target!(|data: &[u8]| {
    let mut one_shot = new_terminal();
    let _ = one_shot.feed(data);
    let _ = one_shot.finish_input();

    let mut bytewise = new_terminal();
    for byte in data {
        let _ = bytewise.feed(&[*byte]);
    }
    let _ = bytewise.finish_input();
    assert_same_state(&one_shot, &bytewise);

    let _ = one_shot.resize(100, 30);
    let _ = bytewise.resize(100, 30);
    assert_same_state(&one_shot, &bytewise);
});
