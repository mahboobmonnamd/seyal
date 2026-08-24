use std::collections::HashSet;

use seyal_terminal::{Color, Style, TerminalError, TerminalState};

fn terminal(cols: u16, rows: u16) -> TerminalState {
    let mut terminal = TerminalState::new(cols, rows).expect("valid terminal");
    let _ = terminal.take_damage();
    terminal
}

fn feed(terminal: &mut TerminalState, bytes: &[u8]) {
    terminal.feed(bytes).expect("terminal feed succeeds");
}

fn row(terminal: &TerminalState, row: u16) -> String {
    terminal.row_text(row).expect("row exists")
}

#[test]
fn printable_utf8_survives_arbitrary_chunking() {
    let mut terminal = terminal(8, 2);
    feed(&mut terminal, b"A");
    feed(&mut terminal, &[0xe2]);
    feed(&mut terminal, &[0x82]);
    feed(&mut terminal, &[0xac]);
    feed(&mut terminal, b"Z");

    assert_eq!(terminal.cell(0, 0).unwrap().character, 'A');
    assert_eq!(terminal.cell(1, 0).unwrap().character, '€');
    assert_eq!(terminal.cell(2, 0).unwrap().character, 'Z');
    assert_eq!(terminal.diagnostics().malformed_sequences, 0);
}

#[test]
fn malformed_utf8_recovers_and_reprocesses_following_ascii() {
    let mut terminal = terminal(8, 2);
    feed(&mut terminal, &[0xf0, b'A']);

    assert_eq!(terminal.cell(0, 0).unwrap().character, '\u{fffd}');
    assert_eq!(terminal.cell(1, 0).unwrap().character, 'A');
    assert_eq!(terminal.diagnostics().malformed_sequences, 1);
}

#[test]
fn controls_apply_without_parser_state_duplication() {
    let mut terminal = terminal(16, 3);
    feed(&mut terminal, b"ab\rZ");
    assert_eq!(&row(&terminal, 0)[..2], "Zb");

    feed(&mut terminal, b"\r\na\tb");
    assert_eq!(terminal.cell(0, 1).unwrap().character, 'a');
    assert_eq!(terminal.cell(8, 1).unwrap().character, 'b');

    feed(&mut terminal, b"\x08X");
    assert_eq!(terminal.cell(8, 1).unwrap().character, 'X');
}

#[test]
fn relative_and_absolute_cursor_movement_clamps_to_grid() {
    let mut terminal = terminal(6, 4);
    feed(&mut terminal, b"\x1b[3;4HX");
    assert_eq!(terminal.cell(3, 2).unwrap().character, 'X');

    feed(&mut terminal, b"\x1b[99A\x1b[99DZ");
    assert_eq!(terminal.cell(0, 0).unwrap().character, 'Z');

    feed(&mut terminal, b"\x1b[4d\x1b[6GY");
    assert_eq!(terminal.cell(5, 3).unwrap().character, 'Y');
}

#[test]
fn erase_line_and_display_support_m001_modes() {
    let mut terminal = terminal(5, 2);
    feed(&mut terminal, b"abcde\r\n12345");
    feed(&mut terminal, b"\x1b[2;3H\x1b[0K");
    assert_eq!(row(&terminal, 1), "12   ");

    feed(&mut terminal, b"\x1b[1;3H\x1b[1K");
    assert_eq!(row(&terminal, 0), "   de");

    feed(&mut terminal, b"\x1b[2J");
    assert_eq!(row(&terminal, 0), "     ");
    assert_eq!(row(&terminal, 1), "     ");
}

#[test]
fn sgr_tracks_supported_attributes_and_colors() {
    let mut terminal = terminal(8, 2);
    feed(&mut terminal, b"\x1b[1;4;7;31;48;5;200mX");
    let first = terminal.cell(0, 0).unwrap();
    assert_eq!(
        first.style,
        Style {
            fg: Color::Indexed(1),
            bg: Color::Indexed(200),
            bold: true,
            underline: true,
            inverse: true,
        }
    );

    feed(&mut terminal, b"\x1b[0;38;2;10;20;30;49mY");
    let second = terminal.cell(1, 0).unwrap();
    assert_eq!(
        second.style.fg,
        Color::Rgb {
            r: 10,
            g: 20,
            b: 30
        }
    );
    assert_eq!(second.style.bg, Color::Default);
    assert!(!second.style.bold);
    assert!(!second.style.underline);
    assert!(!second.style.inverse);
}

