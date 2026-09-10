//! SPEC-006 §21.6 exhaustive encoder fixture matrix.
//!
//! Enumerates every validated V2 kind × flags 0/1/2/3 × press/repeat/release ×
//! accepted Shift/Alt/Control combination × DECCKM × DECNKM, classifies each
//! row as exact bytes, successful no-byte, or explicit unsupported, and pins a
//! regression digest so the table cannot silently drift. Kind-17 invalid
//! shifted-field cases are covered separately because they fail `validate`
//! before encoding.

use super::encode_terminal_key_v2;
use seyal_exec::ModeState;
use seyal_protocol::framing::{
    TerminalKeyV2, TerminalKeyV2Event, TerminalKeyV2Kind, TerminalKeyV2Modifiers,
};

const EXPECTED_VALID_ROWS: u64 = 36_384;
/// FNV-1a 64-bit digest over the ordered classification stream. Update only when
/// the §21.6 table intentionally changes and re-verify against SPEC.
const EXPECTED_DIGEST: u64 = 0xc3c7_7b88_6c50_9ebd;

fn mix(digest: &mut u64, bytes: &[u8]) {
    for &byte in bytes {
        *digest ^= u64::from(byte);
        *digest = digest.wrapping_mul(0x1000_0000_01b3);
    }
}

fn modifiers_mask(bits: u16) -> TerminalKeyV2Modifiers {
    TerminalKeyV2Modifiers::from_bits_for_ffi(bits).expect("fixture modifiers stay in 0b111")
}

fn ascii_shifted(base: u32) -> u32 {
    if (b'a' as u32..=b'z' as u32).contains(&base) {
        base - 32
    } else {
        base
    }
}

fn valid_keys() -> Vec<TerminalKeyV2> {
    let attachment = crate::AttachmentId::from_bytes([0; 16]);
    let events = [
        TerminalKeyV2Event::Press,
        TerminalKeyV2Event::Repeat,
        TerminalKeyV2Event::Release,
    ];
    let mut keys = Vec::with_capacity(12_000);
    let fixed = [
        TerminalKeyV2Kind::Enter,
        TerminalKeyV2Kind::Tab,
        TerminalKeyV2Kind::Backspace,
        TerminalKeyV2Kind::Escape,
        TerminalKeyV2Kind::ArrowUp,
        TerminalKeyV2Kind::ArrowDown,
        TerminalKeyV2Kind::ArrowRight,
        TerminalKeyV2Kind::ArrowLeft,
        TerminalKeyV2Kind::Home,
        TerminalKeyV2Kind::End,
        TerminalKeyV2Kind::Insert,
        TerminalKeyV2Kind::Delete,
        TerminalKeyV2Kind::PageUp,
        TerminalKeyV2Kind::PageDown,
    ];
    for kind in fixed {
        for modifiers in 0u16..=7 {
            for event in events {
                let key = TerminalKeyV2 {
                    attachment_id: attachment,
                    kind,
                    modifiers: modifiers_mask(modifiers),
                    value: 0,
                    event,
                    shifted_ascii: 0,
                    action_id: 1,
                };
                key.validate().expect("fixed-kind fixture must validate");
                keys.push(key);
            }
        }
    }
    for value in 1u32..=12 {
        for modifiers in 0u16..=7 {
            for event in events {
                let key = TerminalKeyV2 {
                    attachment_id: attachment,
                    kind: TerminalKeyV2Kind::Function,
                    modifiers: modifiers_mask(modifiers),
                    value,
                    event,
                    shifted_ascii: 0,
                    action_id: 1,
                };
                key.validate().expect("function fixture must validate");
                keys.push(key);
            }
        }
    }
    for value in 0u32..=16 {
        for modifiers in 0u16..=7 {
            for event in events {
                let key = TerminalKeyV2 {
                    attachment_id: attachment,
                    kind: TerminalKeyV2Kind::Keypad,
                    modifiers: modifiers_mask(modifiers),
                    value,
                    event,
                    shifted_ascii: 0,
                    action_id: 1,
                };
                key.validate().expect("keypad fixture must validate");
                keys.push(key);
            }
        }
    }
    for value in 0x20u32..=0x7e {
        if (b'A' as u32..=b'Z' as u32).contains(&value) {
            continue;
        }
        for modifiers in 0u16..=7 {
            let bits = modifiers;
            let has_alt_or_control = bits
                & (TerminalKeyV2Modifiers::ALT.bits() | TerminalKeyV2Modifiers::CONTROL.bits())
                != 0;
            if !has_alt_or_control {
                continue;
            }
            let shifted = if bits & TerminalKeyV2Modifiers::SHIFT.bits() != 0 {
                ascii_shifted(value)
            } else {
                0
            };
            for event in events {
                let key = TerminalKeyV2 {
                    attachment_id: attachment,
                    kind: TerminalKeyV2Kind::Ascii,
                    modifiers: modifiers_mask(modifiers),
                    value,
                    event,
                    shifted_ascii: shifted,
                    action_id: 1,
                };
                key.validate().expect("ascii fixture must validate");
                keys.push(key);
            }
        }
    }
    keys
}

