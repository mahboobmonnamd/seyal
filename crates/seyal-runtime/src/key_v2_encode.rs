//! SPEC-006 §21.2 TerminalKeyV2 PTY encoding.
//!
//! Kept outside the macOS-only local IPC Runtime so the encoder table can
//! execute on every host. Admission, attach, and PTY submit stay in
//! `runtime::local::ingress`.

#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

use seyal_exec::ModeState;
use seyal_protocol::framing::{TerminalKeyV2, TerminalKeyV2Event, TerminalKeyV2Kind};

fn push_decimal(out: &mut Vec<u8>, mut value: u32) {
    let mut digits = [0u8; 10];
    let mut len = 0;
    loop {
        digits[len] = b'0' + (value % 10) as u8;
        len += 1;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    out.extend(digits[..len].iter().rev());
}

fn csi_u(code: u32, modifiers: u16, event: Option<TerminalKeyV2Event>) -> Vec<u8> {
    let mut out = b"\x1b[".to_vec();
    push_decimal(&mut out, code);
    out.push(b';');
    push_decimal(&mut out, u32::from(1 + modifiers));
    if let Some(event) = event
        && event != TerminalKeyV2Event::Press
    {
        out.push(b':');
        push_decimal(&mut out, event as u32);
    }
    out.push(b'u');
    out
}

fn kitty_keypad_code(value: u32) -> u32 {
    match value {
        15 => 57415, // keypad Equal
        16 => 57414, // keypad Enter
        value => 57399 + value,
    }
}

fn csi_mod(code: u32, final_byte: u8, modifiers: u16) -> Vec<u8> {
    let mut out = b"\x1b[".to_vec();
    push_decimal(&mut out, code);
    out.push(b';');
    push_decimal(&mut out, u32::from(1 + modifiers));
    out.push(final_byte);
    out
}

fn csi_mod_event(
    code: u32,
    final_byte: u8,
    modifiers: u16,
    event: Option<TerminalKeyV2Event>,
) -> Vec<u8> {
    let mut out = csi_mod(code, final_byte, modifiers);
    if let Some(event) = event.filter(|event| *event != TerminalKeyV2Event::Press) {
        let final_byte = out.pop().expect("CSI modifier output has a final byte");
        out.push(b':');
        push_decimal(&mut out, event as u32);
        out.push(final_byte);
    }
    out
}

fn control_byte(scalar: u32) -> Option<u8> {
    Some(match scalar as u8 {
        b' ' | b'@' => 0,
        b'a'..=b'z' => scalar as u8 - b'a' + 1,
        b'A'..=b'Z' => scalar as u8 - b'A' + 1,
        b'[' => 0x1b,
        b'\\' => 0x1c,
        b']' => 0x1d,
        b'^' => 0x1e,
        b'_' => 0x1f,
        b'?' => 0x7f,
        _ => return None,
    })
}

pub(crate) fn encode_terminal_key_v2(key: TerminalKeyV2, modes: ModeState) -> Result<Vec<u8>, ()> {
    let modifiers = key.modifiers.bits();
    let flags = modes.keyboard_flags;
    let event = if flags & 2 != 0 {
        Some(key.event)
    } else {
        None
    };
    if flags & 1 == 0 && key.kind == TerminalKeyV2Kind::Keypad && modifiers != 0 {
        return Err(());
    }
    if flags & 2 == 0 && key.event == TerminalKeyV2Event::Release {
        return Ok(Vec::new());
    }
    if matches!(
        key.kind,
        TerminalKeyV2Kind::Enter | TerminalKeyV2Kind::Tab | TerminalKeyV2Kind::Backspace
    ) {
        let mut out = match key.kind {
            TerminalKeyV2Kind::Enter => vec![0x0d],
            TerminalKeyV2Kind::Tab => vec![0x09],
            TerminalKeyV2Kind::Backspace => vec![0x7f],
            _ => unreachable!(),
        };
        if key.kind == TerminalKeyV2Kind::Tab && modifiers & 1 != 0 {
            out = b"\x1b[Z".to_vec();
        }
        if modifiers & 4 != 0 && key.kind == TerminalKeyV2Kind::Backspace {
            out = vec![0x08];
        }
        if modifiers & 2 != 0 {
            let mut prefixed = vec![0x1b];
            prefixed.extend(out);
            out = prefixed;
        }
        return Ok(if key.event == TerminalKeyV2Event::Release {
            Vec::new()
        } else {
            out
        });
    }
    if flags & 1 != 0 {
        let code = match key.kind {
            TerminalKeyV2Kind::Escape => 27,
            TerminalKeyV2Kind::Ascii => key.value,
            TerminalKeyV2Kind::Keypad => kitty_keypad_code(key.value),
            TerminalKeyV2Kind::Home => 1,
            TerminalKeyV2Kind::End => 1,
            TerminalKeyV2Kind::Insert => 2,
            TerminalKeyV2Kind::Delete => 3,
            TerminalKeyV2Kind::PageUp => 5,
            TerminalKeyV2Kind::PageDown => 6,
            TerminalKeyV2Kind::Function => {
                if key.value == 3 {
                    13
                } else {
                    0
                }
            }
            TerminalKeyV2Kind::ArrowUp
            | TerminalKeyV2Kind::ArrowDown
            | TerminalKeyV2Kind::ArrowRight
            | TerminalKeyV2Kind::ArrowLeft => 1,
            _ => 0,
        };
        if key.kind == TerminalKeyV2Kind::Function && key.value <= 4 && key.value != 3 {
            let final_byte = b'P' + (key.value as u8 - 1);
            return Ok(csi_mod_event(1, final_byte, modifiers, event));
        }
        if matches!(
            key.kind,
            TerminalKeyV2Kind::Home
                | TerminalKeyV2Kind::End
                | TerminalKeyV2Kind::ArrowUp
                | TerminalKeyV2Kind::ArrowDown
                | TerminalKeyV2Kind::ArrowRight
                | TerminalKeyV2Kind::ArrowLeft
        ) {
            let final_byte = match key.kind {
                TerminalKeyV2Kind::Home => b'H',
                TerminalKeyV2Kind::End => b'F',
                TerminalKeyV2Kind::ArrowUp => b'A',
                TerminalKeyV2Kind::ArrowDown => b'B',
                TerminalKeyV2Kind::ArrowRight => b'C',
                _ => b'D',
            };
            return Ok(csi_mod_event(code, final_byte, modifiers, event));
        }
        if key.kind == TerminalKeyV2Kind::Function && key.value == 3 {
            return Ok(csi_mod_event(13, b'~', modifiers, event));
        }
        if key.kind == TerminalKeyV2Kind::Function {
            let code = match key.value {
                5 => 15,
                6 => 17,
                7 => 18,
                8 => 19,
                9 => 20,
                10 => 21,
                11 => 23,
                _ => 24,
            };
            return Ok(csi_mod_event(code, b'~', modifiers, event));
        }
        if matches!(
            key.kind,
            TerminalKeyV2Kind::Insert
                | TerminalKeyV2Kind::Delete
                | TerminalKeyV2Kind::PageUp
                | TerminalKeyV2Kind::PageDown
        ) {
            return Ok(csi_mod_event(code, b'~', modifiers, event));
        }
        return Ok(csi_u(code, modifiers, event));
    }
    if flags & 2 != 0 && key.event != TerminalKeyV2Event::Press {
        let encoded = match key.kind {
            TerminalKeyV2Kind::ArrowUp => Some(csi_mod_event(1, b'A', modifiers, Some(key.event))),
            TerminalKeyV2Kind::ArrowDown => {
                Some(csi_mod_event(1, b'B', modifiers, Some(key.event)))
            }
            TerminalKeyV2Kind::ArrowRight => {
                Some(csi_mod_event(1, b'C', modifiers, Some(key.event)))
            }
            TerminalKeyV2Kind::ArrowLeft => {
                Some(csi_mod_event(1, b'D', modifiers, Some(key.event)))
            }
            TerminalKeyV2Kind::Home => Some(csi_mod_event(1, b'H', modifiers, Some(key.event))),
            TerminalKeyV2Kind::End => Some(csi_mod_event(1, b'F', modifiers, Some(key.event))),
            TerminalKeyV2Kind::Insert => Some(csi_mod_event(2, b'~', modifiers, Some(key.event))),
            TerminalKeyV2Kind::Delete => Some(csi_mod_event(3, b'~', modifiers, Some(key.event))),
            TerminalKeyV2Kind::PageUp => Some(csi_mod_event(5, b'~', modifiers, Some(key.event))),
            TerminalKeyV2Kind::PageDown => Some(csi_mod_event(6, b'~', modifiers, Some(key.event))),
            TerminalKeyV2Kind::Function => Some(if key.value <= 4 && key.value != 3 {
                csi_mod_event(1, b'P' + key.value as u8 - 1, modifiers, Some(key.event))
            } else if key.value == 3 {
                csi_mod_event(13, b'~', modifiers, Some(key.event))
            } else {
                csi_mod_event(
                    match key.value {
                        5 => 15,
                        6 => 17,
                        7 => 18,
                        8 => 19,
                        9 => 20,
                        10 => 21,
                        11 => 23,
                        _ => 24,
                    },
                    b'~',
                    modifiers,
                    Some(key.event),
                )
            }),
            _ => None,
        };
        if let Some(encoded) = encoded {
            return Ok(encoded);
        }
    }
    if flags & 2 != 0
        && modes.application_keypad
        && key.kind == TerminalKeyV2Kind::Keypad
        && key.event != TerminalKeyV2Event::Press
    {
        return Ok(csi_u(
            kitty_keypad_code(key.value),
            modifiers,
            Some(key.event),
        ));
    }
    if key.event == TerminalKeyV2Event::Release {
        return Ok(Vec::new());
    }
    if modifiers != 0 && key.kind != TerminalKeyV2Kind::Ascii {
        let modified = match key.kind {
            TerminalKeyV2Kind::ArrowUp => csi_mod(1, b'A', modifiers),
            TerminalKeyV2Kind::ArrowDown => csi_mod(1, b'B', modifiers),
            TerminalKeyV2Kind::ArrowRight => csi_mod(1, b'C', modifiers),
            TerminalKeyV2Kind::ArrowLeft => csi_mod(1, b'D', modifiers),
            TerminalKeyV2Kind::Home => csi_mod(1, b'H', modifiers),
            TerminalKeyV2Kind::End => csi_mod(1, b'F', modifiers),
            TerminalKeyV2Kind::Insert => csi_mod(2, b'~', modifiers),
            TerminalKeyV2Kind::Delete => csi_mod(3, b'~', modifiers),
            TerminalKeyV2Kind::PageUp => csi_mod(5, b'~', modifiers),
            TerminalKeyV2Kind::PageDown => csi_mod(6, b'~', modifiers),
            TerminalKeyV2Kind::Function => {
                if key.value <= 4 && key.value != 3 {
                    csi_mod(1, b'P' + key.value as u8 - 1, modifiers)
                } else if key.value == 3 {
                    csi_mod(13, b'~', modifiers)
                } else {
                    csi_mod(
                        match key.value {
                            5 => 15,
                            6 => 17,
                            7 => 18,
                            8 => 19,
                            9 => 20,
                            10 => 21,
                            11 => 23,
                            _ => 24,
                        },
                        b'~',
                        modifiers,
                    )
                }
            }
            TerminalKeyV2Kind::Keypad => return Err(()),
            TerminalKeyV2Kind::Escape => {
                if modifiers & 2 != 0 {
                    vec![0x1b, 0x1b]
                } else {
                    vec![0x1b]
                }
            }
            _ => return Err(()),
        };
        return Ok(modified);
    }
    let mut out = match key.kind {
        TerminalKeyV2Kind::Escape => vec![0x1b],
        TerminalKeyV2Kind::ArrowUp => {
            if modes.application_cursor {
                b"\x1bOA".to_vec()
            } else {
                b"\x1b[A".to_vec()
            }
        }
        TerminalKeyV2Kind::ArrowDown => {
            if modes.application_cursor {
                b"\x1bOB".to_vec()
            } else {
                b"\x1b[B".to_vec()
            }
        }
        TerminalKeyV2Kind::ArrowRight => {
            if modes.application_cursor {
                b"\x1bOC".to_vec()
            } else {
                b"\x1b[C".to_vec()
            }
        }
        TerminalKeyV2Kind::ArrowLeft => {
            if modes.application_cursor {
                b"\x1bOD".to_vec()
            } else {
                b"\x1b[D".to_vec()
            }
        }
        TerminalKeyV2Kind::Ascii => {
            if modifiers & 4 != 0 {
                vec![control_byte(key.value).ok_or(())?]
            } else if modifiers & 2 != 0 {
                vec![if modifiers & 1 != 0 {
                    key.shifted_ascii as u8
                } else {
                    key.value as u8
                }]
            } else {
                return Err(());
            }
        }
        TerminalKeyV2Kind::Home => {
            if modes.application_cursor {
                b"\x1bOH".to_vec()
            } else {
                b"\x1b[H".to_vec()
            }
        }
        TerminalKeyV2Kind::End => {
            if modes.application_cursor {
                b"\x1bOF".to_vec()
            } else {
                b"\x1b[F".to_vec()
            }
        }
        TerminalKeyV2Kind::Insert => b"\x1b[2~".to_vec(),
        TerminalKeyV2Kind::Delete => b"\x1b[3~".to_vec(),
        TerminalKeyV2Kind::PageUp => b"\x1b[5~".to_vec(),
        TerminalKeyV2Kind::PageDown => b"\x1b[6~".to_vec(),
        TerminalKeyV2Kind::Function => match key.value {
            1 => b"\x1bOP".to_vec(),
            2 => b"\x1bOQ".to_vec(),
            3 => b"\x1bOR".to_vec(),
            4 => b"\x1bOS".to_vec(),
            5 => b"\x1b[15~".to_vec(),
            6 => b"\x1b[17~".to_vec(),
            7 => b"\x1b[18~".to_vec(),
            8 => b"\x1b[19~".to_vec(),
            9 => b"\x1b[20~".to_vec(),
            10 => b"\x1b[21~".to_vec(),
            11 => b"\x1b[23~".to_vec(),
            _ => b"\x1b[24~".to_vec(),
        },
        TerminalKeyV2Kind::Keypad => {
            if !modes.application_keypad {
                match key.value {
                    0..=9 => vec![b'0' + key.value as u8],
                    10 => vec![b'.'],
                    11 => vec![b'/'],
                    12 => vec![b'*'],
                    13 => vec![b'-'],
                    14 => vec![b'+'],
                    15 => vec![b'='],
                    16 => vec![0x0d],
                    _ => return Err(()),
                }
            } else {
                match key.value {
                    0..=9 => vec![0x1b, b'O', b'p' + key.value as u8],
                    10 => b"\x1bOn".to_vec(),
                    11 => b"\x1bOo".to_vec(),
                    12 => b"\x1bOj".to_vec(),
                    13 => b"\x1bOm".to_vec(),
                    14 => b"\x1bOk".to_vec(),
                    15 => b"\x1bOX".to_vec(),
                    16 => b"\x1bOM".to_vec(),
                    _ => return Err(()),
                }
            }
        }
        _ => return Err(()),
    };
    if modifiers & 2 != 0 {
        let mut prefixed = vec![0x1b];
        prefixed.extend(out);
        out = prefixed;
    }
    Ok(out)
}

#[cfg(test)]
#[path = "key_v2_section_21_6_tests.rs"]
mod section_21_6_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use seyal_protocol::framing::{TerminalKeyV2Event, TerminalKeyV2Kind, TerminalKeyV2Modifiers};

    #[test]
    fn v2_navigation_and_kitty_events_are_bounded_and_deterministic() {
        let key = TerminalKeyV2 {
            attachment_id: crate::AttachmentId::from_bytes([0; 16]),
            kind: TerminalKeyV2Kind::ArrowUp,
            modifiers: TerminalKeyV2Modifiers::CONTROL,
            value: 0,
            event: TerminalKeyV2Event::Repeat,
            shifted_ascii: 0,
            action_id: 1,
        };
        let modes = ModeState {
            keyboard_flags: 3,
            ..ModeState::default()
        };
        assert_eq!(encode_terminal_key_v2(key, modes).unwrap(), b"\x1b[1;5:2A");
        assert!(
            encode_terminal_key_v2(
                TerminalKeyV2 {
                    event: TerminalKeyV2Event::Release,
                    ..key
                },
                modes
            )
            .unwrap()
            .len()
                <= 64
        );
    }

    #[test]
    fn v2_cursor_and_keypad_modes_select_canonical_bytes() {
        let base = TerminalKeyV2 {
            attachment_id: crate::AttachmentId::from_bytes([0; 16]),
            kind: TerminalKeyV2Kind::ArrowLeft,
            modifiers: TerminalKeyV2Modifiers::NONE,
            value: 0,
            event: TerminalKeyV2Event::Press,
            shifted_ascii: 0,
            action_id: 1,
        };
        assert_eq!(
            encode_terminal_key_v2(base, ModeState::default()).unwrap(),
            b"\x1b[D"
        );
        assert_eq!(
            encode_terminal_key_v2(
                base,
                ModeState {
                    application_cursor: true,
                    ..ModeState::default()
                }
            )
            .unwrap(),
            b"\x1bOD"
        );
        let keypad = TerminalKeyV2 {
            kind: TerminalKeyV2Kind::Keypad,
            value: 4,
            ..base
        };
        assert_eq!(
            encode_terminal_key_v2(keypad, ModeState::default()).unwrap(),
            b"4"
        );
        assert_eq!(
            encode_terminal_key_v2(
                keypad,
                ModeState {
                    application_keypad: true,
                    ..ModeState::default()
                }
            )
            .unwrap(),
            b"\x1bOt"
        );
    }

    #[test]
    fn v2_legacy_ascii_requires_alt_or_control_and_preserves_control_mapping() {
        let key = TerminalKeyV2 {
            attachment_id: crate::AttachmentId::from_bytes([0; 16]),
            kind: TerminalKeyV2Kind::Ascii,
            modifiers: TerminalKeyV2Modifiers::CONTROL,
            value: b'i' as u32,
            event: TerminalKeyV2Event::Press,
            shifted_ascii: 0,
            action_id: 1,
        };
        assert_eq!(
            encode_terminal_key_v2(key, ModeState::default()).unwrap(),
            [0x09]
        );
        let alt_control = TerminalKeyV2 {
            modifiers: TerminalKeyV2Modifiers::ALT_CONTROL,
            ..key
        };
        assert_eq!(
            encode_terminal_key_v2(alt_control, ModeState::default()).unwrap(),
            b"\x1b\x09"
        );
        let unsupported = TerminalKeyV2 {
            modifiers: TerminalKeyV2Modifiers::NONE,
            ..key
        };
        assert!(encode_terminal_key_v2(unsupported, ModeState::default()).is_err());
    }

    #[test]
    fn kitty_event_types_cover_application_keypad_with_flag_two_alone() {
        let key = TerminalKeyV2 {
            attachment_id: crate::AttachmentId::from_bytes([0; 16]),
            kind: TerminalKeyV2Kind::Keypad,
            modifiers: TerminalKeyV2Modifiers::NONE,
            value: 1,
            event: TerminalKeyV2Event::Repeat,
            shifted_ascii: 0,
            action_id: 1,
        };
        let modes = ModeState {
            application_keypad: true,
            keyboard_flags: 2,
            ..ModeState::default()
        };
        assert_eq!(
            encode_terminal_key_v2(key, modes).unwrap(),
            b"\x1b[57400;1:2u"
        );
        assert_eq!(
            encode_terminal_key_v2(
                TerminalKeyV2 {
                    event: TerminalKeyV2Event::Release,
                    ..key
                },
                modes
            )
            .unwrap(),
            b"\x1b[57400;1:3u"
        );
    }

    #[test]
    fn modified_application_keypad_without_flag_one_is_unsupported_for_every_event() {
        let key = TerminalKeyV2 {
            attachment_id: crate::AttachmentId::from_bytes([0; 16]),
            kind: TerminalKeyV2Kind::Keypad,
            modifiers: TerminalKeyV2Modifiers::CONTROL,
            value: 16,
            event: TerminalKeyV2Event::Press,
            shifted_ascii: 0,
            action_id: 1,
        };
        let modes = ModeState {
            application_keypad: true,
            keyboard_flags: 2,
            ..ModeState::default()
        };
        assert!(encode_terminal_key_v2(key, modes).is_err());
        assert!(encode_terminal_key_v2(
            TerminalKeyV2 {
                event: TerminalKeyV2Event::Repeat,
                ..key
            },
            modes
        )
        .is_err());
        assert!(encode_terminal_key_v2(
            TerminalKeyV2 {
                event: TerminalKeyV2Event::Release,
                ..key
            },
            modes
        )
        .is_err());
        let flags_zero = ModeState::default();
        assert!(encode_terminal_key_v2(key, flags_zero).is_err());
        assert!(encode_terminal_key_v2(
            TerminalKeyV2 {
                event: TerminalKeyV2Event::Release,
                ..key
            },
            flags_zero
        )
        .is_err());
        assert!(encode_terminal_key_v2(
            TerminalKeyV2 {
                event: TerminalKeyV2Event::Release,
                ..key
            },
            ModeState {
                application_keypad: true,
                ..ModeState::default()
            }
        )
        .is_err());
    }

    #[test]
    fn kitty_keypad_equal_and_enter_use_their_assigned_codepoints() {
        let base = TerminalKeyV2 {
            attachment_id: crate::AttachmentId::from_bytes([0; 16]),
            kind: TerminalKeyV2Kind::Keypad,
            modifiers: TerminalKeyV2Modifiers::NONE,
            value: 15,
            event: TerminalKeyV2Event::Press,
            shifted_ascii: 0,
            action_id: 1,
        };
        let modes = ModeState {
            keyboard_flags: 1,
            ..ModeState::default()
        };
        assert_eq!(
            encode_terminal_key_v2(base, modes).unwrap(),
            b"\x1b[57415;1u"
        );
        assert_eq!(
            encode_terminal_key_v2(TerminalKeyV2 { value: 16, ..base }, modes).unwrap(),
            b"\x1b[57414;1u"
        );

        let event_modes = ModeState {
            application_keypad: true,
            keyboard_flags: 2,
            ..ModeState::default()
        };
        assert_eq!(
            encode_terminal_key_v2(
                TerminalKeyV2 {
                    value: 15,
                    event: TerminalKeyV2Event::Repeat,
                    ..base
                },
                event_modes
            )
            .unwrap(),
            b"\x1b[57415;1:2u"
        );
        assert_eq!(
            encode_terminal_key_v2(
                TerminalKeyV2 {
                    value: 16,
                    event: TerminalKeyV2Event::Release,
                    ..base
                },
                event_modes
            )
            .unwrap(),
            b"\x1b[57414;1:3u"
        );
    }

    #[test]
    fn kitty_event_types_cover_navigation_and_function_with_flag_two_alone() {
        let key = TerminalKeyV2 {
            attachment_id: crate::AttachmentId::from_bytes([0; 16]),
            kind: TerminalKeyV2Kind::ArrowUp,
            modifiers: TerminalKeyV2Modifiers::NONE,
            value: 0,
            event: TerminalKeyV2Event::Repeat,
            shifted_ascii: 0,
            action_id: 1,
        };
        let modes = ModeState {
            application_cursor: true,
            keyboard_flags: 2,
            ..ModeState::default()
        };
        assert_eq!(encode_terminal_key_v2(key, modes).unwrap(), b"\x1b[1;1:2A");
        assert_eq!(
            encode_terminal_key_v2(
                TerminalKeyV2 {
                    event: TerminalKeyV2Event::Release,
                    ..key
                },
                modes
            )
            .unwrap(),
            b"\x1b[1;1:3A"
        );
        let function = TerminalKeyV2 {
            kind: TerminalKeyV2Kind::Function,
            value: 5,
            ..key
        };
        assert_eq!(
            encode_terminal_key_v2(function, modes).unwrap(),
            b"\x1b[15;1:2~"
        );
    }

    #[test]
    fn legacy_alt_shift_ascii_uses_the_layout_derived_shifted_scalar() {
        let key = TerminalKeyV2 {
            attachment_id: crate::AttachmentId::from_bytes([0; 16]),
            kind: TerminalKeyV2Kind::Ascii,
            modifiers: TerminalKeyV2Modifiers::SHIFT,
            value: b'z' as u32,
            event: TerminalKeyV2Event::Press,
            shifted_ascii: b'@' as u32,
            action_id: 1,
        };
        let key = TerminalKeyV2 {
            modifiers: TerminalKeyV2Modifiers::ALT_SHIFT,
            ..key
        };
        assert_eq!(
            encode_terminal_key_v2(key, ModeState::default()).unwrap(),
            b"\x1b@"
        );
    }

    #[test]
    fn v2_escape_and_modified_enter_tab_backspace_use_specified_bytes() {
        let base = TerminalKeyV2 {
            attachment_id: crate::AttachmentId::from_bytes([0; 16]),
            kind: TerminalKeyV2Kind::Escape,
            modifiers: TerminalKeyV2Modifiers::NONE,
            value: 0,
            event: TerminalKeyV2Event::Press,
            shifted_ascii: 0,
            action_id: 1,
        };
        assert_eq!(
            encode_terminal_key_v2(base, ModeState::default()).unwrap(),
            b"\x1b"
        );
        assert_eq!(
            encode_terminal_key_v2(
                base,
                ModeState {
                    keyboard_flags: 1,
                    ..ModeState::default()
                }
            )
            .unwrap(),
            b"\x1b[27;1u"
        );
        let shift_tab = TerminalKeyV2 {
            kind: TerminalKeyV2Kind::Tab,
            modifiers: TerminalKeyV2Modifiers::SHIFT,
            ..base
        };
        assert_eq!(
            encode_terminal_key_v2(shift_tab, ModeState::default()).unwrap(),
            b"\x1b[Z"
        );
        let alt_enter = TerminalKeyV2 {
            kind: TerminalKeyV2Kind::Enter,
            modifiers: TerminalKeyV2Modifiers::ALT,
            ..base
        };
        assert_eq!(
            encode_terminal_key_v2(alt_enter, ModeState::default()).unwrap(),
            b"\x1b\r"
        );
        let control_enter = TerminalKeyV2 {
            kind: TerminalKeyV2Kind::Enter,
            modifiers: TerminalKeyV2Modifiers::CONTROL,
            ..base
        };
        assert_eq!(
            encode_terminal_key_v2(control_enter, ModeState::default()).unwrap(),
            [0x0d]
        );
        let control_backspace = TerminalKeyV2 {
            kind: TerminalKeyV2Kind::Backspace,
            modifiers: TerminalKeyV2Modifiers::CONTROL,
            ..base
        };
        assert_eq!(
            encode_terminal_key_v2(control_backspace, ModeState::default()).unwrap(),
            [0x08]
        );
        let alt_escape = TerminalKeyV2 {
            modifiers: TerminalKeyV2Modifiers::ALT,
            ..base
        };
        assert_eq!(
            encode_terminal_key_v2(alt_escape, ModeState::default()).unwrap(),
            b"\x1b\x1b"
        );
        assert_eq!(
            encode_terminal_key_v2(
                alt_escape,
                ModeState {
                    keyboard_flags: 2,
                    ..ModeState::default()
                }
            )
            .unwrap(),
            b"\x1b\x1b"
        );
        let alt_control_backspace = TerminalKeyV2 {
            kind: TerminalKeyV2Kind::Backspace,
            modifiers: TerminalKeyV2Modifiers::ALT_CONTROL,
            ..base
        };
        assert_eq!(
            encode_terminal_key_v2(alt_control_backspace, ModeState::default()).unwrap(),
            b"\x1b\x08"
        );
        let shift_escape = TerminalKeyV2 {
            modifiers: TerminalKeyV2Modifiers::SHIFT,
            ..base
        };
        assert_eq!(
            encode_terminal_key_v2(shift_escape, ModeState::default()).unwrap(),
            b"\x1b"
        );
        let control_escape = TerminalKeyV2 {
            modifiers: TerminalKeyV2Modifiers::CONTROL,
            ..base
        };
        assert_eq!(
            encode_terminal_key_v2(control_escape, ModeState::default()).unwrap(),
            b"\x1b"
        );
        let shift_control_escape = TerminalKeyV2 {
            modifiers: TerminalKeyV2Modifiers::from_bits_for_ffi(0b101).unwrap(),
            ..base
        };
        assert_eq!(
            encode_terminal_key_v2(
                shift_control_escape,
                ModeState {
                    keyboard_flags: 2,
                    ..ModeState::default()
                }
            )
            .unwrap(),
            b"\x1b"
        );
    }

    #[test]
    fn modified_f3_press_uses_tilde_csi_instead_of_cursor_report() {
        let f3 = TerminalKeyV2 {
            attachment_id: crate::AttachmentId::from_bytes([0; 16]),
            kind: TerminalKeyV2Kind::Function,
            modifiers: TerminalKeyV2Modifiers::SHIFT,
            value: 3,
            event: TerminalKeyV2Event::Press,
            shifted_ascii: 0,
            action_id: 1,
        };
        assert_eq!(
            encode_terminal_key_v2(f3, ModeState::default()).unwrap(),
            b"\x1b[13;2~"
        );
        assert_eq!(
            encode_terminal_key_v2(
                f3,
                ModeState {
                    keyboard_flags: 2,
                    ..ModeState::default()
                }
            )
            .unwrap(),
            b"\x1b[13;2~"
        );
        let control_f3 = TerminalKeyV2 {
            modifiers: TerminalKeyV2Modifiers::CONTROL,
            ..f3
        };
        assert_eq!(
            encode_terminal_key_v2(control_f3, ModeState::default()).unwrap(),
            b"\x1b[13;5~"
        );
        let unmodified = TerminalKeyV2 {
            modifiers: TerminalKeyV2Modifiers::NONE,
            ..f3
        };
        assert_eq!(
            encode_terminal_key_v2(unmodified, ModeState::default()).unwrap(),
            b"\x1bOR"
        );
        assert_eq!(
            encode_terminal_key_v2(
                unmodified,
                ModeState {
                    keyboard_flags: 1,
                    ..ModeState::default()
                }
            )
            .unwrap(),
            b"\x1b[13;1~"
        );
        let shift_f1 = TerminalKeyV2 {
            kind: TerminalKeyV2Kind::Function,
            modifiers: TerminalKeyV2Modifiers::SHIFT,
            value: 1,
            ..f3
        };
        assert_eq!(
            encode_terminal_key_v2(shift_f1, ModeState::default()).unwrap(),
            b"\x1b[1;2P"
        );
    }
}