#[test]
fn deferred_sgr_does_not_corrupt_following_supported_state() {
    let mut terminal = terminal(8, 2);
    feed(&mut terminal, b"\x1b[2m\x1b[31mX");

    assert_eq!(terminal.cell(0, 0).unwrap().style.fg, Color::Indexed(1));
    assert!(terminal.diagnostics().deferred_sequences >= 1);
}

#[test]
fn cursor_visibility_is_runtime_authoritative_mode_state() {
    let mut terminal = terminal(8, 2);
    feed(&mut terminal, b"\x1b[?25l");
    assert!(!terminal.cursor().visible);
    assert!(!terminal.modes().cursor_visible);

    feed(&mut terminal, b"\x1b[?25h");
    assert!(terminal.cursor().visible);
    assert!(terminal.modes().cursor_visible);
}

#[test]
fn save_restore_cursor_supports_csi_and_dec_forms() {
    let mut terminal = terminal(8, 4);
    feed(&mut terminal, b"\x1b[3;4H\x1b[s\x1b[1;1H\x1b[uX");
    assert_eq!(terminal.cell(3, 2).unwrap().character, 'X');

    feed(&mut terminal, b"\x1b[2;2H\x1b7\x1b[4;8H\x1b8Y");
    assert_eq!(terminal.cell(1, 1).unwrap().character, 'Y');
}

#[test]
fn alternate_screen_preserves_primary_and_is_discarded_on_leave() {
    let mut terminal = terminal(6, 3);
    feed(&mut terminal, b"primary");
    let primary_line = terminal.line_id(0).unwrap();

    feed(&mut terminal, b"\x1b[?1049hALT");
    assert!(terminal.modes().alternate_screen);
    assert_eq!(&row(&terminal, 0)[..3], "ALT");
    assert_ne!(terminal.line_id(0).unwrap(), primary_line);

    feed(&mut terminal, b"\x1b[?1049l");
    assert!(!terminal.modes().alternate_screen);
    assert_eq!(&row(&terminal, 0)[..6], "primar");
    assert_eq!(terminal.line_id(0).unwrap(), primary_line);
}

#[test]
fn resize_preserves_retained_cells_and_line_identity() {
    let mut terminal = terminal(4, 2);
    feed(&mut terminal, b"abc");
    let line = terminal.line_id(0).unwrap();

    terminal.resize(6, 3).unwrap();
    assert_eq!(terminal.cols(), 6);
    assert_eq!(terminal.rows(), 3);
    assert_eq!(&row(&terminal, 0)[..3], "abc");
    assert_eq!(terminal.line_id(0).unwrap(), line);

    feed(&mut terminal, b"\x1b[3;6H");
    terminal.resize(2, 1).unwrap();
    assert_eq!(terminal.cursor().col, 1);
    assert_eq!(terminal.cursor().row, 0);
    assert_eq!(terminal.line_id(0).unwrap(), line);
}

#[test]
fn zero_dimensions_are_rejected_without_partial_resize() {
    assert!(matches!(
        TerminalState::new(0, 10),
        Err(TerminalError::InvalidSize)
    ));

    let mut terminal = terminal(4, 2);
    assert_eq!(terminal.resize(0, 3), Err(TerminalError::InvalidSize));
    assert_eq!((terminal.cols(), terminal.rows()), (4, 2));
}

#[test]
fn line_feed_scrolls_full_screen_and_advances_line_identity() {
    let mut terminal = terminal(4, 2);
    let initial_second = terminal.line_id(1).unwrap();
    feed(&mut terminal, b"a\r\nb\r\nc");

    assert_eq!(terminal.line_id(0).unwrap(), initial_second);
    assert_ne!(terminal.line_id(1).unwrap(), initial_second);
    assert_eq!(terminal.cell(0, 0).unwrap().character, 'b');
    assert_eq!(terminal.cell(0, 1).unwrap().character, 'c');
}

