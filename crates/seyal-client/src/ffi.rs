use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    ptr, slice, str,
};

use seyal_render::PreparedCell;
use seyal_runtime::{
    ExecutionId,
    local_ipc::framing::{ComposerResultCode, Role, TerminalKeyKind},
};

use crate::{
    ClientError, LocalDisplayClient,
    local::{InputAdmissionFailure, ResizeFailure, derive_grid_geometry},
};

thread_local! {
    /// Each native Pane owns one entry. Values are boxed so borrowed C
    /// pointers remain stable when another Pane opens or closes a client.
    static CLIENTS: RefCell<HashMap<u64, Box<LocalDisplayClient>>> = RefCell::new(HashMap::new());
    static ACTIVE_HANDLE: Cell<u64> = const { Cell::new(0) };
    static NEXT_HANDLE: Cell<u64> = const { Cell::new(1) };
    static LAST_RECOVERY_RESULT: Cell<SeyalRecoveryResult> = const { Cell::new(SeyalRecoveryResult::empty()) };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct SeyalRecoveryResult {
    pub stage: u8,
    pub failure_class: u8,
    pub retryable: u8,
    pub connection_origin: u8,
    pub handle: u64,
    pub runtime_id_low: u64,
    pub runtime_id_high: u64,
    pub execution_id_low: u64,
    pub execution_id_high: u64,
    pub attachment_id_low: u64,
    pub attachment_id_high: u64,
}

impl SeyalRecoveryResult {
    const fn empty() -> Self {
        Self {
            stage: 0,
            failure_class: 0,
            retryable: 0,
            connection_origin: 0,
            handle: 0,
            runtime_id_low: 0,
            runtime_id_high: 0,
            execution_id_low: 0,
            execution_id_high: 0,
            attachment_id_low: 0,
            attachment_id_high: 0,
        }
    }
}

#[cfg(test)]
mod recovery_result_tests {
    use super::{SeyalRecoveryResult, set_recovery_failure};
    use crate::ClientError;

    #[test]
    fn retryable_discovery_failure_is_typed() {
        set_recovery_failure(ClientError::RuntimeDiscovery);
        let result = super::LAST_RECOVERY_RESULT.with(std::cell::Cell::get);
        assert_eq!(result.failure_class, 1);
        assert_eq!(result.retryable, 1);
        assert_eq!(result.stage, 1);
    }

    #[test]
    fn security_and_controller_failures_are_not_retryable() {
        set_recovery_failure(ClientError::Protocol);
        let protocol = super::LAST_RECOVERY_RESULT.with(std::cell::Cell::get);
        assert_eq!(protocol.failure_class, 4);
        assert_eq!(protocol.retryable, 0);

        set_recovery_failure(ClientError::Server(9));
        let busy = super::LAST_RECOVERY_RESULT.with(std::cell::Cell::get);
        assert_eq!(busy.failure_class, 3);
        assert_eq!(busy.retryable, 1);
        let _ = SeyalRecoveryResult::empty();
    }
}

fn set_recovery_failure(error: ClientError) {
    let (failure_class, retryable) = match error {
        ClientError::RuntimeDiscovery => (1, 1),
        ClientError::Io | ClientError::Disconnected => (2, 1),
        ClientError::NoRunningExecution => (2, 0),
        ClientError::AmbiguousExecutions => (6, 0),
        ClientError::Server(9) => (3, 1),
        ClientError::UnsupportedDisplayCapability
        | ClientError::UnsupportedInteractiveCapability
        | ClientError::Protocol
        | ClientError::InvalidAttachment
        | ClientError::Display
        | ClientError::Prepare
        | ClientError::Capacity
        | ClientError::CommitTooLarge
        | ClientError::LostController
        | ClientError::ResizeProtocolFailure
        | ClientError::InvalidGeometry
        | ClientError::BlockMetadataConflict
        | ClientError::Server(_) => (4, 0),
        ClientError::ClientBackpressure => (5, 1),
    };
    LAST_RECOVERY_RESULT.with(|result| {
        result.set(SeyalRecoveryResult {
            stage: 1,
            failure_class,
            retryable,
            ..SeyalRecoveryResult::empty()
        });
    });
}

fn set_recovery_success(client: &LocalDisplayClient, handle: u64, origin: u8) {
    let (runtime_id_low, runtime_id_high) = identity_words(client.runtime_id());
    let execution = client.execution_id().to_bytes();
    let attachment = client.attachment_id().to_bytes();
    LAST_RECOVERY_RESULT.with(|result| {
        result.set(SeyalRecoveryResult {
            stage: 2,
            connection_origin: origin,
            handle,
            runtime_id_low,
            runtime_id_high,
            execution_id_low: u64::from_le_bytes(execution[..8].try_into().unwrap()),
            execution_id_high: u64::from_le_bytes(execution[8..].try_into().unwrap()),
            attachment_id_low: u64::from_le_bytes(attachment[..8].try_into().unwrap()),
            attachment_id_high: u64::from_le_bytes(attachment[8..].try_into().unwrap()),
            ..SeyalRecoveryResult::empty()
        })
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_last_recovery_result() -> SeyalRecoveryResult {
    LAST_RECOVERY_RESULT.with(Cell::get)
}

fn identity_words(value: u128) -> (u64, u64) {
    let bytes = value.to_le_bytes();
    (
        u64::from_le_bytes(bytes[..8].try_into().unwrap()),
        u64::from_le_bytes(bytes[8..].try_into().unwrap()),
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_runtime_id_low() -> u64 {
    with_active_client(|client| identity_words(client.runtime_id()).0).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_runtime_id_high() -> u64 {
    with_active_client(|client| identity_words(client.runtime_id()).1).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_attachment_id_low() -> u64 {
    with_active_client(|client| {
        identity_words(u128::from_le_bytes(client.attachment_id().to_bytes())).0
    })
    .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_attachment_id_high() -> u64 {
    with_active_client(|client| {
        identity_words(u128::from_le_bytes(client.attachment_id().to_bytes())).1
    })
    .unwrap_or(0)
}

fn allocate_handle() -> u64 {
    NEXT_HANDLE.with(|next| {
        let handle = next.get();
        let next_handle = handle.wrapping_add(1);
        next.set(if next_handle == 0 { 1 } else { next_handle });
        if handle == 0 { 1 } else { handle }
    })
}

fn active_handle() -> u64 {
    ACTIVE_HANDLE.with(Cell::get)
}

fn with_active_client<R>(operation: impl FnOnce(&LocalDisplayClient) -> R) -> Option<R> {
    let handle = active_handle();
    CLIENTS.with(|clients| {
        clients
            .borrow()
            .get(&handle)
            .map(|client| operation(client))
    })
}

fn with_active_client_mut<R>(operation: impl FnOnce(&mut LocalDisplayClient) -> R) -> Option<R> {
    let handle = active_handle();
    CLIENTS.with(|clients| {
        clients
            .borrow_mut()
            .get_mut(&handle)
            .map(|client| operation(client))
    })
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct SeyalPreparedFrame {
    pub cells: *const PreparedCell,
    pub cell_count: u32,
    pub generation: u64,
    pub rows: u16,
    pub columns: u16,
    pub cursor_row: u16,
    pub cursor_column: u16,
    pub cursor_visible: u8,
    pub alternate_screen: u8,
    pub full_rebuild: u8,
    pub reserved0: u8,
    pub rebuilt_row_count: u16,
    pub reserved1: u16,
    pub damage_word0: u64,
    pub damage_word1: u64,
    pub damage_word2: u64,
    pub damage_word3: u64,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct SeyalExecutionBlockMetadata {
    pub block_id_low: u64,
    pub block_id_high: u64,
    pub revision: u64,
    pub start_line_id: u64,
    pub state: u8,
    pub reserved: [u8; 7],
}

impl SeyalExecutionBlockMetadata {
    const fn empty() -> Self {
        Self {
            block_id_low: 0,
            block_id_high: 0,
            revision: 0,
            start_line_id: 0,
            state: 0,
            reserved: [0; 7],
        }
    }
}

#[cfg(test)]
mod pass8_execution_block_abi_tests {
    use std::mem::{align_of, offset_of, size_of};

    use super::SeyalExecutionBlockMetadata;

    #[test]
    fn execution_block_metadata_c_abi_is_exactly_40_bytes() {
        assert_eq!(size_of::<SeyalExecutionBlockMetadata>(), 40);
        assert_eq!(align_of::<SeyalExecutionBlockMetadata>(), 8);
        assert_eq!(offset_of!(SeyalExecutionBlockMetadata, block_id_low), 0);
        assert_eq!(offset_of!(SeyalExecutionBlockMetadata, block_id_high), 8);
        assert_eq!(offset_of!(SeyalExecutionBlockMetadata, revision), 16);
        assert_eq!(offset_of!(SeyalExecutionBlockMetadata, start_line_id), 24);
        assert_eq!(offset_of!(SeyalExecutionBlockMetadata, state), 32);
        assert_eq!(offset_of!(SeyalExecutionBlockMetadata, reserved), 33);
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct SeyalBlockRecord {
    pub id: u64,
    pub start_line: u64,
    pub end_line: u64,
    pub state: u8,
    pub reserved: [u8; 3],
    pub exit_status: i32,
    pub command: *const u8,
    pub command_len: u32,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct SeyalHistoryRow {
    pub line_id: u64,
    pub cells: *const SeyalHistoryCell,
    pub cell_count: u32,
}

/// A typed, immutable identity for one history response. Swift must carry the
/// Runtime pair together; start/end anchors are only a lookup hint and are not
/// sufficient to correlate responses when ranges overlap.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct SeyalHistoryRange {
    pub start_line: u64,
    pub end_line: u64,
    pub block_id: u64,
    pub request_id: u64,
    pub revision: u64,
    pub row_count: u32,
    pub reserved: u32,
}

impl SeyalHistoryRange {
    const fn empty() -> Self {
        Self {
            start_line: 0,
            end_line: 0,
            block_id: 0,
            request_id: 0,
            revision: 0,
            row_count: 0,
            reserved: 0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct SeyalComposerResult {
    pub request_id: u64,
    pub block_id: u64,
    pub code: u8,
    pub reserved: [u8; 7],
}

impl SeyalComposerResult {
    const fn empty() -> Self {
        Self {
            request_id: 0,
            block_id: 0,
            code: 4,
            reserved: [0; 7],
        }
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct SeyalHistoryCell {
    pub scalar: u32,
    pub foreground: u32,
    pub background: u32,
    pub flags: u16,
    pub reserved: u16,
}

impl SeyalHistoryRow {
    const fn empty() -> Self {
        Self {
            line_id: 0,
            cells: ptr::null(),
            cell_count: 0,
        }
    }
}

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

impl SeyalBlockRecord {
    const fn empty() -> Self {
        Self {
            id: 0,
            start_line: 0,
            end_line: 0,
            state: 0,
            reserved: [0; 3],
            exit_status: 0,
            command: ptr::null(),
            command_len: 0,
        }
    }
}

impl SeyalPreparedFrame {
    const fn empty() -> Self {
        Self {
            cells: ptr::null(),
            cell_count: 0,
            generation: 0,
            rows: 0,
            columns: 0,
            cursor_row: 0,
            cursor_column: 0,
            cursor_visible: 0,
            alternate_screen: 0,
            full_rebuild: 0,
            reserved0: 0,
            rebuilt_row_count: 0,
            reserved1: 0,
            damage_word0: 0,
            damage_word1: 0,
            damage_word2: 0,
            damage_word3: 0,
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_connect_first() -> i32 {
    if seyal_bridge_open_first() == 0 {
        -6
    } else {
        0
    }
}

/// Opens the first running execution as a new independent client handle. This
/// is retained for the current single-execution shell bootstrap; production
/// panes should prefer `seyal_bridge_open_execution` once their Runtime
/// execution identity is known.
#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_open_first() -> u64 {
    let client = match LocalDisplayClient::connect_first_running() {
        Ok(client) => client,
        Err(error) => {
            set_recovery_failure(error);
            return 0;
        }
    };
    let handle = allocate_handle();
    CLIENTS.with(|clients| {
        clients.borrow_mut().insert(handle, Box::new(client));
    });
    ACTIVE_HANDLE.with(|active| active.set(handle));
    CLIENTS.with(|clients| {
        if let Some(client) = clients.borrow().get(&handle) {
            set_recovery_success(client, handle, 1);
        }
    });
    handle
}

/// Opens a client for one explicitly selected Runtime execution and returns a
/// stable Pane-local handle. Handles are independent even when two executions
/// use identical Block/request counters.
#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_open_execution(execution_low: u64, execution_high: u64) -> u64 {
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&execution_low.to_le_bytes());
    bytes[8..].copy_from_slice(&execution_high.to_le_bytes());
    let execution_id = ExecutionId::from_bytes(bytes);
    let client = match LocalDisplayClient::connect_execution_id(execution_id, Role::Controller) {
        Ok(client) => client,
        Err(error) => {
            set_recovery_failure(error);
            return 0;
        }
    };
    let handle = allocate_handle();
    CLIENTS.with(|clients| {
        clients.borrow_mut().insert(handle, Box::new(client));
    });
    ACTIVE_HANDLE.with(|active| active.set(handle));
    CLIENTS.with(|clients| {
        if let Some(client) = clients.borrow().get(&handle) {
            set_recovery_success(client, handle, 2);
        }
    });
    handle
}

/// Selects the client used by the legacy-shaped bridge calls. Swift calls
/// this before every operation, allowing the existing ABI to remain compact
/// while each Pane still owns an independent socket/client.
#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_select(handle: u64) -> i32 {
    let exists = CLIENTS.with(|clients| clients.borrow().contains_key(&handle));
    if exists {
        ACTIVE_HANDLE.with(|active| active.set(handle));
        0
    } else {
        -1
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_disconnect_handle(handle: u64) {
    CLIENTS.with(|clients| {
        clients.borrow_mut().remove(&handle);
    });
    ACTIVE_HANDLE.with(|active| {
        if active.get() == handle {
            active.set(0);
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_socket_fd() -> i32 {
    with_active_client(LocalDisplayClient::socket_fd).unwrap_or(-1)
}

#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_execution_id_low() -> u64 {
    with_active_client(|client| {
        u64::from_le_bytes(client.execution_id().to_bytes()[0..8].try_into().unwrap())
    })
    .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_execution_id_high() -> u64 {
    with_active_client(|client| {
        u64::from_le_bytes(client.execution_id().to_bytes()[8..16].try_into().unwrap())
    })
    .unwrap_or(0)
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

/// Atomically submit one already-committed UTF-8 native text action.
///
/// # Safety
/// `bytes` must address `len` readable bytes for the duration of this call when
/// `len != 0`. The bridge copies/adopts no bytes after the function returns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn seyal_bridge_submit_utf8(bytes: *const u8, len: u32) -> i32 {
    if len == 0 {
        return 0;
    }
    if bytes.is_null() {
        return -4;
    }
    let Ok(len) = usize::try_from(len) else {
        return -11;
    };
    // SAFETY: the C/Swift caller contract above guarantees a readable range
    // for this synchronous call. The resulting slice is never retained.
    let bytes = unsafe { slice::from_raw_parts(bytes, len) };
    let Ok(text) = str::from_utf8(bytes) else {
        return -4;
    };
    with_active_client_mut(|client| client.submit_committed_text(text))
        .map_or(-1, |result| result.map_or_else(error_code, |_| 0))
}

/// Submit one complete command from the Pane composer through the
/// capability-negotiated Runtime Block route.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn seyal_bridge_submit_composer(bytes: *const u8, len: u32) -> i32 {
    if len == 0 || bytes.is_null() {
        return -4;
    }
    let Ok(len) = usize::try_from(len) else {
        return -11;
    };
    let bytes = unsafe { slice::from_raw_parts(bytes, len) };
    let Ok(command) = str::from_utf8(bytes) else {
        return -4;
    };
    with_active_client_mut(|client| client.submit_composer_command(command))
        .map_or(-1, |result| result.map_or_else(error_code, |_| 0))
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

/// Submit one M001 logical terminal key. `kind` uses SPEC-006 key-kind values;
/// only ControlAscii carries a nonzero `scalar`.
#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_submit_key(kind: u16, scalar: u32) -> i32 {
    let Some(kind) = terminal_key_kind(kind) else {
        return -4;
    };
    with_active_client_mut(|client| client.submit_terminal_key(kind, scalar))
        .map_or(-1, |result| result.map_or_else(error_code, |_| 0))
}

/// Validate logical viewport/cell metrics, derive a bounded rows/columns
/// proposal, and reconcile it through correlated Pass-7 resize.
#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_propose_geometry(
    viewport_width: f64,
    viewport_height: f64,
    horizontal_insets: f64,
    vertical_insets: f64,
    cell_width: f64,
    cell_height: f64,
    meaningful_layout_epoch: u8,
) -> i32 {
    let Some(geometry) = derive_grid_geometry(
        viewport_width,
        viewport_height,
        horizontal_insets,
        vertical_insets,
        cell_width,
        cell_height,
    ) else {
        return -17;
    };
    with_active_client_mut(|client| {
        client.set_desired_geometry_for_layout(geometry, meaningful_layout_epoch != 0)
    })
    .map_or(-1, |result| result.map_or_else(error_code, |_| 0))
}

#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_retry_resize() -> i32 {
    with_active_client_mut(LocalDisplayClient::retry_resize)
        .map_or(-1, |result| result.map_or_else(error_code, |_| 0))
}

/// Non-secret presentation reason only; never returns rejected input content.
#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_input_failure() -> i32 {
    with_active_client(|client| match client.input_failure() {
        None => 0,
        Some(InputAdmissionFailure::ClientBackpressure) => 1,
        Some(InputAdmissionFailure::CommitTooLarge) => 2,
        Some(InputAdmissionFailure::LostController) => 3,
        Some(InputAdmissionFailure::Disconnected) => 4,
    })
    .unwrap_or(0)
}

/// Non-secret resize failure reason only. Runtime result codes retain their
/// SPEC-004 numeric value under the 100-series namespace.
#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_resize_failure() -> i32 {
    with_active_client(|client| match client.resize_failure() {
        None => 0,
        Some(ResizeFailure::ClientBackpressure) => 1,
        Some(ResizeFailure::Apply(error)) => 100 + error as i32,
        Some(ResizeFailure::Protocol) => 200,
        Some(ResizeFailure::Disconnected) => 201,
    })
    .unwrap_or(0)
}

/// Borrow the current contiguous prepared surface.
///
/// The returned cell pointer is owned by the Rust client and is valid until the
/// next bridge poll that changes geometry or until disconnect. Swift consumes
/// it synchronously to update its native cached/GPU state; it never owns or
/// frees this memory.
#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_frame() -> SeyalPreparedFrame {
    with_active_client(|client| {
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

#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_disconnect() {
    let handle = active_handle();
    if handle != 0 {
        seyal_bridge_disconnect_handle(handle);
    }
}

fn terminal_key_kind(value: u16) -> Option<TerminalKeyKind> {
    Some(match value {
        1 => TerminalKeyKind::Enter,
        2 => TerminalKeyKind::Tab,
        3 => TerminalKeyKind::Backspace,
        4 => TerminalKeyKind::Escape,
        5 => TerminalKeyKind::ArrowUp,
        6 => TerminalKeyKind::ArrowDown,
        7 => TerminalKeyKind::ArrowRight,
        8 => TerminalKeyKind::ArrowLeft,
        9 => TerminalKeyKind::ControlAscii,
        _ => return None,
    })
}

fn error_code(error: ClientError) -> i32 {
    match error {
        ClientError::RuntimeDiscovery => -2,
        ClientError::Io => -3,
        ClientError::Protocol => -4,
        ClientError::UnsupportedDisplayCapability => -5,
        ClientError::NoRunningExecution => -6,
        ClientError::AmbiguousExecutions => -19,
        ClientError::InvalidAttachment => -7,
        ClientError::Display => -8,
        ClientError::Prepare => -9,
        ClientError::Disconnected => -10,
        ClientError::Capacity => -11,
        ClientError::UnsupportedInteractiveCapability => -12,
        ClientError::ClientBackpressure => -13,
        ClientError::CommitTooLarge => -14,
        ClientError::LostController => -15,
        ClientError::ResizeProtocolFailure => -16,
        ClientError::InvalidGeometry => -17,
        ClientError::BlockMetadataConflict => -18,
        ClientError::Server(code) => -1000 - i32::from(code),
    }
}
