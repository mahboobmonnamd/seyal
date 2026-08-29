use std::{cell::RefCell, ptr, slice, str};

use seyal_render::PreparedCell;
use seyal_runtime::local_ipc::framing::TerminalKeyKind;

use crate::{
    ClientError, LocalDisplayClient,
    local::{InputAdmissionFailure, ResizeFailure, derive_grid_geometry},
};

thread_local! {
    static CLIENT: RefCell<Option<LocalDisplayClient>> = const { RefCell::new(None) };
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

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct SeyalBlockMetadata {
    pub available: u8,
    pub state: u8,
    pub reserved0: u16,
    pub reserved1: u32,
    pub block_id_low: u64,
    pub block_id_high: u64,
    pub revision: u64,
    pub start_line_id: u64,
}

impl SeyalBlockMetadata {
    const fn empty() -> Self {
        Self {
            available: 0,
            state: 0,
            reserved0: 0,
            reserved1: 0,
            block_id_low: 0,
            block_id_high: 0,
            revision: 0,
            start_line_id: 0,
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_connect_first() -> i32 {
    CLIENT.with(|slot| {
        let Ok(mut slot) = slot.try_borrow_mut() else {
            return -100;
        };
        if slot.is_some() {
            return 0;
        }
        match LocalDisplayClient::connect_first_running() {
            Ok(client) => {
                *slot = Some(client);
                0
            }
            Err(error) => error_code(error),
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_socket_fd() -> i32 {
    CLIENT.with(|slot| {
        let Ok(slot) = slot.try_borrow() else {
            return -100;
        };
        slot.as_ref().map_or(-1, LocalDisplayClient::socket_fd)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_execution_id_low() -> u64 {
    CLIENT.with(|slot| {
        slot.borrow().as_ref().map_or(0, |client| {
            u64::from_le_bytes(client.execution_id().to_bytes()[0..8].try_into().unwrap())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_execution_id_high() -> u64 {
    CLIENT.with(|slot| {
        slot.borrow().as_ref().map_or(0, |client| {
            u64::from_le_bytes(client.execution_id().to_bytes()[8..16].try_into().unwrap())
        })
    })
}

/// Drain ready Candidate-D work and prepare the latest committed state.
///
/// Returns 1 when the prepared surface changed, 0 when there was no complete
/// new display state, and a stable negative diagnostic code on failure.
#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_poll() -> i32 {
    CLIENT.with(|slot| {
        let Ok(mut slot) = slot.try_borrow_mut() else {
            return -100;
        };
        let Some(client) = slot.as_mut() else {
            return -1;
        };
        match client.poll_prepare() {
            Ok(Some(_)) => 1,
            Ok(None) => 0,
            Err(error) => error_code(error),
        }
    })
}

/// Returns 1 only while bounded nonblocking client→Runtime bytes remain.
#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_wants_write() -> i32 {
    CLIENT.with(|slot| {
        let Ok(slot) = slot.try_borrow() else {
            return -100;
        };
        slot.as_ref()
            .map_or(0, |client| i32::from(client.wants_write()))
    })
}

/// Advance one pending write after writable readiness. A partial write or
/// `WouldBlock` remains queued and is not treated as a disconnect.
#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_flush_writable() -> i32 {
    CLIENT.with(|slot| {
        let Ok(mut slot) = slot.try_borrow_mut() else {
            return -100;
        };
        let Some(client) = slot.as_mut() else {
            return -1;
        };
        match client.flush_control_write() {
            Ok(()) => 0,
            Err(error) => error_code(error),
        }
    })
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
    with_client_mut(|client| client.submit_committed_text(text))
}

/// Submit one M001 logical terminal key. `kind` uses SPEC-006 key-kind values;
/// only ControlAscii carries a nonzero `scalar`.
#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_submit_key(kind: u16, scalar: u32) -> i32 {
    let Some(kind) = terminal_key_kind(kind) else {
        return -4;
    };
    with_client_mut(|client| client.submit_terminal_key(kind, scalar))
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
    with_client_mut(|client| {
        client.set_desired_geometry_for_layout(geometry, meaningful_layout_epoch != 0)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_retry_resize() -> i32 {
    with_client_mut(LocalDisplayClient::retry_resize)
}

/// Non-secret presentation reason only; never returns rejected input content.
#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_input_failure() -> i32 {
    CLIENT.with(|slot| {
        let Ok(slot) = slot.try_borrow() else {
            return -100;
        };
        let Some(client) = slot.as_ref() else {
            return 0;
        };
        match client.input_failure() {
            None => 0,
            Some(InputAdmissionFailure::ClientBackpressure) => 1,
            Some(InputAdmissionFailure::CommitTooLarge) => 2,
            Some(InputAdmissionFailure::LostController) => 3,
            Some(InputAdmissionFailure::Disconnected) => 4,
        }
    })
}

/// Non-secret resize failure reason only. Runtime result codes retain their
/// SPEC-004 numeric value under the 100-series namespace.
#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_resize_failure() -> i32 {
    CLIENT.with(|slot| {
        let Ok(slot) = slot.try_borrow() else {
            return -100;
        };
        let Some(client) = slot.as_ref() else {
            return 0;
        };
        match client.resize_failure() {
            None => 0,
            Some(ResizeFailure::ClientBackpressure) => 1,
            Some(ResizeFailure::Apply(error)) => 100 + error as i32,
            Some(ResizeFailure::Protocol) => 200,
            Some(ResizeFailure::Disconnected) => 201,
        }
    })
}

/// Read-only disposable Block metadata for presentation chrome. It contains no
/// terminal cells, transcript, command text, cwd, input or process data.
#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_block_metadata() -> SeyalBlockMetadata {
    CLIENT.with(|slot| {
        let Ok(slot) = slot.try_borrow() else {
            return SeyalBlockMetadata::empty();
        };
        let Some(client) = slot.as_ref() else {
            return SeyalBlockMetadata::empty();
        };
        let Some(block) = client.block_state() else {
            return SeyalBlockMetadata::empty();
        };
        let bytes = block.block_id.to_bytes();
        SeyalBlockMetadata {
            available: 1,
            state: block.state as u8,
            reserved0: 0,
            reserved1: 0,
            block_id_low: u64::from_le_bytes(bytes[0..8].try_into().unwrap()),
            block_id_high: u64::from_le_bytes(bytes[8..16].try_into().unwrap()),
            revision: block.revision,
            start_line_id: block.start_line_id,
        }
    })
}

/// Borrow the current contiguous prepared surface.
///
/// The returned cell pointer is owned by the Rust client and is valid until the
/// next bridge poll that changes geometry or until disconnect. Swift consumes
/// it synchronously to update its native cached/GPU state; it never owns or
/// frees this memory.
#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_frame() -> SeyalPreparedFrame {
    CLIENT.with(|slot| {
        let Ok(slot) = slot.try_borrow() else {
            return SeyalPreparedFrame::empty();
        };
        let Some(client) = slot.as_ref() else {
            return SeyalPreparedFrame::empty();
        };
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
}

#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_disconnect() {
    CLIENT.with(|slot| {
        if let Ok(mut slot) = slot.try_borrow_mut() {
            *slot = None;
        }
    });
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

fn with_client_mut(
    operation: impl FnOnce(&mut LocalDisplayClient) -> Result<(), ClientError>,
) -> i32 {
    CLIENT.with(|slot| {
        let Ok(mut slot) = slot.try_borrow_mut() else {
            return -100;
        };
        let Some(client) = slot.as_mut() else {
            return -1;
        };
        match operation(client) {
            Ok(()) => 0,
            Err(error) => error_code(error),
        }
    })
}

fn error_code(error: ClientError) -> i32 {
    match error {
        ClientError::RuntimeDiscovery => -2,
        ClientError::Io => -3,
        ClientError::Protocol => -4,
        ClientError::UnsupportedDisplayCapability => -5,
        ClientError::NoRunningExecution => -6,
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