#[test]
fn allocated_line_ids_do_not_repeat_across_scroll_resize_and_alternate_lifetimes() {
    let mut terminal = terminal(4, 2);
    let mut seen = HashSet::new();
    for row in 0..terminal.rows() {
        assert!(seen.insert(terminal.line_id(row).unwrap()));
    }

    feed(&mut terminal, b"\x1b[2;1H");
    for _ in 0..128 {
        feed(&mut terminal, b"\r\n");
        assert!(seen.insert(terminal.line_id(terminal.rows() - 1).unwrap()));
    }

    let old_rows = terminal.rows();
    terminal.resize(4, 5).expect("grow resize succeeds");
    for row in old_rows..terminal.rows() {
        assert!(seen.insert(terminal.line_id(row).unwrap()));
    }

    for _ in 0..16 {
        feed(&mut terminal, b"\x1b[?1049h");
        for row in 0..terminal.rows() {
            assert!(seen.insert(terminal.line_id(row).unwrap()));
        }
        feed(&mut terminal, b"\x1b[?1049l");
    }
}

#[test]
fn deferred_and_unknown_sequences_leave_parser_continuity_intact() {
    let mut terminal = terminal(8, 2);
    feed(&mut terminal, b"\x1b[3@A");
    feed(&mut terminal, b"\x1b]0;title\x07B");
    feed(&mut terminal, b"\x1b[?9999hC");
    feed(&mut terminal, b"\x1b[999zD");

    assert_eq!(&row(&terminal, 0)[..4], "ABCD");
    assert!(terminal.diagnostics().deferred_sequences >= 3);
    assert!(terminal.diagnostics().unknown_sequences >= 1);
}

#[test]
fn truncated_input_is_held_across_feed_and_reported_only_on_finish() {
    let mut terminal = terminal(8, 2);
    feed(&mut terminal, &[0xe2, 0x82]);
    assert_eq!(terminal.diagnostics().malformed_sequences, 0);

    terminal.finish_input().expect("finish input succeeds");
    assert_eq!(terminal.cell(0, 0).unwrap().character, '\u{fffd}');
    assert!(terminal.diagnostics().malformed_sequences >= 1);
}

#[test]
fn damage_generations_are_monotonic_and_coalesce_until_consumed() {
    let mut terminal = terminal(8, 3);
    let initial = terminal.damage_generation();
    feed(&mut terminal, b"A");
    let after_text = terminal.damage_generation();
    feed(&mut terminal, b"\x1b[3;1H");
    let after_cursor = terminal.damage_generation();

    assert!(after_text > initial);
    assert!(after_cursor > after_text);
    let damage = terminal.take_damage().unwrap();
    assert_eq!(damage.generation, after_cursor);
    assert_eq!(damage.first_row, 0);
    assert_eq!(damage.last_row, 2);
    assert!(terminal.take_damage().is_none());
}

#[test]
fn one_shot_and_bytewise_feeds_produce_the_same_canonical_state() {
    let bytes = b"hello\r\n\x1b[31mred\x1b[0m \xe2\x82\xac\x1b[2;2H!\x1b[?25l";
    let mut one_shot = terminal(12, 4);
    feed(&mut one_shot, bytes);

    let mut bytewise = terminal(12, 4);
    for byte in bytes {
        feed(&mut bytewise, &[*byte]);
    }

    for row_index in 0..4 {
        assert_eq!(row(&one_shot, row_index), row(&bytewise, row_index));
        assert_eq!(one_shot.line_id(row_index), bytewise.line_id(row_index));
    }
    assert_eq!(one_shot.cursor(), bytewise.cursor());
    assert_eq!(one_shot.modes(), bytewise.modes());
    assert_eq!(one_shot.diagnostics(), bytewise.diagnostics());
}
