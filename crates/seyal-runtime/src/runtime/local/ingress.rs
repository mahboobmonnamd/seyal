use seyal_exec::ModeState;

use crate::{
    local_ipc::{
        attachment::AttachmentError,
        framing::{
            self, ComposerCommandRef, ComposerResult, ComposerResultCode, ErrorCode, MessageType,
            TerminalKey as WireTerminalKey, TerminalKeyKind, TerminalKeyV2, TerminalKeyV2Event,
            TerminalKeyV2Kind, CAP_COMMAND_BLOCKS, CAP_EXTENDED_TERMINAL_KEY,
        },
    },
    RuntimeError,
};

use super::super::shell_integration::ComposerAdmission;
use super::super::Runtime;

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

fn encode_terminal_key(key: WireTerminalKey, modes: ModeState) -> Vec<u8> {
    match key.kind {
        TerminalKeyKind::Enter => vec![0x0d],
        TerminalKeyKind::Tab => vec![0x09],
        TerminalKeyKind::Backspace => vec![0x7f],
        TerminalKeyKind::Escape => vec![0x1b],
        TerminalKeyKind::ArrowUp => {
            if modes.application_cursor {
                b"\x1bOA".to_vec()
            } else {
                b"\x1b[A".to_vec()
            }
        }
        TerminalKeyKind::ArrowDown => {
            if modes.application_cursor {
                b"\x1bOB".to_vec()
            } else {
                b"\x1b[B".to_vec()
            }
        }
        TerminalKeyKind::ArrowRight => {
            if modes.application_cursor {
                b"\x1bOC".to_vec()
            } else {
                b"\x1b[C".to_vec()
            }
        }
        TerminalKeyKind::ArrowLeft => {
            if modes.application_cursor {
                b"\x1bOD".to_vec()
            } else {
                b"\x1b[D".to_vec()
            }
        }
        TerminalKeyKind::ControlAscii => {
            let scalar = key.scalar as u8;
            vec![match scalar {
                b' ' | b'@' => 0x00,
                b'A'..=b'Z' => scalar - b'@',
                b'[' => 0x1b,
                b'\\' => 0x1c,
                b']' => 0x1d,
                b'^' => 0x1e,
                b'_' => 0x1f,
                b'?' => 0x7f,
                _ => unreachable!("wire validation limits ControlAscii"),
            }]
        }
    }
}

