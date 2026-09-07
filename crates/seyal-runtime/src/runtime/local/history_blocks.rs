use seyal_exec::{Color, LineId};

use crate::{
    local_ipc::{
        attachment::AttachmentError,
        framing::{
            self, BlockTimeline, CommandBlock, CommandBlockState, ErrorCode, MessageType,
            CAP_COMMAND_BLOCKS,
        },
    },
    ExecutionId,
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
        let max_lines = usize::from(request.max_lines);
        let rows = entry.execution.terminal().primary_history_range(
            LineId(request.start_line),
            LineId(request.end_line),
            max_lines,
        );
        // VT already caps at max_lines. A full window means more in-range
        // retained rows may remain — report Truncated so clients can continue.
        let hit_line_cap = max_lines > 0 && rows.len() == max_lines;
        let mapped = rows
            .into_iter()
            .map(|(line_id, cells)| framing::HistoryRow {
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
        let (encoded_rows, budget_truncated) = framing::HistoryRangeSnapshot::admit_rows(
            mapped,
            max_lines,
            usize::try_from(request.max_cells).unwrap_or(0),
        );
        // Truncation is budget-driven only. Never infer Truncated from sparse
        // LineId numeric distance (alt-screen identity burn makes spans large).
        let truncated = budget_truncated || hit_line_cap;
        let mut snapshot = framing::HistoryRangeSnapshot {
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
        // Wire admission should make encode succeed. If an invariant still
        // breaks, shrink to a Truncated prefix rather than CapacityExceeded
        // (which the GUI treated as a fatal attachment tear-down).
        let payload = loop {
            match snapshot.try_encode() {
                Ok(payload) => break payload,
                Err(_) if !snapshot.rows.is_empty() => {
                    snapshot.rows.pop();
                    snapshot.status = framing::HistoryRangeStatus::Truncated;
                }
                Err(_) => {
                    self.send_error(
                        token,
                        ErrorCode::CapacityExceeded,
                        MessageType::HistoryRangeRequest as u16,
                    );
                    return;
                }
            }
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
                    crate::command_block_timeline::CommandBlockLifecycle::Running => {
                        CommandBlockState::Running
                    }
                    crate::command_block_timeline::CommandBlockLifecycle::Completed {
                        exit_status,
                    } => CommandBlockState::Completed { exit_status },
                },
            })
            .collect();
        let payload = match (BlockTimeline {
            revision: entry.block_revision,
            records,
        })
        .try_encode()
        {
            Ok(payload) => payload,
            Err(_) => {
                // Admission enforces the wire budget; encode failure here means
                // an invariant break. Never leave clients on a silently stale
                // replacement timeline — drop the frame and surface capacity
                // on every attached block-capable connection.
                let tokens = self
                    .local_ipc
                    .as_ref()
                    .map(|state| {
                        state
                            .connections
                            .iter()
                            .filter_map(|(&token, meta)| {
                                let supports_blocks =
                                    meta.client_capabilities & CAP_COMMAND_BLOCKS != 0;
                                let attached_here = meta.attachment.and_then(|attachment| {
                                    state.attachments.execution_of(attachment).ok()
                                }) == Some(execution_id);
                                (supports_blocks && attached_here).then_some(token)
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                for token in tokens {
                    self.send_error(
                        token,
                        ErrorCode::CapacityExceeded,
                        MessageType::BlockTimeline as u16,
                    );
                }
                return;
            }
        };
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
