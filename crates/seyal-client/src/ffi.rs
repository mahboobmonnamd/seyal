//! Seyal.app ↔ `seyal-client` C ABI bridge.
//!
//! # Panic / unwind policy
//!
//! Every `extern "C"` entry point in this module is a C ABI surface consumed by
//! Swift. Unwinding across that boundary is undefined behavior. The workspace
//! `dev`/`release` profiles set `panic = "abort"`, so a Rust panic terminates
//! the process instead of unwinding into Swift. Callers must treat abort as the
//! only defined panic outcome; there is no catch-and-continue path across FFI.
//!
//! # Borrow lifetimes
//!
//! Pointer-carrying returns (`seyal_bridge_frame`, history rows, block command
//! bytes) borrow executor-local Rust storage. They are valid only until the next
//! mutating bridge call that can replace that storage (poll, disconnect, or a
//! later history consume). Swift must copy synchronously before returning to the
//! run loop; `NativePreparedFrame` owns a cell copy at construction so Rust
//! `PreparedCell` pointers never escape into long-lived Swift state.

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    path::PathBuf,
    ptr, slice, str,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use seyal_render::PreparedCell;
use seyal_runtime::{
    ExecutionId,
    local_ipc::{
        discovery::{
            DiscoveryError, control_socket_path, darwin_user_runtime_dir, verify_connected_peer_fd,
            verify_control_socket_leaf, verify_runtime_dir,
        },
        framing::{ComposerResultCode, ErrorCode, Role, TerminalKeyKind},
    },
};

use crate::{
    ClientError, DiscoveryFailure, LocalDisplayClient,
    local::{InputAdmissionFailure, ResizeFailure, derive_grid_geometry},
};

/// A completed lifecycle connection crosses executors exactly once, before it
/// becomes a Pane's nonblocking event-loop client. The mutex is never used by
/// poll, input, resize or rendering: those paths use the executor-local map.
struct PendingClient {
    client: Box<LocalDisplayClient>,
    origin: u8,
}

static PENDING_CLIENTS: OnceLock<Mutex<HashMap<u64, PendingClient>>> = OnceLock::new();
static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);
const DEFAULT_RECOVERY_BUDGET_MICROS: u64 = 1_000_000;

thread_local! {
    // A Pane's live Runtime client belongs to its AppKit executor. It is never
    // shared with the lifecycle queue after adoption, keeping all steady-state
    // terminal calls free of cross-pane locks.
    static CLIENTS: RefCell<HashMap<u64, Box<LocalDisplayClient>>> = RefCell::new(HashMap::new());
    static ACTIVE_HANDLE: Cell<u64> = const { Cell::new(0) };
    static LAST_RECOVERY_RESULT: Cell<SeyalRecoveryResult> = const { Cell::new(SeyalRecoveryResult::empty()) };
}

