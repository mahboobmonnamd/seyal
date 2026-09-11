use std::{env, fs, path::PathBuf};

use seyal_terminal::{CellRole, HistoryBreakAfter, LineId, TerminalState};

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

fn live_line_ids(terminal: &TerminalState) -> std::collections::HashSet<LineId> {
    (0..terminal.rows())
        .filter_map(|row| terminal.line_id(row))
        .collect()
}

fn snapshot_retained_history(
    terminal: &TerminalState,
) -> Vec<(seyal_terminal::HistoryAnchor, String, u8)> {
    let live = live_line_ids(terminal);
    terminal
        .primary_history_units_range(LineId(1), LineId(u64::MAX), 4_096)
        .into_iter()
        .filter(|unit| unit.break_after == HistoryBreakAfter::HardBreak)
        .filter(|unit| !live.contains(&unit.anchor.line_id))
        .map(|unit| (unit.anchor, unit.text.clone(), unit.width))
        .collect()
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
            if let Some(cell) = left.cell(col, row)
                && cell.role == CellRole::Lead
                && cell.width == 2
            {
                assert!(col + 1 < left.cols());
                assert!(left
                    .cell(col + 1, row)
                    .is_some_and(|next| next.role == CellRole::Continuation));
            }
        }
    }
    assert_eq!(
        left.primary_history_units_range(LineId(1), LineId(u64::MAX), 4_096),
        right.primary_history_units_range(LineId(1), LineId(u64::MAX), 4_096)
    );
}

#[test]
#[ignore = "executed by fuzz/targets/vt-byte-parser with a retained seed"]
fn vt_byte_parser_seed() {
    let bytes = input();
    let mut terminal = new_terminal();
    terminal.feed(&bytes).expect("fuzz feed succeeds");
    terminal.finish_input().expect("fuzz finish succeeds");

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
#[ignore = "executed by fuzz/targets/parser-state-mutation with a retained seed"]
fn parser_state_mutation_seed() {
    let bytes = input();

    let mut one_shot = new_terminal();
    one_shot.feed(&bytes).expect("one-shot feed succeeds");
    one_shot
        .finish_input()
        .expect("one-shot finish input succeeds");

    let mut bytewise = new_terminal();
    for byte in &bytes {
        bytewise.feed(&[*byte]).expect("bytewise feed succeeds");
    }
    bytewise
        .finish_input()
        .expect("bytewise finish input succeeds");

    assert_same_state(&one_shot, &bytewise);

    for (index, byte) in bytes.iter().take(8).enumerate() {
        let cols = u16::from(byte % 32) + 1;
        let rows = u16::from(bytes.get(index + 1).copied().unwrap_or(*byte) % 8) + 1;
        assert_eq!(one_shot.resize(cols, rows), bytewise.resize(cols, rows));
        assert_same_state(&one_shot, &bytewise);
    }

    for _ in 0..bytes.first().copied().unwrap_or(0).min(8) {
        assert_eq!(
            one_shot.evict_oldest_primary_history_segment(),
            bytewise.evict_oldest_primary_history_segment()
        );
        assert_same_state(&one_shot, &bytewise);
    }
}

#[test]
#[ignore = "executed by fuzz/targets/history-resize-eviction with a retained seed"]
fn history_resize_eviction_seed() {
    const SPEC_WIDTHS: [u16; 7] = [40, 48, 64, 80, 96, 132, 160];
    let bytes = input();
    let cols = SPEC_WIDTHS[usize::from(bytes.first().copied().unwrap_or(80)) % SPEC_WIDTHS.len()];
    let rows = u16::from(bytes.get(1).copied().unwrap_or(2) % 8) + 1;

    let mut one_shot = TerminalState::new(cols, rows).expect("one-shot terminal");
    let _ = one_shot.take_damage();
    one_shot.feed(&bytes).expect("one-shot feed succeeds");
    one_shot
        .finish_input()
        .expect("one-shot finish input succeeds");

    let mut bytewise = TerminalState::new(cols, rows).expect("bytewise terminal");
    let _ = bytewise.take_damage();
    for byte in &bytes {
        bytewise.feed(&[*byte]).expect("bytewise feed succeeds");
    }
    bytewise
        .finish_input()
        .expect("bytewise finish input succeeds");

    assert_eq!(
        one_shot.primary_history_units_range(LineId(1), LineId(u64::MAX), 4_096),
        bytewise.primary_history_units_range(LineId(1), LineId(u64::MAX), 4_096)
    );

    let mut snapshot = snapshot_retained_history(&one_shot);
    for (index, byte) in bytes.iter().take(SPEC_WIDTHS.len()).enumerate() {
        let next_cols = SPEC_WIDTHS[usize::from(*byte) % SPEC_WIDTHS.len()];
        let next_rows = u16::from(bytes.get(index + 2).copied().unwrap_or(*byte) % 8) + 1;
        one_shot
            .resize(next_cols, next_rows)
            .expect("one-shot resize");
        bytewise
            .resize(next_cols, next_rows)
            .expect("bytewise resize");
        if !one_shot.modes().alternate_screen {
            let after = snapshot_retained_history(&one_shot);
            for (anchor, text, width) in &snapshot {
                if let Some((_, got, got_width)) = after.iter().find(|(item, _, _)| item == anchor)
                {
                    assert_eq!(got, text, "hard-break unit changed text at {anchor:?}");
                    assert_eq!(
                        got_width, width,
                        "hard-break unit changed width at {anchor:?}"
                    );
                }
            }
            snapshot = snapshot_retained_history(&one_shot);
        }
        assert_eq!(
            one_shot.primary_history_units_range(LineId(1), LineId(u64::MAX), 4_096),
            bytewise.primary_history_units_range(LineId(1), LineId(u64::MAX), 4_096)
        );
    }

    for _ in 0..bytes.first().copied().unwrap_or(0).min(8) {
        let _ = one_shot.evict_oldest_primary_history_segment();
        let _ = bytewise.evict_oldest_primary_history_segment();
    }
}