#[test]
fn section_21_6_enumerates_and_classifies_every_valid_row() {
    let keys = valid_keys();
    let mut digest = 0xcbf2_9ce4_8422_2325; // FNV-1a offset basis
    let mut exact_bytes = 0u64;
    let mut no_byte = 0u64;
    let mut unsupported = 0u64;
    let mut rows = 0u64;

    for application_cursor in [false, true] {
        for application_keypad in [false, true] {
            for flags in 0u8..=3 {
                let modes = ModeState {
                    application_cursor,
                    application_keypad,
                    keyboard_flags: flags,
                    ..ModeState::default()
                };
                for key in &keys {
                    rows += 1;
                    mix(&mut digest, &[flags, application_cursor as u8, application_keypad as u8]);
                    mix(&mut digest, &(key.kind as u16).to_le_bytes());
                    mix(&mut digest, &key.modifiers.bits().to_le_bytes());
                    mix(&mut digest, &key.value.to_le_bytes());
                    mix(&mut digest, &[key.event as u8]);
                    mix(&mut digest, &key.shifted_ascii.to_le_bytes());
                    match encode_terminal_key_v2(*key, modes) {
                        Ok(bytes) if bytes.is_empty() => {
                            no_byte += 1;
                            mix(&mut digest, b"N");
                        }
                        Ok(bytes) => {
                            exact_bytes += 1;
                            mix(&mut digest, b"B");
                            mix(&mut digest, &bytes);
                            assert!(
                                bytes.len() <= 64,
                                "§21.6 encoder output must stay bounded: {:?}",
                                bytes
                            );
                            assert!(
                                !(bytes.starts_with(b"\x1b[") && bytes.ends_with(b"R")),
                                "CSI form must not end in R (cursor-report collision): {:?}",
                                bytes
                            );
                        }
                        Err(()) => {
                            unsupported += 1;
                            mix(&mut digest, b"E");
                        }
                    }
                }
            }
        }
    }

    assert_eq!(rows, EXPECTED_VALID_ROWS, "§21.6 valid-row cardinality drifted");
    assert_eq!(
        exact_bytes + no_byte + unsupported,
        EXPECTED_VALID_ROWS,
        "every row must be classified"
    );
    assert!(exact_bytes > 0 && no_byte > 0 && unsupported > 0);
    assert_eq!(
        digest, EXPECTED_DIGEST,
        "§21.6 classification digest drifted (exact={exact_bytes} empty={no_byte} err={unsupported})"
    );
}

