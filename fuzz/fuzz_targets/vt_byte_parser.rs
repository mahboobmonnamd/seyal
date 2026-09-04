#![no_main]

use libfuzzer_sys::fuzz_target;
use seyal_terminal::TerminalState;

fn new_terminal() -> TerminalState {
    let mut terminal = TerminalState::new(80, 24).expect("valid fuzz terminal");
    let _ = terminal.take_damage();
    terminal
}

fuzz_target!(|data: &[u8]| {
    let mut terminal = new_terminal();
    let _ = terminal.feed(data);
    let _ = terminal.finish_input();
    let _ = terminal.resize(132, 43);
    let _ = terminal.resize(80, 24);
    for row in 0..terminal.rows() {
        let _ = terminal.row_text(row);
        let _ = terminal.line_id(row);
    }
});