fn pending_clients() -> &'static Mutex<HashMap<u64, PendingClient>> {
    PENDING_CLIENTS.get_or_init(|| Mutex::new(HashMap::new()))
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
    use super::{
        SeyalRecoveryResult, classify_bridge_discovery_error, recovery_deadline,
        set_recovery_failure,
    };
    use crate::{ClientError, DiscoveryFailure};
    use seyal_runtime::local_ipc::{discovery::DiscoveryError, framing::ErrorCode};
    use std::io;

    #[test]
    fn retryable_discovery_failure_is_typed() {
        set_recovery_failure(ClientError::Discovery(DiscoveryFailure::EndpointMissing));
        let result = super::LAST_RECOVERY_RESULT.with(std::cell::Cell::get);
        assert_eq!(result.failure_class, 1);
        assert_eq!(result.retryable, 1);
        assert_eq!(result.stage, 1);
    }

    #[test]
    fn refusal_retries_without_claiming_a_missing_endpoint_or_launch_path() {
        set_recovery_failure(ClientError::Discovery(DiscoveryFailure::ConnectionRefused));
        let refused = super::LAST_RECOVERY_RESULT.with(std::cell::Cell::get);
        assert_eq!(refused.failure_class, 2);
        assert_eq!(refused.retryable, 1);

        set_recovery_failure(ClientError::Discovery(
            DiscoveryFailure::EndpointDisappeared,
        ));
        let disappeared = super::LAST_RECOVERY_RESULT.with(std::cell::Cell::get);
        assert_eq!(disappeared.failure_class, 2);
        assert_eq!(disappeared.retryable, 1);
    }

    #[test]
    fn untrusted_or_invalid_endpoint_is_never_retryable_or_launchable() {
        for failure in [
            DiscoveryFailure::UntrustedEndpoint,
            DiscoveryFailure::InvalidPath,
        ] {
            set_recovery_failure(ClientError::Discovery(failure));
            let result = super::LAST_RECOVERY_RESULT.with(std::cell::Cell::get);
            assert_eq!(result.failure_class, 4);
            assert_eq!(result.retryable, 0);
        }
    }

    #[test]
    fn security_and_controller_failures_are_not_retryable() {
        set_recovery_failure(ClientError::Protocol);
        let protocol = super::LAST_RECOVERY_RESULT.with(std::cell::Cell::get);
        assert_eq!(protocol.failure_class, 4);
        assert_eq!(protocol.retryable, 0);

        set_recovery_failure(ClientError::Server(ErrorCode::ControllerBusy));
        let busy = super::LAST_RECOVERY_RESULT.with(std::cell::Cell::get);
        assert_eq!(busy.failure_class, 3);
        assert_eq!(busy.retryable, 1);
        set_recovery_failure(ClientError::Server(ErrorCode::CapacityExceeded));
        let capacity = super::LAST_RECOVERY_RESULT.with(std::cell::Cell::get);
        assert_eq!(capacity.failure_class, 4);
        assert_eq!(capacity.retryable, 0);
        set_recovery_failure(ClientError::StartupDeadlineExceeded);
        let deadline = super::LAST_RECOVERY_RESULT.with(std::cell::Cell::get);
        assert_eq!(deadline.failure_class, 4);
        assert_eq!(deadline.retryable, 0);
        let _ = SeyalRecoveryResult::empty();
    }

    #[test]
    fn zero_budget_and_insecure_leaf_classification_fail_closed() {
        assert_eq!(
            recovery_deadline(0),
            Err(ClientError::StartupDeadlineExceeded)
        );
        assert_eq!(
            classify_bridge_discovery_error(DiscoveryError::NotOwnedByEffectiveUser),
            ClientError::Discovery(DiscoveryFailure::UntrustedEndpoint)
        );
        assert_eq!(
            classify_bridge_discovery_error(DiscoveryError::Io(io::Error::from(
                io::ErrorKind::NotFound,
            ))),
            ClientError::Discovery(DiscoveryFailure::EndpointMissing)
        );
    }
}

