use seyal_runtime::local_ipc::framing::ComposerResultCode;

use crate::LocalDisplayClient;

use super::{
    SeyalBlockRecord, SeyalComposerResult, SeyalExecutionBlockMetadata, SeyalHistoryCell,
    SeyalHistoryRange, SeyalHistoryRow, SeyalPreparedFrame, error_code, with_active_client,
    with_active_client_mut,
};

#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_block_timeline_revision() -> u64 {
    with_active_client(|client| client.block_timeline().revision).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_next_composer_request_id() -> u64 {
    with_active_client(LocalDisplayClient::next_composer_request_id).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_block_count() -> u32 {
    with_active_client(|client| u32::try_from(client.block_timeline().records.len()).ok())
        .flatten()
        .unwrap_or(0)
}

/// Returns a borrowed record from the current Runtime-owned replacement
/// cache. The command pointer is valid until the next bridge poll/disconnect.
#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_block_record(index: u32) -> SeyalBlockRecord {
    with_active_client(|client| {
        let Some(record) = client.block_timeline().records.get(index as usize) else {
            return SeyalBlockRecord::empty();
        };
        let command = record.command.as_bytes();
        SeyalBlockRecord {
            id: record.id,
            start_line: record.start_line,
            end_line: record.end_line.unwrap_or(0),
            state: match record.state {
                seyal_runtime::local_ipc::framing::CommandBlockState::Running => 0,
                seyal_runtime::local_ipc::framing::CommandBlockState::Completed { .. } => 1,
            },
            reserved: [0; 3],
            exit_status: match record.state {
                seyal_runtime::local_ipc::framing::CommandBlockState::Running => 0,
                seyal_runtime::local_ipc::framing::CommandBlockState::Completed { exit_status } => {
                    exit_status
                }
            },
            command: command.as_ptr(),
            command_len: command.len() as u32,
        }
    })
    .unwrap_or_else(SeyalBlockRecord::empty)
}

/// Returns the next request fence that will be assigned to a history request.
/// Native callers retain this typed fence and use it for every later lookup.
#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_next_history_request_id() -> u64 {
    with_active_client(LocalDisplayClient::next_history_request_id).unwrap_or(0)
}

/// Atomically peeks one bounded response by its typed Block/request identity.
/// Anchor coordinates are metadata only and never select a response.
#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_history_range_peek_for(
    block_id: u64,
    request_id: u64,
) -> SeyalHistoryRange {
    with_active_client(|client| {
        let Some(range) = client.history_range_for(block_id, request_id) else {
            return SeyalHistoryRange::empty();
        };
        let Some(first) = range.rows.first() else {
            return SeyalHistoryRange {
                start_line: 0,
                end_line: 0,
                block_id: range.block_id,
                request_id: range.request_id,
                revision: range.revision,
                row_count: 0,
                reserved: 0,
            };
        };
        let last = range.rows.last().map_or(first.line_id, |row| row.line_id);
        SeyalHistoryRange {
            start_line: first.line_id,
            end_line: last,
            block_id: range.block_id,
            request_id: range.request_id,
            revision: range.revision,
            row_count: range.rows.len() as u32,
            reserved: 0,
        }
    })
    .unwrap_or_else(SeyalHistoryRange::empty)
}

/// Returns one borrowed row using the typed block/request identity returned by
/// `seyal_bridge_history_range_peek_for`. The pointer remains valid until poll or
/// disconnect, and Swift copies the row before returning to the run loop.
#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_history_range_row_for(
    block_id: u64,
    request_id: u64,
    index: u32,
) -> SeyalHistoryRow {
    with_active_client(|client| {
        let Some(range) = client.history_range_for(block_id, request_id) else {
            return SeyalHistoryRow::empty();
        };
        let Some(row) = range.rows.get(index as usize) else {
            return SeyalHistoryRow::empty();
        };
        SeyalHistoryRow {
            line_id: row.line_id,
            cells: row.cells.as_ptr().cast::<SeyalHistoryCell>(),
            cell_count: row.cells.len() as u32,
        }
    })
    .unwrap_or_else(SeyalHistoryRow::empty)
}

/// Consumes a previously peeked response after its rows have been copied by
/// the native consumer. Identity is always the typed block/request pair.
#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_history_range_consume(block_id: u64, request_id: u64) -> u8 {
    with_active_client_mut(|client| u8::from(client.consume_history_range(block_id, request_id)))
        .unwrap_or(0)
}

/// Copies the latest Runtime ComposerResult into a typed C value. No command
/// text crosses this boundary; request ID is the only submission identity.
#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_composer_result() -> SeyalComposerResult {
    with_active_client(|client| client.last_composer_result())
        .flatten()
        .map(|result| SeyalComposerResult {
            request_id: result.request_id,
            block_id: result.block_id,
            code: match result.code {
                ComposerResultCode::Accepted => 0,
                ComposerResultCode::Busy => 1,
                ComposerResultCode::Unsupported => 2,
                ComposerResultCode::Backpressure => 3,
                ComposerResultCode::Invalid => 4,
            },
            reserved: [0; 7],
        })
        .unwrap_or_else(SeyalComposerResult::empty)
}

