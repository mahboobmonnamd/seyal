use seyal_terminal::{
    apply_host_mouse_gesture, encode_mouse_report, mouse_takes_host_override, MouseEventKind,
    MouseReport, MouseReporting, TerminalState, VisualPos,
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
    let release = MouseReport {
        kind: MouseEventKind::Release,
        ..press
    };
    assert_eq!(
        encode_mouse_report(release, terminal.modes()).unwrap(),
        b"\x1b[M##\""
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

#[test]
fn shift_press_latches_host_and_suppresses_report() {
    let mut anchor = None;
    let cell = VisualPos { col: 1, row: 1 };
    assert!(mouse_takes_host_override(
        MouseReporting::Button,
        true,
        anchor,
        MouseEventKind::Press
    ));
    assert_eq!(
        apply_host_mouse_gesture(&mut anchor, MouseEventKind::Press, cell),
        Some((cell, cell))
    );
    assert_eq!(anchor, Some(cell));
    // Application encoding must not run on the host path; prove Button+Press
    // would otherwise encode so the helper is what suppresses the report.
    let would_encode = encode_mouse_report(
        MouseReport {
            kind: MouseEventKind::Press,
            button: 0,
            shift: false,
            alt: false,
            control: false,
            col: 1,
            row: 1,
        },
        mode_button_sgr(),
    );
    assert!(would_encode.is_some());
}

#[test]
fn unshifted_move_release_while_latched_stay_host_no_report() {
    let mut anchor = Some(VisualPos { col: 1, row: 1 });
    let move_cell = VisualPos { col: 3, row: 1 };
    assert!(mouse_takes_host_override(
        MouseReporting::Button,
        false,
        anchor,
        MouseEventKind::Move
    ));
    assert_eq!(
        apply_host_mouse_gesture(&mut anchor, MouseEventKind::Move, move_cell),
        Some((VisualPos { col: 1, row: 1 }, move_cell))
    );
    assert_eq!(anchor, Some(VisualPos { col: 1, row: 1 }));

    let release_cell = VisualPos { col: 4, row: 1 };
    assert!(mouse_takes_host_override(
        MouseReporting::Any,
        false,
        anchor,
        MouseEventKind::Release
    ));
    assert_eq!(
        apply_host_mouse_gesture(&mut anchor, MouseEventKind::Release, release_cell),
        Some((VisualPos { col: 1, row: 1 }, release_cell))
    );
    assert_eq!(anchor, None);
}

#[test]
fn after_release_unshifted_press_resumes_application_report() {
    let anchor = None;
    assert!(!mouse_takes_host_override(
        MouseReporting::Button,
        false,
        anchor,
        MouseEventKind::Press
    ));
    let bytes = encode_mouse_report(
        MouseReport {
            kind: MouseEventKind::Press,
            button: 0,
            shift: false,
            alt: false,
            control: false,
            col: 0,
            row: 0,
        },
        mode_button_sgr(),
    )
    .unwrap();
    assert_eq!(bytes, b"\x1b[<0;1;1M");
}

#[test]
fn reporting_off_press_is_host_without_shift() {
    let mut anchor = None;
    let cell = VisualPos { col: 2, row: 2 };
    assert!(mouse_takes_host_override(
        MouseReporting::Off,
        false,
        anchor,
        MouseEventKind::Press
    ));
    assert_eq!(
        apply_host_mouse_gesture(&mut anchor, MouseEventKind::Press, cell),
        Some((cell, cell))
    );
}

#[test]
fn shift_wheel_is_host_noop_no_selection_change() {
    let mut anchor = None;
    assert!(mouse_takes_host_override(
        MouseReporting::Any,
        true,
        anchor,
        MouseEventKind::Wheel
    ));
    assert_eq!(
        apply_host_mouse_gesture(
            &mut anchor,
            MouseEventKind::Wheel,
            VisualPos { col: 5, row: 5 }
        ),
        None
    );
    assert_eq!(anchor, None);
}

#[test]
fn host_latched_move_ignores_reporting_any() {
    let anchor = Some(VisualPos { col: 0, row: 0 });
    assert!(mouse_takes_host_override(
        MouseReporting::Any,
        false,
        anchor,
        MouseEventKind::Move
    ));
}

fn mode_button_sgr() -> seyal_terminal::ModeState {
    seyal_terminal::ModeState {
        mouse_reporting: MouseReporting::Button,
        mouse_sgr: true,
        ..seyal_terminal::ModeState::default()
    }
}
