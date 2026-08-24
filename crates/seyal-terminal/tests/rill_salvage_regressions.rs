use seyal_terminal::{Color, Style, TerminalState};

fn terminal(cols: u16, rows: u16) -> TerminalState {
    let mut terminal = TerminalState::new(cols, rows).expect("valid terminal");
    let _ = terminal.take_damage();
    terminal
}

#[test]
fn full_screen_scroll_blanks_new_row_with_active_background() {
    let mut terminal = terminal(4, 2);

    terminal
        .feed(b"\x1b[41mA\r\nB\r\n")
        .expect("scroll feed succeeds");

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
fn alternate_screen_inherits_full_pen_but_blank_cells_keep_background_only() {
    let mut terminal = terminal(8, 2);
    let inherited = Style {
        fg: Color::Indexed(1),
        bg: Color::Indexed(4),
        bold: true,
        underline: true,
        inverse: true,
    };

    terminal
        .feed(b"\x1b[1;4;7;31;44mP\x1b[?1049hA")
        .expect("alternate enter feed succeeds");

    let alternate = terminal.cell(0, 0).expect("alternate printed cell");
    assert_eq!(alternate.character, 'A');
    assert_eq!(alternate.style, inherited);

    let blank = terminal.cell(1, 0).expect("alternate blank cell");
    assert_eq!(blank.character, ' ');
    assert_eq!(blank.style.bg, Color::Indexed(4));
    assert_eq!(blank.style.fg, Color::Default);
    assert!(!blank.style.bold);
    assert!(!blank.style.underline);
    assert!(!blank.style.inverse);

    terminal
        .feed(b"\x1b[0;32;41mR\x1b[?1049lQ")
        .expect("alternate leave feed succeeds");

    let primary = terminal.cell(1, 0).expect("primary continuation");
    assert_eq!(primary.character, 'Q');
    assert_eq!(primary.style, inherited);
}

#[test]
fn m001_1049_does_not_overwrite_existing_primary_cursor_save_slot() {
    let mut terminal = terminal(8, 4);

    terminal
        .feed(b"\x1b[31m\x1b[2;2H\x1b7\x1b[44m\x1b[3;3H\x1b[?1049hALT\x1b[?1049l\x1b8X")
        .expect("save-slot fixture succeeds");

    let restored = terminal.cell(1, 1).expect("restored primary cell");
    assert_eq!(restored.character, 'X');
    assert_eq!(restored.style.fg, Color::Indexed(1));
    assert_eq!(restored.style.bg, Color::Default);
}