/// Read-only Pass 8 execution metadata for the active Pane client. No command
/// text, terminal cells, history, cwd, or PTY bytes cross this seam.
#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_execution_block_metadata() -> SeyalExecutionBlockMetadata {
    with_active_client(|client| client.block_state())
        .flatten()
        .map(|block| {
            let bytes = block.block_id.to_bytes();
            SeyalExecutionBlockMetadata {
                block_id_low: u64::from_le_bytes(bytes[..8].try_into().unwrap()),
                block_id_high: u64::from_le_bytes(bytes[8..].try_into().unwrap()),
                revision: block.revision,
                start_line_id: block.start_line_id,
                state: match block.state {
                    seyal_runtime::pass8::BlockLifecycle::Current => 1,
                    seyal_runtime::pass8::BlockLifecycle::Completed => 2,
                },
                reserved: [0; 7],
            }
        })
        .unwrap_or_else(SeyalExecutionBlockMetadata::empty)
}

/// Drain ready Candidate-D work and prepare the latest committed state.
///
/// Returns 1 when the prepared surface changed, 0 when there was no complete
/// new display state, and a stable negative diagnostic code on failure.
#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_poll() -> i32 {
    let Some(result) = with_active_client_mut(|client| client.poll_prepare()) else {
        return -1;
    };
    match result {
        Ok(Some(_)) => 1,
        Ok(None) => 0,
        Err(error) => error_code(error),
    }
}

/// Ensure the initial PreparedSurface after attach snapshot commit. Idempotent.
///
/// Returns 0 on success, -1 when no active client is selected, and a stable
/// negative diagnostic code on prepare failure.
#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_ensure_prepared() -> i32 {
    match with_active_client_mut(|client| client.ensure_prepared_surface()) {
        Some(Ok(_)) => 0,
        Some(Err(error)) => error_code(error),
        None => -1,
    }
}

/// Returns 1 only while bounded nonblocking client→Runtime bytes remain.
#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_wants_write() -> i32 {
    with_active_client(|client| i32::from(client.wants_write())).unwrap_or(0)
}

/// Advance one pending write after writable readiness. A partial write or
/// `WouldBlock` remains queued and is not treated as a disconnect.
#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_flush_writable() -> i32 {
    let Some(result) = with_active_client_mut(LocalDisplayClient::flush_control_write) else {
        return -1;
    };
    match result {
        Ok(()) => 0,
        Err(error) => error_code(error),
    }
}

/// Request a bounded canonical primary-history range for a completed Block.
/// The response is delivered asynchronously into the disposable client cache.
#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_request_history_range(
    block_id: u64,
    start_line: u64,
    end_line: u64,
    max_lines: u16,
    max_cells: u32,
) -> i32 {
    with_active_client_mut(|client| {
        client.request_history_range(block_id, start_line, end_line, max_lines, max_cells)
    })
    .map_or(-1, |result| result.map_or_else(error_code, |_| 0))
}

/// Borrow the current contiguous prepared surface.
///
/// The returned cell pointer borrows Rust-owned prepared storage and is valid
/// only until the next poll/prepare/disconnect that can replace that surface.
/// Swift must copy cells synchronously before returning to the run loop;
/// `NativePreparedFrame(bridgeFrame:)` performs that copy so Rust
/// `PreparedCell` pointers never escape into long-lived Swift state.
///
/// # Panic policy
///
/// Panics abort the process; they never unwind into Swift.
#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_frame() -> SeyalPreparedFrame {
    with_active_client_mut(|client| {
        if client.ensure_prepared_surface().is_err() {
            return SeyalPreparedFrame::empty();
        }
        let prepared = client.prepared_surface();
        let cells = prepared.prepared_cells();
        let Ok(cell_count) = u32::try_from(cells.len()) else {
            return SeyalPreparedFrame::empty();
        };
        let result = client.last_preparation();
        let cursor = prepared.cursor();
        let damage = result.rebuilt_rows.words();
        SeyalPreparedFrame {
            cells: cells.as_ptr(),
            cell_count,
            generation: prepared.generation().unwrap_or_default(),
            rows: prepared.rows(),
            columns: prepared.columns(),
            cursor_row: cursor.row,
            cursor_column: cursor.column,
            cursor_visible: u8::from(cursor.visible),
            alternate_screen: u8::from(prepared.alternate_screen()),
            full_rebuild: u8::from(result.full_rebuild),
            reserved0: 0,
            rebuilt_row_count: u16::try_from(result.rebuilt_row_count).unwrap_or(u16::MAX),
            reserved1: 0,
            damage_word0: damage[0],
            damage_word1: damage[1],
            damage_word2: damage[2],
            damage_word3: damage[3],
        }
    })
    .unwrap_or_else(SeyalPreparedFrame::empty)
}
