use std::{cell::RefCell, ptr};

use seyal_render::PreparedCell;

use crate::{ClientError, LocalDisplayClient};

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
    pub damage_words: [u64; 4],
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
            damage_words: [0; 4],
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
            damage_words: result.rebuilt_rows.words(),
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
        ClientError::Server(code) => -1000 - i32::from(code),
        ClientError::Disconnected => -10,
        ClientError::Capacity => -11,
    }
}
