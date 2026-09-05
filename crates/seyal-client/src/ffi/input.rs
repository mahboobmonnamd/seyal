use std::{slice, str};

use seyal_runtime::local_ipc::framing::TerminalKeyKind;

use crate::{LocalDisplayClient, local::derive_grid_geometry};

use super::{error_code, with_active_client_mut};

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
