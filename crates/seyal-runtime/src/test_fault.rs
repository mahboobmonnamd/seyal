//! Test-only deterministic failure injection for Pass-5 resource rollback.
//!
//! This module is compiled only when the non-default `test-fault-injection`
//! feature is enabled. Normal/production Seyal builds contain neither the
//! state nor the branch checks that consume it.

use std::cell::Cell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaultPoint {
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

/// Injects `point` into the next matching operation on the current thread.
/// Setting a new point replaces any unconsumed point from the same test.
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
