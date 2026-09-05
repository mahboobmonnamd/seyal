use seyal_protocol::pass8::encode_block_state_frame;

use crate::{
    RuntimeError,
    display::EncodedDisplayBatch,
    local_ipc::{
        framing::{self, ErrorCode, MessageType, ResizeResult as WireResizeResult, ResizeResultCode},
        recovery,
    },
};

use crate::AttachmentId;
use super::Runtime;

impl Runtime {
    pub(super) fn send_mandatory_frame(&mut self, token: u64, bytes: Vec<u8>) -> bool {
        let queued = self
            .local_ipc
            .as_mut()
            .is_some_and(|state| state.server.enqueue_mandatory(token, bytes).is_ok());
        if !queued {
            self.close_local_connection(token);
            return false;
        }
        self.sync_local_writable(token)
    }

    pub(super) fn send_after_display_frame(&mut self, token: u64, bytes: Vec<u8>) -> bool {
        let queued = self
            .local_ipc
            .as_mut()
            .is_some_and(|state| state.server.enqueue_after_display(token, bytes).is_ok());
        if !queued {
            self.close_local_connection(token);
            return false;
        }
        self.sync_local_writable(token)
    }

    pub(super) fn send_snapshot_batch(&mut self, token: u64, batch: EncodedDisplayBatch) -> bool {
        let queued = self
            .local_ipc
            .as_mut()
            .is_some_and(|state| state.server.enqueue_snapshot(token, batch).is_ok());
        if !queued {
            self.close_local_connection(token);
            return false;
        }
        self.sync_local_writable(token)
    }

    pub(super) fn schedule_snapshot_recovery(&mut self, token: u64) {
        let should_wake = if let Some(state) = self.local_ipc.as_mut() {
            if !state.connections.contains_key(&token) || !state.server.contains(token) {
                false
            } else if recovery::schedule_snapshot_recovery(
                &mut state.pending_resync,
                &mut state.pending_resync_set,
                token,
            ) {
                !state.server.has_snapshot_delivery(token)
            } else {
                false
            }
        } else {
            false
        };
        if should_wake {
            let _ = self.reactor.waker().wake();
        }
    }

    pub(super) fn send_error(&mut self, token: u64, code: ErrorCode, offending: u16) {
        let message = framing::ErrorMessage {
            error_code: code as u16,
            offending_message_type: offending,
            detail_code: 0,
        };
        let _ = self.send_mandatory_frame(
            token,
            framing::encode_frame(MessageType::Error, &message.encode()),
        );
    }

    pub(super) fn send_resize_result(
        &mut self,
        token: u64,
        attachment_id: AttachmentId,
        request_id: u64,
        result_code: ResizeResultCode,
        applied_generation: u64,
    ) {
        let message = WireResizeResult {
            attachment_id,
            request_id,
            result_code,
            detail_code: 0,
            applied_generation,
        };
        let _ = self.send_mandatory_frame(
            token,
            framing::encode_frame(MessageType::ResizeResult, &message.encode()),
        );
    }
}