fn set_recovery_failure(error: ClientError) {
    let (failure_class, retryable) = match error {
        // Only an absent verified canonical endpoint permits the one helper
        // launch action. Refusal/disappearance remain bounded retries of that
        // same endpoint; trust/path failures fail closed.
        ClientError::Discovery(DiscoveryFailure::EndpointMissing) => (1, 1),
        ClientError::Discovery(
            DiscoveryFailure::ConnectionRefused | DiscoveryFailure::EndpointDisappeared,
        ) => (2, 1),
        ClientError::Discovery(
            DiscoveryFailure::UntrustedEndpoint | DiscoveryFailure::InvalidPath,
        ) => (4, 0),
        // The deadline is the caller's episode budget, not a transient I/O
        // failure. Retrying it would permit a recovery episode to outlive its
        // specified wall-clock bound.
        ClientError::StartupDeadlineExceeded => (4, 0),
        ClientError::Io | ClientError::Disconnected => (2, 1),
        ClientError::NoRunningExecution => (2, 0),
        ClientError::AmbiguousExecutions => (6, 0),
        ClientError::Server(ErrorCode::ControllerBusy) => (3, 1),
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct SeyalPass9DiagSnapshot {
    pub connected: u8,
    pub reserved0: [u8; 7],
    pub socket_fd: i32,
    pub live_handles: u32,
    pub pending_handles: u32,
    pub active_handle: u64,
    pub runtime_id_low: u64,
    pub runtime_id_high: u64,
    pub execution_id_low: u64,
    pub execution_id_high: u64,
    pub attachment_id_low: u64,
    pub attachment_id_high: u64,
}

impl SeyalPass9DiagSnapshot {
    const fn empty() -> Self {
        Self {
            connected: 0,
            reserved0: [0; 7],
            socket_fd: -1,
            live_handles: 0,
            pending_handles: 0,
            active_handle: 0,
            runtime_id_low: 0,
            runtime_id_high: 0,
            execution_id_low: 0,
            execution_id_high: 0,
            attachment_id_low: 0,
            attachment_id_high: 0,
        }
    }
}

/// Quiescent-only Pass 9 diagnostic. Callers must not invoke this from poll,
/// input, resize, or render hot paths.
#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_pass9_diag_snapshot() -> SeyalPass9DiagSnapshot {
    let live_handles = CLIENTS.with(|clients| clients.borrow().len() as u32);
    let pending_handles = pending_clients()
        .lock()
        .map(|pending| pending.len() as u32)
        .unwrap_or(0);
    let active_handle = ACTIVE_HANDLE.with(Cell::get);
    let Some(client) = with_active_client(|client| {
        let (runtime_id_low, runtime_id_high) = identity_words(client.runtime_id());
        let execution = client.execution_id().to_bytes();
        let attachment = client.attachment_id().to_bytes();
        SeyalPass9DiagSnapshot {
            connected: 1,
            reserved0: [0; 7],
            socket_fd: client.socket_fd(),
            live_handles,
            pending_handles,
            active_handle,
            runtime_id_low,
            runtime_id_high,
            execution_id_low: u64::from_le_bytes(execution[..8].try_into().unwrap()),
            execution_id_high: u64::from_le_bytes(execution[8..].try_into().unwrap()),
            attachment_id_low: u64::from_le_bytes(attachment[..8].try_into().unwrap()),
            attachment_id_high: u64::from_le_bytes(attachment[8..].try_into().unwrap()),
        }
    }) else {
        return SeyalPass9DiagSnapshot {
            live_handles,
            pending_handles,
            active_handle,
            ..SeyalPass9DiagSnapshot::empty()
        };
    };
    client
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
    let handle = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
    if handle == 0 { 1 } else { handle }
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

    use seyal_render::PreparedCell;
    use seyal_runtime::local_ipc::framing::HistoryCell;

    use super::{
        SeyalBlockRecord, SeyalExecutionBlockMetadata, SeyalHistoryCell, SeyalHistoryRow,
        SeyalPreparedFrame,
    };

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

    #[test]
    fn prepared_cell_matches_seyal_bridge_h() {
        assert_eq!(size_of::<PreparedCell>(), 16);
        assert_eq!(align_of::<PreparedCell>(), 4);
        assert_eq!(offset_of!(PreparedCell, scalar), 0);
        assert_eq!(offset_of!(PreparedCell, foreground), 4);
        assert_eq!(offset_of!(PreparedCell, background), 8);
        assert_eq!(offset_of!(PreparedCell, flags), 12);
        assert_eq!(offset_of!(PreparedCell, reserved), 14);
    }

    #[test]
    fn history_cell_matches_seyal_history_cell_and_header() {
        assert_eq!(size_of::<HistoryCell>(), 16);
        assert_eq!(size_of::<SeyalHistoryCell>(), 16);
        assert_eq!(align_of::<HistoryCell>(), align_of::<SeyalHistoryCell>());
        assert_eq!(offset_of!(HistoryCell, scalar), 0);
        assert_eq!(offset_of!(HistoryCell, foreground), 4);
        assert_eq!(offset_of!(HistoryCell, background), 8);
        assert_eq!(offset_of!(HistoryCell, flags), 12);
        assert_eq!(offset_of!(HistoryCell, reserved), 14);
        assert_eq!(
            offset_of!(SeyalHistoryCell, reserved),
            offset_of!(HistoryCell, reserved)
        );
    }

    #[test]
    fn prepared_frame_matches_seyal_bridge_h() {
        assert_eq!(size_of::<SeyalPreparedFrame>(), 72);
        assert_eq!(align_of::<SeyalPreparedFrame>(), 8);
        assert_eq!(offset_of!(SeyalPreparedFrame, cells), 0);
        assert_eq!(offset_of!(SeyalPreparedFrame, cell_count), 8);
        assert_eq!(offset_of!(SeyalPreparedFrame, generation), 16);
        assert_eq!(offset_of!(SeyalPreparedFrame, rows), 24);
        assert_eq!(offset_of!(SeyalPreparedFrame, columns), 26);
        assert_eq!(offset_of!(SeyalPreparedFrame, cursor_row), 28);
        assert_eq!(offset_of!(SeyalPreparedFrame, cursor_column), 30);
        assert_eq!(offset_of!(SeyalPreparedFrame, cursor_visible), 32);
        assert_eq!(offset_of!(SeyalPreparedFrame, alternate_screen), 33);
        assert_eq!(offset_of!(SeyalPreparedFrame, full_rebuild), 34);
        assert_eq!(offset_of!(SeyalPreparedFrame, reserved0), 35);
        assert_eq!(offset_of!(SeyalPreparedFrame, rebuilt_row_count), 36);
        assert_eq!(offset_of!(SeyalPreparedFrame, reserved1), 38);
        assert_eq!(offset_of!(SeyalPreparedFrame, damage_word0), 40);
        assert_eq!(offset_of!(SeyalPreparedFrame, damage_word3), 64);
    }

    #[test]
    fn history_row_and_block_record_match_seyal_bridge_h() {
        assert_eq!(size_of::<SeyalHistoryRow>(), 24);
        assert_eq!(align_of::<SeyalHistoryRow>(), 8);
        assert_eq!(offset_of!(SeyalHistoryRow, line_id), 0);
        assert_eq!(offset_of!(SeyalHistoryRow, cells), 8);
        assert_eq!(offset_of!(SeyalHistoryRow, cell_count), 16);

        assert_eq!(size_of::<SeyalBlockRecord>(), 48);
        assert_eq!(align_of::<SeyalBlockRecord>(), 8);
        assert_eq!(offset_of!(SeyalBlockRecord, id), 0);
        assert_eq!(offset_of!(SeyalBlockRecord, start_line), 8);
        assert_eq!(offset_of!(SeyalBlockRecord, end_line), 16);
        assert_eq!(offset_of!(SeyalBlockRecord, state), 24);
        assert_eq!(offset_of!(SeyalBlockRecord, reserved), 25);
        assert_eq!(offset_of!(SeyalBlockRecord, exit_status), 28);
        assert_eq!(offset_of!(SeyalBlockRecord, command), 32);
        assert_eq!(offset_of!(SeyalBlockRecord, command_len), 40);
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

fn recovery_deadline(budget_micros: u64) -> Result<Instant, ClientError> {
    if budget_micros == 0 {
        return Err(ClientError::StartupDeadlineExceeded);
    }
    Instant::now()
        .checked_add(Duration::from_micros(budget_micros))
        .ok_or(ClientError::StartupDeadlineExceeded)
}

fn ensure_recovery_deadline(deadline: Instant) -> Result<(), ClientError> {
    if Instant::now() < deadline {
        Ok(())
    } else {
        Err(ClientError::StartupDeadlineExceeded)
    }
}

fn classify_bridge_discovery_error(error: DiscoveryError) -> ClientError {
    match error {
        DiscoveryError::Io(error) if error.kind() == std::io::ErrorKind::NotFound => {
            ClientError::Discovery(DiscoveryFailure::EndpointMissing)
        }
        DiscoveryError::NotADirectory
        | DiscoveryError::NotOwnedByEffectiveUser
        | DiscoveryError::GroupOrWorldWritable
        | DiscoveryError::ActiveEndpoint => {
            ClientError::Discovery(DiscoveryFailure::UntrustedEndpoint)
        }
        DiscoveryError::ConfstrFailed
        | DiscoveryError::PathTooLongForSocket
        | DiscoveryError::Io(_) => ClientError::Discovery(DiscoveryFailure::InvalidPath),
    }
}

fn verified_recovery_socket_path() -> Result<PathBuf, ClientError> {
    let runtime_dir = darwin_user_runtime_dir().map_err(classify_bridge_discovery_error)?;
    verify_runtime_dir(&runtime_dir).map_err(classify_bridge_discovery_error)?;
    let socket_path = control_socket_path(&runtime_dir).map_err(classify_bridge_discovery_error)?;
    verify_control_socket_leaf(&socket_path).map_err(classify_bridge_discovery_error)?;
    Ok(socket_path)
}

fn verify_connected_recovery_client(
    client: &LocalDisplayClient,
    socket_path: &std::path::Path,
    deadline: Instant,
) -> Result<(), ClientError> {
    ensure_recovery_deadline(deadline)?;
    verify_control_socket_leaf(socket_path).map_err(classify_bridge_discovery_error)?;
    verify_connected_peer_fd(client.socket_fd()).map_err(classify_bridge_discovery_error)?;
    ensure_recovery_deadline(deadline)
}

fn register_pending_client(client: LocalDisplayClient, origin: u8) -> Result<u64, ClientError> {
    let handle = allocate_handle();
    let mut registry = pending_clients().lock().map_err(|_| ClientError::Io)?;
    registry.insert(
        handle,
        PendingClient {
            client: Box::new(client),
            origin,
        },
    );
    if let Some(pending) = registry.get(&handle) {
        set_recovery_success(&pending.client, handle, pending.origin);
    }
    Ok(handle)
}

/// Test-only hook: register an already-connected client as a pending adopt handle.
/// Used by adversarial FFI misuse tests that need a live handle without going
/// through discovery.
#[doc(hidden)]
pub fn test_register_pending_client(
    client: LocalDisplayClient,
    origin: u8,
) -> Result<u64, ClientError> {
    register_pending_client(client, origin)
}

#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_connect_first() -> i32 {
    let handle = seyal_bridge_open_first();
    if handle == 0 {
        return -6;
    }
    if seyal_bridge_adopt_handle(handle) == 0 {
        0
    } else {
        seyal_bridge_disconnect_handle(handle);
        -1
    }
}

/// Opens the first running execution as a new independent client handle. This
/// is retained for the current single-execution shell bootstrap; production
/// panes should prefer `seyal_bridge_open_execution` once their Runtime
/// execution identity is known.
#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_open_first() -> u64 {
    seyal_bridge_open_first_until(DEFAULT_RECOVERY_BUDGET_MICROS)
}

#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_open_first_until(budget_micros: u64) -> u64 {
    let deadline = match recovery_deadline(budget_micros) {
        Ok(deadline) => deadline,
        Err(error) => {
            set_recovery_failure(error);
            return 0;
        }
    };
    let socket_path = match verified_recovery_socket_path() {
        Ok(path) => path,
        Err(error) => {
            set_recovery_failure(error);
            return 0;
        }
    };
    let client = match LocalDisplayClient::connect_first_running_until(deadline) {
        Ok(client) => client,
        Err(error) => {
            set_recovery_failure(error);
            return 0;
        }
    };
    if let Err(error) = verify_connected_recovery_client(&client, &socket_path, deadline) {
        set_recovery_failure(error);
        return 0;
    }
    match register_pending_client(client, 1) {
        Ok(handle) => handle,
        Err(error) => {
            set_recovery_failure(error);
            0
        }
    }
}

