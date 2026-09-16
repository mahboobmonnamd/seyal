use seyal_terminal::{
    encode_mouse_report, MouseEventKind, MouseReport, MouseReporting, TerminalState,
};

fn terminal() -> TerminalState {
    let mut terminal = TerminalState::new(80, 24).unwrap();
    let _ = terminal.take_damage();
    terminal
}

#[test]
fn mouse_modes_are_canonical_mutually_exclusive_and_queryable() {
    let mut terminal = terminal();
    assert_eq!(terminal.modes().mouse_reporting, MouseReporting::Off);
    assert!(!terminal.modes().mouse_sgr);

    terminal.feed(b"\x1b[?1000;1006h").unwrap();
    assert_eq!(terminal.modes().mouse_reporting, MouseReporting::Button);
    assert!(terminal.modes().mouse_sgr);
    terminal.feed(b"\x1b[?1002h").unwrap();
    assert_eq!(terminal.modes().mouse_reporting, MouseReporting::ButtonDrag);
    terminal.feed(b"\x1b[?1003h").unwrap();
    assert_eq!(terminal.modes().mouse_reporting, MouseReporting::Any);
    terminal.feed(b"\x1b[?1003l").unwrap();
    assert_eq!(terminal.modes().mouse_reporting, MouseReporting::Off);
    assert!(terminal.modes().mouse_sgr);

    terminal.feed(b"\x1b[?1000;1002;1006$p").unwrap();
    assert_eq!(
        terminal.take_protocol_reply().unwrap().as_bytes(),
        b"\x1b[?1000;2$y"
    );
    assert_eq!(
        terminal.take_protocol_reply().unwrap().as_bytes(),
        b"\x1b[?1002;2$y"
    );
    assert_eq!(
        terminal.take_protocol_reply().unwrap().as_bytes(),
        b"\x1b[?1006;1$y"
    );
}

#[test]
fn sgr_and_x10_reports_follow_canonical_mode() {
    let mut terminal = terminal();
    terminal.feed(b"\x1b[?1000;1006h").unwrap();
    let press = MouseReport {
        kind: MouseEventKind::Press,
        button: 0,
        shift: false,
        alt: false,
        control: false,
        col: 2,
        row: 1,
    };
    assert_eq!(
        encode_mouse_report(press, terminal.modes()).unwrap(),
        b"\x1b[<0;3;2M"
    );

    terminal.feed(b"\x1b[?1006l").unwrap();
    assert_eq!(
        encode_mouse_report(press, terminal.modes()).unwrap(),
        b"\x1b[M #\""
    );
}

#[test]
fn drag_and_any_motion_matrix() {
    let mut terminal = terminal();
    terminal.feed(b"\x1b[?1002;1006h").unwrap();
    let move_held = MouseReport {
        kind: MouseEventKind::Move,
        button: 0,
        shift: false,
        alt: false,
        control: false,
        col: 4,
        row: 4,
    };
    assert_eq!(
        encode_mouse_report(move_held, terminal.modes()).unwrap(),
        b"\x1b[<32;5;5M"
    );
    let move_idle = MouseReport {
        kind: MouseEventKind::Move,
        button: 3,
        shift: false,
        alt: false,
        control: false,
        col: 4,
        row: 4,
    };
    assert!(encode_mouse_report(move_idle, terminal.modes()).is_none());

    terminal.feed(b"\x1b[?1003h").unwrap();
    assert_eq!(
        encode_mouse_report(move_idle, terminal.modes()).unwrap(),
        b"\x1b[<35;5;5M"
    );
}

#[test]
fn wheel_encodes_sgr_buttons() {
    let mut terminal = terminal();
    terminal.feed(b"\x1b[?1000;1006h").unwrap();
    let bytes = encode_mouse_report(
        MouseReport {
            kind: MouseEventKind::Wheel,
            button: 64,
            shift: false,
            alt: false,
            control: false,
            col: 0,
            row: 0,
        },
        terminal.modes(),
    )
    .unwrap();
    assert_eq!(bytes, b"\x1b[<64;1;1M");
}
