#![no_main]

use libfuzzer_sys::fuzz_target;
use seyal_terminal::{CellRole, LineId, TerminalState};

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
        for col in 0..left.cols() {
            if let Some(cell) = left.cell(col, row) {
                if cell.role == CellRole::Lead && cell.width == 2 {
                    assert!(col + 1 < left.cols());
                    assert!(left
                        .cell(col + 1, row)
                        .is_some_and(|next| next.role == CellRole::Continuation));
                }
            }
        }
    }
    assert_eq!(
        left.primary_history_units_range(LineId(1), LineId(u64::MAX), 4_096),
        right.primary_history_units_range(LineId(1), LineId(u64::MAX), 4_096)
    );
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

    for (index, byte) in data.iter().take(8).enumerate() {
        let cols = u16::from(byte % 32) + 1;
        let rows = u16::from(data.get(index + 1).copied().unwrap_or(*byte) % 8) + 1;
        assert_eq!(one_shot.resize(cols, rows), bytewise.resize(cols, rows));
        assert_same_state(&one_shot, &bytewise);
    }

    for _ in 0..data.first().copied().unwrap_or(0).min(8) {
        assert_eq!(
            one_shot.evict_oldest_primary_history_segment(),
            bytewise.evict_oldest_primary_history_segment()
        );
        assert_same_state(&one_shot, &bytewise);
    }
});
