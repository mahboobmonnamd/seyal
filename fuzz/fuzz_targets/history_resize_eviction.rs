#![no_main]

use libfuzzer_sys::fuzz_target;
use seyal_terminal::{CellRole, HistoryAnchor, HistoryAnchorResolution, LineId, TerminalState};

const SPEC_WIDTHS: [u16; 7] = [40, 48, 64, 80, 96, 132, 160];

fn new_terminal(cols: u16, rows: u16) -> TerminalState {
    let mut terminal = TerminalState::new(cols, rows).expect("valid fuzz terminal");
    let _ = terminal.take_damage();
    terminal
}

fn spec_width(byte: u8) -> u16 {
    SPEC_WIDTHS[usize::from(byte) % SPEC_WIDTHS.len()]
}

fn snapshot_retained_units(terminal: &TerminalState) -> Vec<(HistoryAnchor, String, u8)> {
    terminal
        .primary_history_units_range(LineId(1), LineId(u64::MAX), 4_096)
        .into_iter()
        .filter(|unit| {
            matches!(
                terminal.primary_history_unit(unit.anchor),
                HistoryAnchorResolution::Resolved { .. }
            )
        })
        .map(|unit| (unit.anchor, unit.text, unit.width))
        .collect()
}

fn assert_wide_units_atomic(terminal: &TerminalState) {
    for row in 0..terminal.rows() {
        for col in 0..terminal.cols() {
            if let Some(cell) = terminal.cell(col, row)
                && cell.role == CellRole::Lead
                && cell.width == 2
            {
                assert!(col + 1 < terminal.cols());
                assert!(terminal
                    .cell(col + 1, row)
                    .is_some_and(|next| next.role == CellRole::Continuation));
            }
        }
    }
}

fn assert_surviving_anchors(before: &[(HistoryAnchor, String, u8)], terminal: &TerminalState) {
    for (anchor, text, width) in before {
        match terminal.primary_history_unit(*anchor) {
            HistoryAnchorResolution::Resolved {
                text: got,
                width: got_width,
                ..
            } => {
                assert_eq!(&got, text, "resize/eviction changed canonical unit text");
                assert_eq!(got_width, *width, "resize/eviction changed canonical width");
            }
            HistoryAnchorResolution::Unavailable => {}
            HistoryAnchorResolution::Invalid => {
                // Resize can pull a retained suffix back onto the live
                // viewport. That LineId remains allocated; it is no longer a
                // HistoryStore record until it scrolls out again.
            }
        }
    }
}

fn assert_same_history(left: &TerminalState, right: &TerminalState) {
    assert_eq!(left.cols(), right.cols());
    assert_eq!(left.rows(), right.rows());
    assert_eq!(left.cursor(), right.cursor());
    assert_eq!(
        left.modes().alternate_screen,
        right.modes().alternate_screen
    );
    assert_eq!(
        left.primary_history_units_range(LineId(1), LineId(u64::MAX), 4_096),
        right.primary_history_units_range(LineId(1), LineId(u64::MAX), 4_096)
    );
}

fuzz_target!(|data: &[u8]| {
    let cols = spec_width(data.first().copied().unwrap_or(80));
    let rows = u16::from(data.get(1).copied().unwrap_or(2) % 8) + 1;
    let mut one_shot = new_terminal(cols, rows);
    let mut bytewise = new_terminal(cols, rows);

    let _ = one_shot.feed(data);
    let _ = one_shot.finish_input();
    for byte in data {
        let _ = bytewise.feed(&[*byte]);
    }
    let _ = bytewise.finish_input();
    assert_same_history(&one_shot, &bytewise);
    assert_wide_units_atomic(&one_shot);

    let mut snapshot = snapshot_retained_units(&one_shot);
    for (index, byte) in data.iter().take(SPEC_WIDTHS.len()).enumerate() {
        let next_cols = spec_width(*byte);
        let next_rows = u16::from(data.get(index + 2).copied().unwrap_or(*byte) % 8) + 1;
        if one_shot.modes().alternate_screen {
            snapshot = snapshot_retained_units(&one_shot);
        }
        assert_eq!(
            one_shot.resize(next_cols, next_rows),
            bytewise.resize(next_cols, next_rows)
        );
        assert_same_history(&one_shot, &bytewise);
        assert_wide_units_atomic(&one_shot);
        if !one_shot.modes().alternate_screen {
            assert_surviving_anchors(&snapshot, &one_shot);
            snapshot = snapshot_retained_units(&one_shot);
        }
    }

    let evictions = data.first().copied().unwrap_or(0).min(8);
    for _ in 0..evictions {
        if one_shot.modes().alternate_screen {
            snapshot = snapshot_retained_units(&one_shot);
        }
        assert_eq!(
            one_shot.evict_oldest_primary_history_segment(),
            bytewise.evict_oldest_primary_history_segment()
        );
        assert_same_history(&one_shot, &bytewise);
        if !one_shot.modes().alternate_screen {
            assert_surviving_anchors(&snapshot, &one_shot);
            snapshot = snapshot_retained_units(&one_shot);
        }
    }
});
