//! Coarse product-session C ABI for the thin macOS host.

use std::sync::Mutex;

use crate::product::{ProductAction, ProductSession};

#[repr(C)]
pub struct SeyalProductSnapshot {
    pub composer_mode: u8,
    pub can_submit: u8,
    pub left_panel_tabs: u8,
    pub presentation_mode: u8,
    pub composer_epoch: u64,
    pub workspace_name: [u8; 64],
    pub tab_title: [u8; 64],
    pub pane_title: [u8; 64],
    pub draft: [u8; 512],
}

impl SeyalProductSnapshot {
    fn empty() -> Self {
        Self {
            composer_mode: 0,
            can_submit: 0,
            left_panel_tabs: 0,
            presentation_mode: 0,
            composer_epoch: 0,
            workspace_name: [0; 64],
            tab_title: [0; 64],
            pane_title: [0; 64],
            draft: [0; 512],
        }
    }
}

fn product() -> &'static Mutex<Option<ProductSession>> {
    static PRODUCT: Mutex<Option<ProductSession>> = Mutex::new(None);
    &PRODUCT
}

fn copy_text(dst: &mut [u8], src: &str) {
    dst.fill(0);
    let bytes = src.as_bytes();
    let n = bytes.len().min(dst.len().saturating_sub(1));
    dst[..n].copy_from_slice(&bytes[..n]);
}

#[unsafe(no_mangle)]
pub extern "C" fn seyal_product_create() -> i32 {
    match product().lock() {
        Ok(mut guard) => {
            *guard = Some(ProductSession::m001_local("local"));
            0
        }
        Err(_) => -1,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn seyal_product_destroy() {
    if let Ok(mut guard) = product().lock() {
        *guard = None;
    }
}

/// Copy one composer draft into the product session.
///
/// # Safety
/// - When `len != 0`, `bytes` must be non-null and address `len` readable bytes
///   for the full duration of this call.
/// - The bridge copies the bytes synchronously and retains nothing after return.
/// - Panics abort; they must never unwind into Swift.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn seyal_product_set_draft(bytes: *const u8, len: u32) -> i32 {
    if bytes.is_null() && len != 0 {
        return -2;
    }
    let text = if len == 0 {
        String::new()
    } else {
        // SAFETY: caller contract matches seyal_bridge_submit_utf8: readable
        // range for this synchronous call only.
        let slice = unsafe { std::slice::from_raw_parts(bytes, len as usize) };
        String::from_utf8_lossy(slice).into_owned()
    };
    apply(ProductAction::SetDraft(text))
}

#[unsafe(no_mangle)]
pub extern "C" fn seyal_product_submit() -> i32 {
    apply(ProductAction::Submit)
}

#[unsafe(no_mangle)]
pub extern "C" fn seyal_product_set_left_panel(tabs: u8) -> i32 {
    apply(if tabs == 0 {
        ProductAction::SetLeftPanelWorkspaces
    } else {
        ProductAction::SetLeftPanelTabs
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn seyal_product_snapshot() -> SeyalProductSnapshot {
    let Ok(guard) = product().lock() else {
        return SeyalProductSnapshot::empty();
    };
    let Some(session) = guard.as_ref() else {
        return SeyalProductSnapshot::empty();
    };
    let snap = session.snapshot();
    let mut out = SeyalProductSnapshot::empty();
    out.composer_mode = if snap.composer_hidden {
        2
    } else if snap.composer_busy {
        1
    } else {
        0
    };
    out.can_submit = u8::from(snap.can_submit);
    out.left_panel_tabs = u8::from(snap.left_panel_tabs);
    out.presentation_mode = match snap.presentation {
        crate::presentation::PresentationMode::Flow => 0,
        crate::presentation::PresentationMode::Raw => 1,
        crate::presentation::PresentationMode::Tui => 2,
    };
    out.composer_epoch = snap.composer_epoch;
    copy_text(&mut out.workspace_name, &snap.workspace_name);
    copy_text(&mut out.tab_title, &snap.tab_title);
    copy_text(&mut out.pane_title, &snap.pane_title);
    copy_text(&mut out.draft, &snap.draft);
    out
}

fn apply(action: ProductAction) -> i32 {
    let Ok(mut guard) = product().lock() else {
        return -1;
    };
    let Some(session) = guard.as_mut() else {
        return -3;
    };
    match session.apply(action) {
        Ok(()) => 0,
        Err(_) => -4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn product_abi_round_trips_draft() {
        assert_eq!(seyal_product_create(), 0);
        let text = b"pwd";
        assert_eq!(
            unsafe { seyal_product_set_draft(text.as_ptr(), text.len() as u32) },
            0
        );
        let snap = seyal_product_snapshot();
        assert_eq!(&snap.draft[..3], b"pwd");
        assert_eq!(snap.can_submit, 1);
        seyal_product_destroy();
    }
}
