use crate::{
    local_ipc::{
        attachment::AttachmentError,
        framing::{
            self, ComposerCommandRef, ComposerResult, ComposerResultCode, ErrorCode, HostSearch,
            HostSelectionAction, MessageType, TerminalKey as WireTerminalKey, TerminalKeyKind,
            CAP_COMMAND_BLOCKS, MAX_INPUT_BYTES,
        },
    },
    RuntimeError,
};

use seyal_exec::{CopyModeMotion, PasteError};

use super::super::shell_integration::ComposerAdmission;
use super::super::Runtime;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CopyModeKeyAction {
    Exit,
    Yank,
    ToggleAnchor,
    Motion(CopyModeMotion),
    Swallow,
}

fn copy_mode_key_action(kind: TerminalKeyKind) -> CopyModeKeyAction {
    match kind {
        TerminalKeyKind::Escape => CopyModeKeyAction::Exit,
        TerminalKeyKind::Enter => CopyModeKeyAction::Yank,
        TerminalKeyKind::Tab => CopyModeKeyAction::ToggleAnchor,
        TerminalKeyKind::ArrowLeft => CopyModeKeyAction::Motion(CopyModeMotion::Left),
        TerminalKeyKind::ArrowRight => CopyModeKeyAction::Motion(CopyModeMotion::Right),
        TerminalKeyKind::ArrowUp => CopyModeKeyAction::Motion(CopyModeMotion::Up),
        TerminalKeyKind::ArrowDown => CopyModeKeyAction::Motion(CopyModeMotion::Down),
        TerminalKeyKind::Backspace | TerminalKeyKind::ControlAscii => CopyModeKeyAction::Swallow,
    }
}