#[test]
fn section_21_6_prose_examples_match_exact_bytes() {
    let attachment = crate::AttachmentId::from_bytes([0; 16]);
    let flags3 = ModeState {
        keyboard_flags: 3,
        ..ModeState::default()
    };
    let escape = TerminalKeyV2 {
        attachment_id: attachment,
        kind: TerminalKeyV2Kind::Escape,
        modifiers: TerminalKeyV2Modifiers::NONE,
        value: 0,
        event: TerminalKeyV2Event::Press,
        shifted_ascii: 0,
        action_id: 1,
    };
    assert_eq!(encode_terminal_key_v2(escape, flags3).unwrap(), b"\x1b[27;1u");

    let control_i = TerminalKeyV2 {
        kind: TerminalKeyV2Kind::Ascii,
        modifiers: TerminalKeyV2Modifiers::CONTROL,
        value: b'i' as u32,
        ..escape
    };
    assert_eq!(encode_terminal_key_v2(control_i, flags3).unwrap(), b"\x1b[105;5u");

    let up = TerminalKeyV2 {
        kind: TerminalKeyV2Kind::ArrowUp,
        modifiers: TerminalKeyV2Modifiers::NONE,
        ..escape
    };
    assert_eq!(
        encode_terminal_key_v2(
            TerminalKeyV2 {
                event: TerminalKeyV2Event::Repeat,
                ..up
            },
            flags3
        )
        .unwrap(),
        b"\x1b[1;1:2A"
    );
    assert_eq!(
        encode_terminal_key_v2(
            TerminalKeyV2 {
                event: TerminalKeyV2Event::Release,
                ..up
            },
            flags3
        )
        .unwrap(),
        b"\x1b[1;1:3A"
    );

    let keypad_enter = TerminalKeyV2 {
        kind: TerminalKeyV2Kind::Keypad,
        modifiers: TerminalKeyV2Modifiers::CONTROL,
        value: 16,
        ..escape
    };
    assert_eq!(
        encode_terminal_key_v2(keypad_enter, flags3).unwrap(),
        b"\x1b[57414;5u"
    );

    let f3 = TerminalKeyV2 {
        kind: TerminalKeyV2Kind::Function,
        value: 3,
        ..escape
    };
    assert_eq!(encode_terminal_key_v2(f3, flags3).unwrap(), b"\x1b[13;1~");

    let tab = TerminalKeyV2 {
        kind: TerminalKeyV2Kind::Tab,
        ..escape
    };
    assert_eq!(encode_terminal_key_v2(tab, flags3).unwrap(), b"\t");
    assert!(
        encode_terminal_key_v2(
            TerminalKeyV2 {
                event: TerminalKeyV2Event::Release,
                ..tab
            },
            flags3
        )
        .unwrap()
        .is_empty()
    );
}

#[test]
fn section_21_6_kind17_rejects_invalid_shifted_fields() {
    let attachment = crate::AttachmentId::from_bytes([0; 16]);
    let base = TerminalKeyV2 {
        attachment_id: attachment,
        kind: TerminalKeyV2Kind::Ascii,
        modifiers: TerminalKeyV2Modifiers::ALT,
        value: b'a' as u32,
        event: TerminalKeyV2Event::Press,
        shifted_ascii: 0,
        action_id: 1,
    };
    assert!(base.validate().is_ok());
    assert!(
        TerminalKeyV2 {
            modifiers: TerminalKeyV2Modifiers::ALT_SHIFT,
            shifted_ascii: 0,
            ..base
        }
        .validate()
        .is_err(),
        "Shift requires a printable shifted_ascii"
    );
    assert!(
        TerminalKeyV2 {
            shifted_ascii: b'A' as u32,
            ..base
        }
        .validate()
        .is_err(),
        "shifted_ascii without Shift is rejected"
    );
    assert!(
        TerminalKeyV2 {
            modifiers: TerminalKeyV2Modifiers::ALT_SHIFT,
            shifted_ascii: 0x7f,
            ..base
        }
        .validate()
        .is_err(),
        "non-printable shifted_ascii is rejected"
    );
    assert!(
        TerminalKeyV2 {
            value: b'A' as u32,
            ..base
        }
        .validate()
        .is_err(),
        "uppercase ASCII bases are rejected"
    );
    assert!(
        TerminalKeyV2 {
            modifiers: TerminalKeyV2Modifiers::NONE,
            ..base
        }
        .validate()
        .is_err(),
        "ASCII without Alt/Control belongs to committed text"
    );
    assert!(
        TerminalKeyV2 {
            modifiers: TerminalKeyV2Modifiers::SHIFT,
            shifted_ascii: b'A' as u32,
            ..base
        }
        .validate()
        .is_err(),
        "Shift alone without Alt/Control is rejected"
    );
}
