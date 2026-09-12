//! Versioned one-Pane application-root C ABI.

use std::{cell::RefCell, collections::HashMap, ptr, slice, str, time::Duration};

use seyal_core::{AttachmentId, ExecutionId, PaneId};

use crate::app::{
    AppAction, AppError, AppFence, AppSnapshot, ApplicationRoot, BindingEvidence, NativeEffect,
    PresentationEligibility, APP_ABI_VERSION,
};
use crate::recovery::{AttemptOutcome, LaunchResult, RecoveryEffect, RecoveryStage};

use super::allocate_handle;

const FLAG_HAS_EXECUTION: u16 = 1;
const FLAG_HAS_ATTACHMENT: u16 = 2;
const FLAG_CONTROLLER: u16 = 4;
const FLAG_ALTERNATE_SCREEN: u16 = 8;
const FLAG_TARGET_CONTROLLER: u16 = 16;
const SNAP_COMPOSER: u16 = 1;
const SNAP_CONTROLLER: u16 = 2;
const SNAP_FROZEN: u16 = 4;
const SNAP_HAS_EXECUTION: u16 = 8;
const SNAP_HAS_ATTACHMENT: u16 = 16;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SeyalAppAction {
    pub version: u16,
    pub size: u16,
    pub kind: u16,
    pub flags: u16,
    pub fence_pane_lo: u64,
    pub fence_pane_hi: u64,
    pub fence_execution_lo: u64,
    pub fence_execution_hi: u64,
    pub fence_attachment_lo: u64,
    pub fence_attachment_hi: u64,
    pub fence_epoch: u64,
    pub target_execution_lo: u64,
    pub target_execution_hi: u64,
    pub target_attachment_lo: u64,
    pub target_attachment_hi: u64,
    pub target_pty_generation: u64,
    pub payload: *const u8,
    pub payload_len: u32,
    pub reserved: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SeyalAppSnapshot {
    pub version: u16,
    pub size: u16,
    pub eligibility: u16,
    pub flags: u16,
    pub generation: u64,
    pub pane_lo: u64,
    pub pane_hi: u64,
    pub execution_lo: u64,
    pub execution_hi: u64,
    pub attachment_lo: u64,
    pub attachment_hi: u64,
    pub epoch: u64,
    pub last_error: u32,
    pub pending_effect: u32,
    pub output_utf8: *const u8,
    pub output_utf8_len: u32,
    pub reserved: u32,
    pub recovery_stage: u16,
    pub recovery_attempts: u16,
    pub recovery_effect: u32,
    pub recovery_generation: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SeyalAppAxNode {
    pub id: u64,
    pub parent: u64,
    pub role: u8,
    pub enabled: u8,
    pub selected: u8,
    pub focused: u8,
    pub actions: u32,
    pub label: *const u8,
    pub label_len: u32,
    pub reserved0: u32,
    pub value: *const u8,
    pub value_len: u32,
    pub reserved1: u32,
    pub help: *const u8,
    pub help_len: u32,
    pub reserved2: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SeyalAppAccessibility {
    pub version: u16,
    pub size: u16,
    pub node_count: u32,
    pub nodes: *const SeyalAppAxNode,
    pub reserved: u32,
}

struct AppHandle {
    root: ApplicationRoot,
    output: Vec<u8>,
    ax_nodes: Vec<SeyalAppAxNode>,
    ax_text: Vec<u8>,
}

thread_local! {
    static APPS: RefCell<HashMap<u64, AppHandle>> = RefCell::new(HashMap::new());
}

impl SeyalAppSnapshot {
    const fn empty() -> Self {
        Self {
            version: APP_ABI_VERSION,
            size: 0,
            eligibility: 0,
            flags: 0,
            generation: 0,
            pane_lo: 0,
            pane_hi: 0,
            execution_lo: 0,
            execution_hi: 0,
            attachment_lo: 0,
            attachment_hi: 0,
            epoch: 0,
            last_error: 0,
            pending_effect: 0,
            output_utf8: ptr::null(),
            output_utf8_len: 0,
            reserved: 0,
            recovery_stage: 0,
            recovery_attempts: 0,
            recovery_effect: 0,
            recovery_generation: 0,
        }
    }
}

impl SeyalAppAccessibility {
    const fn empty() -> Self {
        Self {
            version: APP_ABI_VERSION,
            size: 0,
            node_count: 0,
            nodes: ptr::null(),
            reserved: 0,
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn seyal_app_create() -> u64 {
    let handle = allocate_handle();
    APPS.with(|apps| {
        apps.borrow_mut().insert(
            handle,
            AppHandle {
                root: ApplicationRoot::new(),
                output: Vec::new(),
                ax_nodes: Vec::new(),
                ax_text: Vec::new(),
            },
        );
    });
    handle
}

#[unsafe(no_mangle)]
pub extern "C" fn seyal_app_destroy(handle: u64) -> i32 {
    APPS.with(|apps| {
        if apps.borrow_mut().remove(&handle).is_some() {
            0
        } else {
            -1
        }
    })
}

/// Apply one versioned action to an explicit application-root handle.
///
/// # Safety
/// - `action` must be non-null and readable for `action.size` bytes.
/// - When `payload_len != 0`, `payload` must address that many readable bytes
///   for this call only.
/// - Pointers from a prior snapshot on this handle are invalidated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn seyal_app_apply(handle: u64, action: *const SeyalAppAction) -> i32 {
    if action.is_null() {
        return -5;
    }
    // SAFETY: caller supplies a readable action record for this call.
    let action = unsafe { &*action };
    if action.version != APP_ABI_VERSION {
        return -2;
    }
    if action.size as usize != size_of::<SeyalAppAction>() {
        return -3;
    }
    let decoded = match decode_action(action) {
        Ok(decoded) => decoded,
        Err(code) => return code,
    };
    APPS.with(|apps| {
        let mut apps = apps.borrow_mut();
        let Some(state) = apps.get_mut(&handle) else {
            return -1;
        };
        match state.root.apply(decoded) {
            Ok(()) => 0,
            Err(_) => -4,
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn seyal_app_snapshot(handle: u64) -> SeyalAppSnapshot {
    APPS.with(|apps| {
        let mut apps = apps.borrow_mut();
        let Some(state) = apps.get_mut(&handle) else {
            return SeyalAppSnapshot::empty();
        };
        let snap = state.root.snapshot();
        state.output = snap.output_utf8.as_bytes().to_vec();
        encode_snapshot(&snap, &state.output)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn seyal_app_accessibility(handle: u64) -> SeyalAppAccessibility {
    APPS.with(|apps| {
        let mut apps = apps.borrow_mut();
        let Some(state) = apps.get_mut(&handle) else {
            return SeyalAppAccessibility::empty();
        };
        let snap = state.root.snapshot();
        encode_accessibility(&snap, state)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn seyal_app_last_error(handle: u64) -> i32 {
    APPS.with(|apps| {
        apps.borrow()
            .get(&handle)
            .and_then(|state| state.root.snapshot().last_error)
            .map(error_number)
            .unwrap_or(0)
    })
}

fn decode_action(action: &SeyalAppAction) -> Result<AppAction, i32> {
    let fence = AppFence {
        pane: id16(action.fence_pane_lo, action.fence_pane_hi).map(PaneId::from_bytes)?,
        execution: optional_id(
            action.flags & FLAG_HAS_EXECUTION != 0,
            action.fence_execution_lo,
            action.fence_execution_hi,
        )?
        .map(ExecutionId::from_bytes),
        attachment: optional_id(
            action.flags & FLAG_HAS_ATTACHMENT != 0,
            action.fence_attachment_lo,
            action.fence_attachment_hi,
        )?
        .map(AttachmentId::from_bytes),
        controller: action.flags & FLAG_CONTROLLER != 0,
        presentation_epoch: action.fence_epoch,
    };
    match action.kind {
        0 => Ok(AppAction::Focus { fence }),
        1 => Ok(AppAction::Bind {
            fence,
            evidence: BindingEvidence {
                execution: ExecutionId::from_bytes(id16(
                    action.target_execution_lo,
                    action.target_execution_hi,
                )?),
                attachment: AttachmentId::from_bytes(id16(
                    action.target_attachment_lo,
                    action.target_attachment_hi,
                )?),
                controller: action.flags & FLAG_TARGET_CONTROLLER != 0,
                pty_generation: action.target_pty_generation,
                alternate_screen: action.flags & FLAG_ALTERNATE_SCREEN != 0,
            },
        }),
        2 => Ok(AppAction::Refresh { fence }),
        3 => {
            let text = read_payload(action.payload, action.payload_len)?;
            Ok(AppAction::SubmitInput { fence, text })
        }
        4 => Ok(AppAction::Quit),
        5 => Ok(AppAction::AckEffect),
        6 => Ok(AppAction::BeginRecovery {
            now: Duration::from_millis(action.target_pty_generation),
        }),
        7 => Ok(AppAction::CompleteRecovery {
            generation: action.target_execution_lo,
            outcome: decode_outcome(action.reserved, action.target_attachment_lo)?,
            now: Duration::from_millis(action.target_pty_generation),
            launch: decode_launch(action.reserved),
        }),
        8 => Ok(AppAction::FireScheduledRecovery {
            generation: action.target_execution_lo,
            now: Duration::from_millis(action.target_pty_generation),
        }),
        9 => Ok(AppAction::AckRecoveryEffect),
        _ => Err(-6),
    }
}

fn decode_outcome(reserved: u32, handle: u64) -> Result<AttemptOutcome, i32> {
    match reserved & 0xff {
        0 => Ok(AttemptOutcome::Connected),
        1 => Ok(AttemptOutcome::Opened {
            handle,
            adopted: true,
        }),
        2 => Ok(AttemptOutcome::Opened {
            handle,
            adopted: false,
        }),
        3 => Ok(AttemptOutcome::EndpointMissing),
        4 => Ok(AttemptOutcome::Retryable),
        5 => Ok(AttemptOutcome::ControllerBusy),
        6 => Ok(AttemptOutcome::Blocked),
        _ => Err(-6),
    }
}

fn decode_launch(reserved: u32) -> Option<LaunchResult> {
    match (reserved >> 8) & 0xff {
        1 => Some(LaunchResult::Started),
        2 => Some(LaunchResult::HelperMissing),
        _ => None,
    }
}

fn read_payload(ptr: *const u8, len: u32) -> Result<String, i32> {
    if len == 0 {
        return Ok(String::new());
    }
    if ptr.is_null() {
        return Err(-5);
    }
    let len = usize::try_from(len).map_err(|_| -6)?;
    // SAFETY: apply caller contract: readable for this call only.
    let bytes = unsafe { slice::from_raw_parts(ptr, len) };
    str::from_utf8(bytes).map(str::to_owned).map_err(|_| -6)
}

fn encode_snapshot(snap: &AppSnapshot, output: &[u8]) -> SeyalAppSnapshot {
    let pane = snap.pane.to_bytes();
    let execution = snap
        .execution
        .unwrap_or(ExecutionId::from_bytes([0; 16]))
        .to_bytes();
    let attachment = snap
        .attachment
        .unwrap_or(AttachmentId::from_bytes([0; 16]))
        .to_bytes();
    let mut flags = 0;
    if snap.composer_eligible {
        flags |= SNAP_COMPOSER;
    }
    if snap.controller {
        flags |= SNAP_CONTROLLER;
    }
    if snap.frozen {
        flags |= SNAP_FROZEN;
    }
    if snap.execution.is_some() {
        flags |= SNAP_HAS_EXECUTION;
    }
    if snap.attachment.is_some() {
        flags |= SNAP_HAS_ATTACHMENT;
    }
    SeyalAppSnapshot {
        version: APP_ABI_VERSION,
        size: size_of::<SeyalAppSnapshot>() as u16,
        eligibility: match snap.eligibility {
            PresentationEligibility::Unbound => 0,
            PresentationEligibility::Flow => 1,
            PresentationEligibility::Raw => 2,
            PresentationEligibility::Tui => 3,
        },
        flags,
        generation: snap.generation,
        pane_lo: u64::from_le_bytes(pane[..8].try_into().unwrap()),
        pane_hi: u64::from_le_bytes(pane[8..].try_into().unwrap()),
        execution_lo: u64::from_le_bytes(execution[..8].try_into().unwrap()),
        execution_hi: u64::from_le_bytes(execution[8..].try_into().unwrap()),
        attachment_lo: u64::from_le_bytes(attachment[..8].try_into().unwrap()),
        attachment_hi: u64::from_le_bytes(attachment[8..].try_into().unwrap()),
        epoch: snap.presentation_epoch,
        last_error: snap.last_error.map(error_number).unwrap_or(0) as u32,
        pending_effect: match snap.pending_effect {
            NativeEffect::None => 0,
            NativeEffect::BoundedDetachThenTerminate => 1,
        },
        output_utf8: if output.is_empty() {
            ptr::null()
        } else {
            output.as_ptr()
        },
        output_utf8_len: output.len() as u32,
        reserved: 0,
        recovery_stage: match snap.recovery_stage {
            RecoveryStage::Disconnected => 0,
            RecoveryStage::Discovering => 1,
            RecoveryStage::StartingRuntime => 2,
            RecoveryStage::WaitingForController => 3,
            RecoveryStage::Reconstructing => 4,
            RecoveryStage::RestoringInteraction => 5,
            RecoveryStage::Usable => 6,
            RecoveryStage::Exhausted => 7,
            RecoveryStage::Blocked => 8,
        },
        recovery_attempts: snap.recovery_attempts.min(u32::from(u16::MAX)) as u16,
        recovery_effect: match snap.recovery_effect {
            None => 0,
            Some(RecoveryEffect::PerformAttempt { .. }) => 1,
            Some(RecoveryEffect::Schedule { .. }) => 2,
            Some(RecoveryEffect::LaunchHelper { .. }) => 3,
            Some(RecoveryEffect::DisposeHandle(_)) => 4,
        },
        recovery_generation: snap.recovery_generation,
    }
}

fn encode_accessibility(snap: &AppSnapshot, state: &mut AppHandle) -> SeyalAppAccessibility {
    state.ax_text.clear();
    state.ax_nodes.clear();
    let mut nodes = Vec::with_capacity(snap.accessibility.len());
    for node in &snap.accessibility {
        let label = push_text(&mut state.ax_text, &node.label);
        let value = push_text(&mut state.ax_text, &node.value);
        let help = push_text(&mut state.ax_text, &node.help);
        nodes.push((node, label, value, help));
    }
    let base = state.ax_text.as_ptr();
    state.ax_nodes = nodes
        .into_iter()
        .map(|(node, label, value, help)| SeyalAppAxNode {
            id: node.id,
            parent: node.parent.unwrap_or(0),
            role: match node.role {
                crate::app::AccessibilityRole::Application => 0,
                crate::app::AccessibilityRole::Pane => 1,
                crate::app::AccessibilityRole::Composer => 2,
                crate::app::AccessibilityRole::Terminal => 3,
            },
            enabled: u8::from(node.enabled),
            selected: u8::from(node.selected),
            focused: u8::from(node.focused),
            actions: node.actions,
            // SAFETY: offsets were recorded into `ax_text` on this handle.
            label: unsafe { base.add(label.0) },
            label_len: label.1,
            reserved0: 0,
            value: unsafe { base.add(value.0) },
            value_len: value.1,
            reserved1: 0,
            help: unsafe { base.add(help.0) },
            help_len: help.1,
            reserved2: 0,
        })
        .collect();
    SeyalAppAccessibility {
        version: APP_ABI_VERSION,
        size: size_of::<SeyalAppAccessibility>() as u16,
        node_count: state.ax_nodes.len() as u32,
        nodes: state.ax_nodes.as_ptr(),
        reserved: 0,
    }
}

fn push_text(buf: &mut Vec<u8>, text: &str) -> (usize, u32) {
    let start = buf.len();
    buf.extend_from_slice(text.as_bytes());
    buf.push(0);
    (start, text.len() as u32)
}

fn id16(lo: u64, hi: u64) -> Result<[u8; 16], i32> {
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&lo.to_le_bytes());
    bytes[8..].copy_from_slice(&hi.to_le_bytes());
    Ok(bytes)
}

fn optional_id(present: bool, lo: u64, hi: u64) -> Result<Option<[u8; 16]>, i32> {
    if present {
        Ok(Some(id16(lo, hi)?))
    } else {
        Ok(None)
    }
}

fn error_number(error: AppError) -> i32 {
    match error {
        AppError::UnknownPane => 1,
        AppError::StalePane => 2,
        AppError::StaleExecution => 3,
        AppError::StaleAttachment => 4,
        AppError::StaleController => 5,
        AppError::StalePresentationEpoch => 6,
        AppError::UnboundUnauthorized => 7,
        AppError::AlreadyBound => 8,
        AppError::NotController => 9,
        AppError::DirectInputUnauthorized => 10,
        AppError::ZeroPtyGeneration => 11,
        AppError::Frozen => 12,
        AppError::NoLiveClient => 13,
        AppError::InvalidPayload => 14,
        AppError::StaleRecoveryGeneration => 15,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, offset_of, size_of};

    fn fence_action(kind: u16, root: &ApplicationRoot) -> SeyalAppAction {
        let fence = root.fence();
        let pane = fence.pane.to_bytes();
        SeyalAppAction {
            version: APP_ABI_VERSION,
            size: size_of::<SeyalAppAction>() as u16,
            kind,
            flags: 0,
            fence_pane_lo: u64::from_le_bytes(pane[..8].try_into().unwrap()),
            fence_pane_hi: u64::from_le_bytes(pane[8..].try_into().unwrap()),
            fence_execution_lo: 0,
            fence_execution_hi: 0,
            fence_attachment_lo: 0,
            fence_attachment_hi: 0,
            fence_epoch: fence.presentation_epoch,
            target_execution_lo: 0,
            target_execution_hi: 0,
            target_attachment_lo: 0,
            target_attachment_hi: 0,
            target_pty_generation: 0,
            payload: ptr::null(),
            payload_len: 0,
            reserved: 0,
        }
    }

    #[test]
    fn action_and_snapshot_match_published_sizes() {
        assert_eq!(size_of::<SeyalAppAction>(), 120);
        assert_eq!(align_of::<SeyalAppAction>(), 8);
        assert_eq!(offset_of!(SeyalAppAction, version), 0);
        assert_eq!(offset_of!(SeyalAppAction, payload), 104);
        assert_eq!(size_of::<SeyalAppSnapshot>(), 112);
        assert_eq!(offset_of!(SeyalAppSnapshot, output_utf8), 80);
        assert_eq!(offset_of!(SeyalAppSnapshot, recovery_generation), 104);
        assert_eq!(size_of::<SeyalAppAxNode>(), 72);
        assert_eq!(size_of::<SeyalAppAccessibility>(), 24);
    }

    #[test]
    fn explicit_handle_round_trip_and_unknown_handle_fail_closed() {
        let handle = seyal_app_create();
        assert_ne!(handle, 0);
        let snap = seyal_app_snapshot(handle);
        assert_eq!(snap.version, APP_ABI_VERSION);
        assert_eq!(snap.eligibility, 0);
        assert_eq!(unsafe { seyal_app_apply(u64::MAX, ptr::null()) }, -5);
        assert_eq!(seyal_app_destroy(handle), 0);
        assert_eq!(seyal_app_destroy(handle), -1);
        let missing = seyal_app_snapshot(handle);
        assert_eq!(missing.generation, 0);
    }

    #[test]
    fn version_and_size_mismatch_fail_closed() {
        let handle = seyal_app_create();
        let mut action = fence_action(0, &ApplicationRoot::new());
        action.version = 99;
        assert_eq!(unsafe { seyal_app_apply(handle, &action) }, -2);
        action.version = APP_ABI_VERSION;
        action.size = 4;
        assert_eq!(unsafe { seyal_app_apply(handle, &action) }, -3);
        assert_eq!(seyal_app_destroy(handle), 0);
    }

    #[test]
    fn bind_through_explicit_handle_does_not_use_implicit_select() {
        let handle = seyal_app_create();
        let snap = seyal_app_snapshot(handle);
        let mut action = SeyalAppAction {
            version: APP_ABI_VERSION,
            size: size_of::<SeyalAppAction>() as u16,
            kind: 1,
            flags: FLAG_TARGET_CONTROLLER,
            fence_pane_lo: snap.pane_lo,
            fence_pane_hi: snap.pane_hi,
            fence_execution_lo: 0,
            fence_execution_hi: 0,
            fence_attachment_lo: 0,
            fence_attachment_hi: 0,
            fence_epoch: snap.epoch,
            target_execution_lo: 1,
            target_execution_hi: 0,
            target_attachment_lo: 2,
            target_attachment_hi: 0,
            target_pty_generation: 1,
            payload: ptr::null(),
            payload_len: 0,
            reserved: 0,
        };
        assert_eq!(unsafe { seyal_app_apply(handle, &action) }, 0);
        let bound = seyal_app_snapshot(handle);
        assert_eq!(bound.eligibility, 1);
        assert_eq!(bound.flags & SNAP_COMPOSER, SNAP_COMPOSER);
        action.kind = 0;
        assert_eq!(unsafe { seyal_app_apply(handle, &action) }, -4);
        assert_eq!(seyal_app_last_error(handle), 3);
        assert_eq!(seyal_app_destroy(handle), 0);
    }
}