fn encode_terminal_key(key: WireTerminalKey) -> Vec<u8> {
    match key.kind {
        TerminalKeyKind::Enter => vec![0x0d],
        TerminalKeyKind::Tab => vec![0x09],
        TerminalKeyKind::Backspace => vec![0x7f],
        TerminalKeyKind::Escape => vec![0x1b],
        TerminalKeyKind::ArrowUp => b"\x1b[A".to_vec(),
        TerminalKeyKind::ArrowDown => b"\x1b[B".to_vec(),
        TerminalKeyKind::ArrowRight => b"\x1b[C".to_vec(),
        TerminalKeyKind::ArrowLeft => b"\x1b[D".to_vec(),
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

    pub(super) fn handle_paste(&mut self, token: u64, payload: &[u8]) {
        let Ok(input) = framing::InputRef::decode(payload) else {
            self.send_error(
                token,
                ErrorCode::MalformedPayload,
                MessageType::Paste as u16,
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
                    MessageType::Paste as u16,
                );
                return;
            }
            _ => {
                self.send_error(token, ErrorCode::StaleIdentity, MessageType::Paste as u16);
                return;
            }
        };
        let Some(entry) = self.entries.get(&execution_id) else {
            self.send_error(
                token,
                ErrorCode::InvalidExecution,
                MessageType::Paste as u16,
            );
            return;
        };
        let wrapped = match entry.execution.terminal().encode_host_paste(input.bytes) {
            Ok(bytes) => bytes,
            Err(PasteError::Empty) => {
                self.send_error(
                    token,
                    ErrorCode::MalformedPayload,
                    MessageType::Paste as u16,
                );
                return;
            }
            Err(PasteError::TooLarge) => {
                self.send_error(
                    token,
                    ErrorCode::CapacityExceeded,
                    MessageType::Paste as u16,
                );
                return;
            }
        };
        match self.input_ingress(execution_id) {
            Ok(ingress) => {
                if ingress.try_submit(wrapped).is_err() {
                    self.send_error(token, ErrorCode::Backpressure, MessageType::Paste as u16);
                }
            }
            Err(_) => self.send_error(
                token,
                ErrorCode::InvalidExecution,
                MessageType::Paste as u16,
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
        if self.intercept_copy_mode_key(token, execution_id, key.kind, key.attachment_id) {
            return;
        }
        let bytes = encode_terminal_key(key);
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

    pub(super) fn handle_host_selection(&mut self, token: u64, payload: &[u8]) {
        let Ok(command) = framing::HostSelection::decode(payload) else {
            self.send_error(
                token,
                ErrorCode::MalformedPayload,
                MessageType::HostSelection as u16,
            );
            return;
        };
        let execution_id = match self.local_ipc.as_ref().map(|state| {
            state
                .attachments
                .authorize_mutation(token, command.attachment_id)
        }) {
            Some(Ok(id)) => id,
            Some(Err(AttachmentError::PermissionDenied)) => {
                self.send_error(
                    token,
                    ErrorCode::PermissionDenied,
                    MessageType::HostSelection as u16,
                );
                return;
            }
            _ => {
                self.send_error(
                    token,
                    ErrorCode::StaleIdentity,
                    MessageType::HostSelection as u16,
                );
                return;
            }
        };
        let mut copied = None;
        let mut malformed_kind = false;
        let mut yank_failed = false;
        {
            let Some(entry) = self.entries.get_mut(&execution_id) else {
                self.send_error(
                    token,
                    ErrorCode::InvalidExecution,
                    MessageType::HostSelection as u16,
                );
                return;
            };
            match command.action {
                HostSelectionAction::EnterCopyMode => entry.execution.enter_copy_mode(),
                HostSelectionAction::ExitCopyMode => entry.execution.exit_copy_mode(),
                HostSelectionAction::ToggleAnchor => entry.execution.copy_mode_toggle_anchor(),
                HostSelectionAction::ToggleKind => entry.execution.copy_mode_toggle_kind(),
                HostSelectionAction::Clear => entry.execution.clear_selection(),
                HostSelectionAction::SetVisual => {
                    let start = seyal_exec::VisualPos {
                        col: command.start_col,
                        row: command.start_row,
                    };
                    let end = seyal_exec::VisualPos {
                        col: command.end_col,
                        row: command.end_row,
                    };
                    match command.kind {
                        0 => entry.execution.set_linear_selection(start, end),
                        1 => entry.execution.set_rectangular_selection(start, end),
                        _ => malformed_kind = true,
                    }
                }
                HostSelectionAction::Yank => match entry.execution.yank_selection() {
                    Ok(text) => copied = Some(text),
                    Err(_) => yank_failed = true,
                },
            }
        }
        if malformed_kind {
            self.send_error(
                token,
                ErrorCode::MalformedPayload,
                MessageType::HostSelection as u16,
            );
        }
        if yank_failed {
            self.send_error(
                token,
                ErrorCode::InvalidState,
                MessageType::HostSelection as u16,
            );
        }
        if let Some(text) = copied {
            self.emit_copied_text(token, command.attachment_id, &text);
        }
    }

    pub(super) fn handle_host_search(&mut self, token: u64, payload: &[u8]) {
        let Ok(search) = HostSearch::decode(payload) else {
            self.send_error(
                token,
                ErrorCode::MalformedPayload,
                MessageType::HostSearch as u16,
            );
            return;
        };
        let execution_id = match self.local_ipc.as_ref().map(|state| {
            state
                .attachments
                .authorize_mutation(token, search.attachment_id)
        }) {
            Some(Ok(id)) => id,
            Some(Err(AttachmentError::PermissionDenied)) => {
                self.send_error(
                    token,
                    ErrorCode::PermissionDenied,
                    MessageType::HostSearch as u16,
                );
                return;
            }
            _ => {
                self.send_error(
                    token,
                    ErrorCode::StaleIdentity,
                    MessageType::HostSearch as u16,
                );
                return;
            }
        };
        let missed = {
            let Some(entry) = self.entries.get_mut(&execution_id) else {
                self.send_error(
                    token,
                    ErrorCode::InvalidExecution,
                    MessageType::HostSearch as u16,
                );
                return;
            };
            entry
                .execution
                .search_and_select(search.needle, search.forward)
                .is_none()
                && !search.needle.is_empty()
        };
        if missed {
            self.send_error(
                token,
                ErrorCode::InvalidState,
                MessageType::HostSearch as u16,
            );
        }
    }

    fn emit_copied_text(&mut self, token: u64, attachment_id: crate::AttachmentId, text: &str) {
        if text.len() > MAX_INPUT_BYTES as usize {
            self.send_error(
                token,
                ErrorCode::CapacityExceeded,
                MessageType::CopiedText as u16,
            );
            return;
        }
        let payload = framing::InputRef {
            attachment_id,
            bytes: text.as_bytes(),
        }
        .encode();
        let _ = self.send_mandatory_frame(
            token,
            framing::encode_frame(MessageType::CopiedText, &payload),
        );
    }

    fn intercept_copy_mode_key(
        &mut self,
        token: u64,
        execution_id: crate::ExecutionId,
        kind: TerminalKeyKind,
        attachment_id: crate::AttachmentId,
    ) -> bool {
        let copied = {
            let Some(entry) = self.entries.get_mut(&execution_id) else {
                return false;
            };
            if !entry.execution.copy_mode_active() {
                return false;
            }
            match copy_mode_key_action(kind) {
                CopyModeKeyAction::Exit => {
                    entry.execution.exit_copy_mode();
                    None
                }
                CopyModeKeyAction::Yank => entry.execution.yank_selection().ok(),
                CopyModeKeyAction::ToggleAnchor => {
                    entry.execution.copy_mode_toggle_anchor();
                    None
                }
                CopyModeKeyAction::Motion(motion) => {
                    entry.execution.copy_mode_motion(motion);
                    None
                }
                CopyModeKeyAction::Swallow => None,
            }
        };
        if let Some(text) = copied {
            self.emit_copied_text(token, attachment_id, &text);
        }
        true
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
    fn copy_mode_keys_are_classified_without_pty_fallthrough() {
        assert_eq!(
            copy_mode_key_action(TerminalKeyKind::Escape),
            CopyModeKeyAction::Exit
        );
        assert_eq!(
            copy_mode_key_action(TerminalKeyKind::Enter),
            CopyModeKeyAction::Yank
        );
        assert_eq!(
            copy_mode_key_action(TerminalKeyKind::Tab),
            CopyModeKeyAction::ToggleAnchor
        );
        assert_eq!(
            copy_mode_key_action(TerminalKeyKind::ArrowLeft),
            CopyModeKeyAction::Motion(CopyModeMotion::Left)
        );
        assert_eq!(
            copy_mode_key_action(TerminalKeyKind::ArrowRight),
            CopyModeKeyAction::Motion(CopyModeMotion::Right)
        );
        assert_eq!(
            copy_mode_key_action(TerminalKeyKind::ArrowUp),
            CopyModeKeyAction::Motion(CopyModeMotion::Up)
        );
        assert_eq!(
            copy_mode_key_action(TerminalKeyKind::ArrowDown),
            CopyModeKeyAction::Motion(CopyModeMotion::Down)
        );
        assert_eq!(
            copy_mode_key_action(TerminalKeyKind::Backspace),
            CopyModeKeyAction::Swallow
        );
        assert_eq!(
            copy_mode_key_action(TerminalKeyKind::ControlAscii),
            CopyModeKeyAction::Swallow
        );
    }
}
