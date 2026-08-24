use seyal_terminal::{Color, TerminalState};

fn terminal(cols: u16, rows: u16) -> TerminalState {
    let mut terminal = TerminalState::new(cols, rows).expect("valid terminal");
    let _ = terminal.take_damage();
    terminal
}

#[test]
fn full_screen_scroll_blanks_new_row_with_active_background() {
    let mut terminal = terminal(4, 2);

    terminal.feed(b"\x1b[41mA\r\nB\r\n");

    for col in 0..terminal.cols() {
        let cell = terminal.cell(col, 1).expect("bottom row cell");
        assert_eq!(cell.character, ' ');
        assert_eq!(cell.style.bg, Color::Indexed(1));
        assert!(!cell.style.bold);
        assert!(!cell.style.underline);
        assert!(!cell.style.inverse);
    }
}

#[test]
fn alternate_screen_inherits_saved_rendition_and_restores_primary_rendition() {
    let mut terminal = terminal(6, 2);

    terminal.feed(b"\x1b[44mP\x1b[?1049hA");
    let alternate = terminal.cell(0, 0).expect("alternate cell");
    assert_eq!(alternate.character, 'A');
    assert_eq!(alternate.style.bg, Color::Indexed(4));
    assert_eq!(
        terminal.cell(1, 0).expect("alternate blank").style.bg,
        Color::Indexed(4)
    );

    terminal.feed(b"\x1b[41mR\x1b[?1049lQ");
    let primary = terminal.cell(1, 0).expect("primary continuation");
    assert_eq!(primary.character, 'Q');
    assert_eq!(primary.style.bg, Color::Indexed(4));
}
