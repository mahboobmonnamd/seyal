//! Test-only deterministic failure injection for Pass-5 rollback paths.
//!
//! This module is compiled only with `test-fault-injection`. Legacy shared-
//! projection points remain solely for isolated comparator/reference tests;
//! Candidate-D production faults exercise bounded attach admission/flush.

use std::cell::Cell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaultPoint {
    // Candidate-D production attachment transaction.
    AttachAdmission,
    AttachFlush,
    // Legacy Candidate-B comparator/reference resource lifecycle.
    ShmOpenWriter,
    Truncate,
    MmapWriter,
    ShmOpenReader,
    ShmUnlink,
    SendAttachedDescriptor,
}

thread_local! {
    static NEXT_FAULT: Cell<Option<FaultPoint>> = const { Cell::new(None) };
}

pub fn fail_next(point: FaultPoint) {
    NEXT_FAULT.with(|slot| slot.set(Some(point)));
}

pub(crate) fn take(point: FaultPoint) -> bool {
    NEXT_FAULT.with(|slot| {
        if slot.get() == Some(point) {
            slot.set(None);
            true
        } else {
            false
        }
    })
}
