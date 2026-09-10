use seyal_exec::ModeState;

use crate::{
    key_v2_encode::encode_terminal_key_v2,
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

impl Runtime {
    pub(super) fn fatal_terminal_key_v2(&mut self, token: u64) {
        self.send_error(
            token,
            ErrorCode::MalformedPayload,
            MessageType::TerminalKeyV2 as u16,
        );
        if let Some(state) = self.local_ipc.as_mut()
            && let Some(meta) = state.connections.get_mut(&token)
        {
            meta.close_after_flush = true;
        }
        let _ = self.sync_local_writable(token);
    }

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
            self.fatal_terminal_key_v2(token);
            return;
        };
        let supports = self
            .local_ipc
            .as_ref()
            .and_then(|state| state.connections.get(&token))
            .is_some_and(|meta| meta.client_capabilities & CAP_EXTENDED_TERMINAL_KEY != 0);
        if !supports {
            self.fatal_terminal_key_v2(token);
            return;
        }
        let monotonic = self
            .local_ipc
            .as_ref()
            .and_then(|state| state.connections.get(&token))
            .is_some_and(|meta| key.action_id > meta.last_terminal_key_action_id);
        if !monotonic {
            self.fatal_terminal_key_v2(token);
            return;
        }
        if let Some(state) = self.local_ipc.as_mut()
            && let Some(meta) = state.connections.get_mut(&token)
        {
            meta.last_terminal_key_action_id = key.action_id;
        }
        let execution_id = match self.local_ipc.as_ref().map(|state| {
            state
                .attachments
                .authorize_mutation(token, key.attachment_id)
        }) {
            Some(Ok(id)) => id,
            Some(Err(AttachmentError::PermissionDenied)) => {
                self.send_error_detail(
                    token,
                    ErrorCode::PermissionDenied,
                    MessageType::TerminalKeyV2 as u16,
                    key.action_id,
                );
                return;
            }
            _ => {
                self.send_error_detail(
                    token,
                    ErrorCode::StaleIdentity,
                    MessageType::TerminalKeyV2 as u16,
                    key.action_id,
                );
                return;
            }
        };
        let Some(modes) = self
            .entries
            .get(&execution_id)
            .map(|entry| entry.execution.terminal().modes())
        else {
            self.send_error_detail(
                token,
                ErrorCode::InvalidExecution,
                MessageType::TerminalKeyV2 as u16,
                key.action_id,
            );
            return;
        };
        let Ok(bytes) = encode_terminal_key_v2(key, modes) else {
            self.send_error_detail(
                token,
                ErrorCode::MalformedPayload,
                MessageType::TerminalKeyV2 as u16,
                key.action_id,
            );
            return;
        };
        if !bytes.is_empty() {
            match self.input_ingress(execution_id) {
                Ok(ingress) => {
                    if ingress.try_submit(bytes).is_err() {
                        self.send_error_detail(
                            token,
                            ErrorCode::Backpressure,
                            MessageType::TerminalKeyV2 as u16,
                            key.action_id,
                        );
                    }
                }
                Err(_) => self.send_error_detail(
                    token,
                    ErrorCode::InvalidExecution,
                    MessageType::TerminalKeyV2 as u16,
                    key.action_id,
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
}