/// Opens a client for one explicitly selected Runtime execution and returns a
/// stable Pane-local handle. Handles are independent even when two executions
/// use identical Block/request counters.
#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_open_execution(execution_low: u64, execution_high: u64) -> u64 {
    seyal_bridge_open_execution_until(
        execution_low,
        execution_high,
        DEFAULT_RECOVERY_BUDGET_MICROS,
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_open_execution_until(
    execution_low: u64,
    execution_high: u64,
    budget_micros: u64,
) -> u64 {
    let deadline = match recovery_deadline(budget_micros) {
        Ok(deadline) => deadline,
        Err(error) => {
            set_recovery_failure(error);
            return 0;
        }
    };
    let socket_path = match verified_recovery_socket_path() {
        Ok(path) => path,
        Err(error) => {
            set_recovery_failure(error);
            return 0;
        }
    };
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&execution_low.to_le_bytes());
    bytes[8..].copy_from_slice(&execution_high.to_le_bytes());
    let execution_id = ExecutionId::from_bytes(bytes);
    let client = match LocalDisplayClient::connect_execution_id_until(
        execution_id,
        Role::Controller,
        deadline,
    ) {
        Ok(client) => client,
        Err(error) => {
            set_recovery_failure(error);
            return 0;
        }
    };
    if let Err(error) = verify_connected_recovery_client(&client, &socket_path, deadline) {
        set_recovery_failure(error);
        return 0;
    }
    match register_pending_client(client, 2) {
        Ok(handle) => handle,
        Err(error) => {
            set_recovery_failure(error);
            0
        }
    }
}

