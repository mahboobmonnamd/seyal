use seyal_exec::{Color, LineId};

use crate::{
    ExecutionId, RuntimeError,
    local_ipc::{
        attachment::AttachmentError,
        framing::{
            self, BlockTimeline, CAP_COMMAND_BLOCKS, CommandBlock, CommandBlockState, ErrorCode,
            MessageType,
        },
    },
};

use super::super::Runtime;

fn pack_terminal_color(color: Color) -> u32 {
    match color {
        Color::Default => 0,
        Color::Indexed(index) => 0x0100_0000 | u32::from(index),
        Color::Rgb { r, g, b } => {
            0x0200_0000 | (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b)
        }
    }
}

impl Runtime {
    pub(super) fn handle_history_range_request(&mut self, token: u64, payload: &[u8]) {
        let Ok(request) = framing::HistoryRangeRequest::decode(payload) else {
            self.send_error(
                token,
                ErrorCode::MalformedPayload,
                MessageType::HistoryRangeRequest as u16,
            );
            return;
        };
        let execution_id = match self.local_ipc.as_ref().map(|state| {
            state
                .attachments
                .execution_of(request.attachment_id)
                .and_then(|id| {
                    let attached = state
                        .connections
                        .get(&token)
                        .and_then(|meta| meta.attachment)
                        == Some(request.attachment_id);
                    attached
                        .then_some(id)
                        .ok_or(AttachmentError::PermissionDenied)
                })
        }) {
            Some(Ok(id)) => id,
            _ => {
                self.send_error(
                    token,
                    ErrorCode::StaleIdentity,
                    MessageType::HistoryRangeRequest as u16,
                );
                return;
            }
        };
        let Some(entry) = self.entries.get(&execution_id) else {
            self.send_error(
                token,
                ErrorCode::InvalidExecution,
                MessageType::HistoryRangeRequest as u16,
            );
            return;
        };
        let rows = entry.execution.terminal().primary_history_range(
            LineId(request.start_line),
            LineId(request.end_line),
            usize::from(request.max_lines),
        );
        let mut cell_budget = usize::try_from(request.max_cells).unwrap_or(0);
        let mut truncated = false;
        let mut encoded_rows = Vec::with_capacity(rows.len());
        for (line_id, cells) in rows {
            if cells.len() > cell_budget {
                truncated = true;
                break;
            }
            cell_budget -= cells.len();
            encoded_rows.push(framing::HistoryRow {
                line_id: line_id.0,
                cells: cells
                    .into_iter()
                    .map(|cell| framing::HistoryCell {
                        scalar: cell.character as u32,
                        foreground: pack_terminal_color(cell.style.fg),
                        background: pack_terminal_color(cell.style.bg),
                        flags: (u16::from(cell.style.bold))
                            | (u16::from(cell.style.underline) << 1)
                            | (u16::from(cell.style.inverse) << 2),
                        reserved: 0,
                    })
                    .collect(),
            });
        }
        if encoded_rows.len() < usize::from(request.max_lines) {
            truncated = truncated
                || request.end_line.saturating_sub(request.start_line) >= encoded_rows.len() as u64;
        }
        let snapshot = framing::HistoryRangeSnapshot {
            request_id: request.request_id,
            block_id: request.block_id,
            revision: entry.execution.terminal().damage_generation(),
            status: if truncated {
                framing::HistoryRangeStatus::Truncated
            } else {
                framing::HistoryRangeStatus::Complete
            },
            rows: encoded_rows,
        };
        let Ok(payload) = snapshot.try_encode() else {
            self.send_error(
                token,
                ErrorCode::CapacityExceeded,
                MessageType::HistoryRangeRequest as u16,
            );
            return;
        };
        let _ = self.send_mandatory_frame(
            token,
            framing::encode_frame(MessageType::HistoryRangeSnapshot, &payload),
        );
    }

    /// Broadcast a bounded replacement cache after the Runtime has observed a
    /// trusted OSC lifecycle transition. It is queued after display work so a
    /// slow client cannot delay PTY/VT or terminal projection progress.
    pub(in crate::runtime) fn publish_block_timeline(&mut self, execution_id: ExecutionId) {
        let Some(entry) = self.entries.get(&execution_id) else {
            return;
        };
        let records = entry
            .block_timeline
            .records()
            .map(|record| CommandBlock {
                id: record.id.raw(),
                command: record.command.clone(),
                start_line: record.start_line,
                end_line: record.end_line,
                state: match record.lifecycle {
                    crate::command_block_timeline::CommandBlockLifecycle::Running => CommandBlockState::Running,
                    crate::command_block_timeline::CommandBlockLifecycle::Completed { exit_status } => {
                        CommandBlockState::Completed { exit_status }
                    }
                },
            })
            .collect();
        let payload = BlockTimeline {
            revision: entry.block_revision,
            records,
        }
        .try_encode()
        .unwrap_or_default();
        if payload.is_empty() {
            return;
        }
        let frame = framing::encode_frame(MessageType::BlockTimeline, &payload);
        let tokens = self
            .local_ipc
            .as_ref()
            .map(|state| {
                state
                    .connections
                    .iter()
                    .filter_map(|(&token, meta)| {
                        let supports_blocks = meta.client_capabilities & CAP_COMMAND_BLOCKS != 0;
                        let attached_here = meta
                            .attachment
                            .and_then(|attachment| state.attachments.execution_of(attachment).ok())
                            == Some(execution_id);
                        (supports_blocks && attached_here).then_some(token)
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for token in tokens {
            let _ = self.send_after_display_frame(token, frame.clone());
        }
    }
}
