use seyal_terminal::TerminalState;

fn terminal() -> TerminalState {
    let mut terminal = TerminalState::new(80, 24).unwrap();
    let _ = terminal.take_damage();
    terminal
}

#[test]
fn dec_cursor_and_keypad_modes_are_canonical_and_queryable() {
    let mut terminal = terminal();
    assert!(!terminal.modes().application_cursor);
    assert!(!terminal.modes().application_keypad);

    terminal.feed(b"\x1b[?1;66h").unwrap();
    assert!(terminal.modes().application_cursor);
    assert!(terminal.modes().application_keypad);
    terminal.feed(b"\x1b[?1;66$p").unwrap();
    assert_eq!(
        terminal.take_protocol_reply().unwrap().as_bytes(),
        b"\x1b[?1;1$y"
    );
    assert_eq!(
        terminal.take_protocol_reply().unwrap().as_bytes(),
        b"\x1b[?66;1$y"
    );

    terminal.feed(b"\x1b[?1;66l\x1b=\x1b>").unwrap();
    assert!(!terminal.modes().application_cursor);
    assert!(!terminal.modes().application_keypad);
}

#[test]
fn kitty_flags_are_masked_bounded_and_screen_local() {
    let mut terminal = terminal();
    terminal.feed(b"\x1b[=31;1u\x1b[?u").unwrap();
    assert_eq!(terminal.modes().keyboard_flags, 3);
    assert_eq!(
        terminal.take_protocol_reply().unwrap().as_bytes(),
        b"\x1b[?3u"
    );

    terminal.feed(b"\x1b[>1u\x1b[>2u\x1b[<1u").unwrap();
    assert_eq!(terminal.modes().keyboard_flags, 1);
    terminal.feed(b"\x1b[<65535u").unwrap();
    assert_eq!(terminal.modes().keyboard_flags, 0);

    terminal.feed(b"\x1b[=3;1u\x1b[?1049h").unwrap();
    assert_eq!(terminal.modes().keyboard_flags, 0);
    terminal.feed(b"\x1b[=1;1u\x1b[?1049l").unwrap();
    assert_eq!(terminal.modes().keyboard_flags, 3);
}

#[test]
fn unsupported_keyboard_controls_do_not_mutate_or_print() {
    let mut terminal = terminal();
    terminal.feed(b"\x1b[=1;0uX\x1bcY").unwrap();
    assert_eq!(terminal.modes().keyboard_flags, 0);
    assert_eq!(terminal.row_text(0).unwrap().chars().next(), Some('X'));
    assert_eq!(terminal.diagnostics().deferred_sequences, 1);
    assert_eq!(terminal.diagnostics().unknown_sequences, 1);
}