/// Transfer a fully validated, disposable startup client from the lifecycle
/// queue to the calling Pane executor. A handle may be adopted exactly once.
///
/// # Thread affinity
///
/// Adoption installs the client into the calling thread's executor-local map.
/// Steady-state bridge calls must run on that same thread; selecting a handle
/// adopted on another thread fails closed.
///
/// # Panic policy
///
/// Panics abort the process (`panic = "abort"`); they never unwind into Swift.
#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_adopt_handle(handle: u64) -> i32 {
    let Some(pending) = pending_clients()
        .lock()
        .ok()
        .and_then(|mut pending| pending.remove(&handle))
    else {
        return -1;
    };
    set_recovery_success(&pending.client, handle, pending.origin);
    CLIENTS.with(|clients| {
        clients.borrow_mut().insert(handle, pending.client);
    });
    ACTIVE_HANDLE.with(|active| active.set(handle));
    0
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
    if let Ok(mut pending) = pending_clients().lock() {
        pending.remove(&handle);
    }
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

/// Atomically submit one already-committed UTF-8 native text action.
///
/// # Safety
/// - When `len != 0`, `bytes` must be non-null and address `len` readable bytes
///   for the full duration of this call.
/// - The bridge copies the bytes synchronously and retains nothing after return.
/// - Caller thread must own the active adopted handle (executor-local client).
/// - Panics abort; they must never unwind into Swift.
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
///
/// # Safety
/// - `bytes` must be non-null and address `len` readable bytes for the full
///   duration of this call (`len == 0` is rejected as invalid).
/// - The bridge validates UTF-8 and copies the command synchronously; no caller
///   bytes are retained after return.
/// - Caller thread must own the active adopted handle (executor-local client).
/// - Panics abort; they must never unwind into Swift.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn seyal_bridge_submit_composer(bytes: *const u8, len: u32) -> i32 {
    if len == 0 || bytes.is_null() {
        return -4;
    }
    let Ok(len) = usize::try_from(len) else {
        return -11;
    };
    // SAFETY: the C/Swift caller contract above guarantees a readable range
    // for this synchronous call. The resulting slice is never retained.
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
        ClientError::Discovery(_) => -2,
        ClientError::StartupDeadlineExceeded => -20,
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
        ClientError::Server(code) => -1000 - i32::from(code as u16),
    }
}