fn encode_terminal_key_v2(key: TerminalKeyV2, modes: ModeState) -> Result<Vec<u8>, ()> {
    let modifiers = key.modifiers.bits();
    let flags = modes.keyboard_flags;
    let event = if flags & 2 != 0 {
        Some(key.event)
    } else {
        None
    };
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
        if modifiers & 2 != 0 {
            let mut prefixed = vec![0x1b];
            prefixed.extend(out);
            out = prefixed;
        }
        if modifiers & 4 != 0 && key.kind == TerminalKeyV2Kind::Backspace {
            out = vec![0x08];
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
            TerminalKeyV2Kind::Keypad => 57399 + key.value,
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
                if key.value <= 4 {
                    csi_mod(1, b'P' + key.value as u8 - 1, modifiers)
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
            _ => return Err(()),
        };
        return Ok(modified);
    }
    let mut out = match key.kind {
        TerminalKeyV2Kind::Escape => vec![0x1b],
        TerminalKeyV2Kind::Ascii => {
            if modifiers & 4 != 0 {
                vec![control_byte(key.value).ok_or(())?]
            } else if modifiers & 2 != 0 {
                vec![key.shifted_ascii.max(key.value) as u8]
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
                    0..=9 => vec![0x1b, b'p' + key.value as u8],
                    10 => b"\x1b[n".to_vec(),
                    11 => b"\x1b[o".to_vec(),
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

impl Runtime {
    pub(super) fn handle_input(&mut self, token: u64, payload: &[u8]) {
        let Ok(input) = framing::InputRef::decode(payload) else {
            self.send_error(
                token,
                ErrorCode::MalformedPayload,
                MessageType::Input as u16,
            );
            return;
        };
        let execution_id = match self.local_ipc.as_ref().map(|state| {
            state
                .attachments
                .authorize_mutation(token, input.attachment_id)
        }) {
            Some(Ok(id)) => id,
            Some(Err(AttachmentError::PermissionDenied)) => {
                self.send_error(
                    token,
                    ErrorCode::PermissionDenied,
                    MessageType::Input as u16,
                );
                return;
            }
            _ => {
                self.send_error(token, ErrorCode::StaleIdentity, MessageType::Input as u16);
                return;
            }
        };
        match self.input_ingress(execution_id) {
            Ok(ingress) => {
                if ingress.try_submit(input.bytes.to_vec()).is_err() {
                    self.send_error(token, ErrorCode::Backpressure, MessageType::Input as u16);
                }
            }
            Err(_) => self.send_error(
                token,
                ErrorCode::InvalidExecution,
                MessageType::Input as u16,
            ),
        }
    }

    pub(super) fn handle_terminal_key(&mut self, token: u64, payload: &[u8]) {
        let Ok(key) = WireTerminalKey::decode(payload) else {
            self.send_error(
                token,
                ErrorCode::MalformedPayload,
                MessageType::TerminalKey as u16,
            );
            return;
        };
        let execution_id = match self.local_ipc.as_ref().map(|state| {
            state
                .attachments
                .authorize_mutation(token, key.attachment_id)
        }) {
            Some(Ok(id)) => id,
            Some(Err(AttachmentError::PermissionDenied)) => {
                self.send_error(
                    token,
                    ErrorCode::PermissionDenied,
                    MessageType::TerminalKey as u16,
                );
                return;
            }
            _ => {
                self.send_error(
                    token,
                    ErrorCode::StaleIdentity,
                    MessageType::TerminalKey as u16,
                );
                return;
            }
        };
        let modes = self
            .entries
            .get(&execution_id)
            .map(|entry| entry.execution.terminal().modes());
        let Some(modes) = modes else {
            self.send_error(
                token,
                ErrorCode::InvalidExecution,
                MessageType::TerminalKey as u16,
            );
            return;
        };
        let bytes = encode_terminal_key(key, modes);
        match self.input_ingress(execution_id) {
            Ok(ingress) => {
                if ingress.try_submit(bytes).is_err() {
                    self.send_error(
                        token,
                        ErrorCode::Backpressure,
                        MessageType::TerminalKey as u16,
                    );
                }
            }
            Err(_) => self.send_error(
                token,
                ErrorCode::InvalidExecution,
                MessageType::TerminalKey as u16,
            ),
        }
    }

    pub(super) fn handle_terminal_key_v2(&mut self, token: u64, payload: &[u8]) {
        let Ok(key) = TerminalKeyV2::decode(payload) else {
            self.send_error(
                token,
                ErrorCode::MalformedPayload,
                MessageType::TerminalKeyV2 as u16,
            );
            return;
        };
        let supports = self
            .local_ipc
            .as_ref()
            .and_then(|state| state.connections.get(&token))
            .is_some_and(|meta| meta.client_capabilities & CAP_EXTENDED_TERMINAL_KEY != 0);
        if !supports {
            self.send_error(
                token,
                ErrorCode::PermissionDenied,
                MessageType::TerminalKeyV2 as u16,
            );
            return;
        }
        let execution_id = match self.local_ipc.as_ref().map(|state| {
            state
                .attachments
                .authorize_mutation(token, key.attachment_id)
        }) {
            Some(Ok(id)) => id,
            Some(Err(AttachmentError::PermissionDenied)) => {
                self.send_error(
                    token,
                    ErrorCode::PermissionDenied,
                    MessageType::TerminalKeyV2 as u16,
                );
                return;
            }
            _ => {
                self.send_error(
                    token,
                    ErrorCode::StaleIdentity,
                    MessageType::TerminalKeyV2 as u16,
                );
                return;
            }
        };
        let monotonic = self
            .local_ipc
            .as_ref()
            .and_then(|state| state.connections.get(&token))
            .is_some_and(|meta| key.action_id > meta.last_terminal_key_action_id);
        if !monotonic {
            self.send_error(
                token,
                ErrorCode::MalformedPayload,
                MessageType::TerminalKeyV2 as u16,
            );
            return;
        }
        let Some(modes) = self
            .entries
            .get(&execution_id)
            .map(|entry| entry.execution.terminal().modes())
        else {
            self.send_error(
                token,
                ErrorCode::InvalidExecution,
                MessageType::TerminalKeyV2 as u16,
            );
            return;
        };
        let Ok(bytes) = encode_terminal_key_v2(key, modes) else {
            self.send_error(
                token,
                ErrorCode::MalformedPayload,
                MessageType::TerminalKeyV2 as u16,
            );
            return;
        };
        if let Some(state) = self.local_ipc.as_mut()
            && let Some(meta) = state.connections.get_mut(&token)
        {
            meta.last_terminal_key_action_id = key.action_id;
        }
        if !bytes.is_empty() {
            match self.input_ingress(execution_id) {
                Ok(ingress) => {
                    if ingress.try_submit(bytes).is_err() {
                        self.send_error(
                            token,
                            ErrorCode::Backpressure,
                            MessageType::TerminalKeyV2 as u16,
                        );
                    }
                }
                Err(_) => self.send_error(
                    token,
                    ErrorCode::InvalidExecution,
                    MessageType::TerminalKeyV2 as u16,
                ),
            }
        }
    }

    pub(super) fn handle_composer_command(&mut self, token: u64, payload: &[u8]) {
        let Ok(request) = ComposerCommandRef::decode(payload) else {
            self.send_error(
                token,
                ErrorCode::MalformedPayload,
                MessageType::ComposerCommand as u16,
            );
            return;
        };
        let supports_blocks = self
            .local_ipc
            .as_ref()
            .and_then(|state| {
                state
                    .connections
                    .get(&token)
                    .map(|meta| meta.client_capabilities & CAP_COMMAND_BLOCKS != 0)
            })
            .unwrap_or(false);
        if !supports_blocks {
            self.send_error(
                token,
                ErrorCode::PermissionDenied,
                MessageType::ComposerCommand as u16,
            );
            return;
        }
        let execution_id = match self.local_ipc.as_ref().map(|state| {
            state
                .attachments
                .authorize_mutation(token, request.attachment_id)
        }) {
            Some(Ok(id)) => id,
            Some(Err(AttachmentError::PermissionDenied)) => {
                self.send_error(
                    token,
                    ErrorCode::PermissionDenied,
                    MessageType::ComposerCommand as u16,
                );
                return;
            }
            _ => {
                self.send_error(
                    token,
                    ErrorCode::StaleIdentity,
                    MessageType::ComposerCommand as u16,
                );
                return;
            }
        };
        match self.submit_composer_command(execution_id, request.command.to_owned()) {
            Ok(ComposerAdmission::Accepted(block_id)) => {
                self.publish_block_timeline(execution_id);
                let result = ComposerResult {
                    attachment_id: request.attachment_id,
                    code: ComposerResultCode::Accepted,
                    block_id: block_id.raw(),
                    request_id: request.request_id,
                };
                let _ = self.send_mandatory_frame(
                    token,
                    framing::encode_frame(MessageType::ComposerResult, &result.encode()),
                );
            }
            Ok(ComposerAdmission::Unsupported) => {
                let result = ComposerResult {
                    attachment_id: request.attachment_id,
                    code: ComposerResultCode::Unsupported,
                    block_id: 0,
                    request_id: request.request_id,
                };
                let _ = self.send_mandatory_frame(
                    token,
                    framing::encode_frame(MessageType::ComposerResult, &result.encode()),
                );
            }
            Ok(ComposerAdmission::Busy) => {
                let result = ComposerResult {
                    attachment_id: request.attachment_id,
                    code: ComposerResultCode::Busy,
                    block_id: 0,
                    request_id: request.request_id,
                };
                let _ = self.send_mandatory_frame(
                    token,
                    framing::encode_frame(MessageType::ComposerResult, &result.encode()),
                );
            }
            Err(RuntimeError::InputBackpressure | RuntimeError::ControlQueueFull) => {
                let result = ComposerResult {
                    attachment_id: request.attachment_id,
                    code: ComposerResultCode::Backpressure,
                    block_id: 0,
                    request_id: request.request_id,
                };
                let _ = self.send_mandatory_frame(
                    token,
                    framing::encode_frame(MessageType::ComposerResult, &result.encode()),
                );
            }
            Err(RuntimeError::ExecutionNotRunning) => {
                let result = ComposerResult {
                    attachment_id: request.attachment_id,
                    code: ComposerResultCode::Invalid,
                    block_id: 0,
                    request_id: request.request_id,
                };
                let _ = self.send_mandatory_frame(
                    token,
                    framing::encode_frame(MessageType::ComposerResult, &result.encode()),
                );
            }
            Err(_) => {
                let result = ComposerResult {
                    attachment_id: request.attachment_id,
                    code: ComposerResultCode::Invalid,
                    block_id: 0,
                    request_id: request.request_id,
                };
                let _ = self.send_mandatory_frame(
                    token,
                    framing::encode_frame(MessageType::ComposerResult, &result.encode()),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use seyal_protocol::framing::{TerminalKeyV2Event, TerminalKeyV2Kind, TerminalKeyV2Modifiers};

    #[test]
    fn m001_arrows_follow_canonical_cursor_mode() {
        let key = WireTerminalKey {
            attachment_id: crate::AttachmentId::from_bytes([0; 16]),
            kind: TerminalKeyKind::ArrowUp,
            modifiers: seyal_protocol::framing::TerminalKeyModifiers::NONE,
            scalar: 0,
        };
        assert_eq!(encode_terminal_key(key, ModeState::default()), b"\x1b[A");
        let modes = ModeState {
            application_cursor: true,
            ..ModeState::default()
        };
        assert_eq!(encode_terminal_key(key, modes), b"\x1bOA");
    }

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
}