#[cfg(test)]
mod adversarial_ffi_misuse_tests {
    use std::{
        ptr,
        sync::{Arc, Barrier},
        thread,
    };

    use super::{
        SeyalPreparedFrame, seyal_bridge_adopt_handle, seyal_bridge_disconnect_handle,
        seyal_bridge_frame, seyal_bridge_poll, seyal_bridge_select, seyal_bridge_submit_composer,
        seyal_bridge_submit_utf8,
    };

    #[test]
    fn submit_utf8_rejects_null_with_nonzero_len() {
        let code = unsafe { seyal_bridge_submit_utf8(ptr::null(), 4) };
        assert_eq!(code, -4);
    }

    #[test]
    fn submit_utf8_accepts_empty_without_pointer() {
        let code = unsafe { seyal_bridge_submit_utf8(ptr::null(), 0) };
        assert_eq!(code, 0);
    }

    #[test]
    fn submit_composer_rejects_null_or_empty() {
        assert_eq!(unsafe { seyal_bridge_submit_composer(ptr::null(), 0) }, -4);
        assert_eq!(unsafe { seyal_bridge_submit_composer(ptr::null(), 3) }, -4);
    }

    #[test]
    fn frame_without_active_client_is_empty() {
        seyal_bridge_disconnect_handle(u64::MAX);
        let frame = seyal_bridge_frame();
        assert!(frame.cells.is_null());
        assert_eq!(frame.cell_count, 0);
        assert_eq!(SeyalPreparedFrame::empty().cell_count, 0);
    }

    #[test]
    fn poll_without_active_client_fails_closed() {
        seyal_bridge_disconnect_handle(u64::MAX);
        assert_eq!(seyal_bridge_poll(), -1);
    }

    #[test]
    fn select_unknown_handle_fails_closed() {
        assert_eq!(seyal_bridge_select(0), -1);
        assert_eq!(seyal_bridge_select(u64::MAX - 1), -1);
    }

    #[test]
    fn adopt_missing_handle_fails_closed() {
        assert_eq!(seyal_bridge_adopt_handle(0), -1);
        assert_eq!(seyal_bridge_adopt_handle(u64::MAX), -1);
    }

    #[test]
    fn double_adopt_of_absent_handle_stays_fail_closed() {
        // Absent-handle branch only. Live double-adopt after a successful first
        // adopt is covered by `tests/ffi_misuse_macos.rs`.
        let handle = 0x0ff1_ceda_u64;
        assert_eq!(seyal_bridge_adopt_handle(handle), -1);
        assert_eq!(seyal_bridge_adopt_handle(handle), -1);
    }

    #[test]
    fn wrong_thread_cannot_select_unadopted_handle() {
        // Absent-handle / unadopted branch only. Cross-thread select after a
        // successful adopt is covered by `tests/ffi_misuse_macos.rs`.
        let barrier = Arc::new(Barrier::new(2));
        let handle = 0x7ead_u64;
        let barrier_thread = Arc::clone(&barrier);
        let worker = thread::spawn(move || {
            assert_eq!(seyal_bridge_adopt_handle(handle), -1);
            barrier_thread.wait();
            assert_eq!(seyal_bridge_select(handle), -1);
        });
        barrier.wait();
        assert_eq!(seyal_bridge_select(handle), -1);
        worker.join().expect("worker");
    }

    #[test]
    fn use_after_poll_without_client_keeps_frame_empty() {
        // No-client fail-closed branch only. Live poll → disconnect invalidation
        // is covered by `tests/ffi_misuse_macos.rs`.
        assert_eq!(seyal_bridge_poll(), -1);
        let frame = seyal_bridge_frame();
        assert!(frame.cells.is_null());
        assert_eq!(seyal_bridge_poll(), -1);
        let after = seyal_bridge_frame();
        assert!(after.cells.is_null());
        assert_eq!(after.cell_count, 0);
    }
}
