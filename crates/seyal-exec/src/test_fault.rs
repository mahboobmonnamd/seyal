//! Deterministic test-only faults for platform endpoint boundaries.

use std::cell::Cell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaultPoint {
    /// Fail the next N PTY winsize syscalls before canonical resize commit.
    ResizeWinsize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FaultState {
    point: FaultPoint,
    remaining: usize,
}

thread_local! {
    static NEXT_FAULT: Cell<Option<FaultState>> = const { Cell::new(None) };
}

pub fn fail_times(point: FaultPoint, count: usize) {
    NEXT_FAULT.with(|slot| {
        slot.set((count > 0).then_some(FaultState {
            point,
            remaining: count,
        }))
    });
}

pub fn remaining(point: FaultPoint) -> usize {
    NEXT_FAULT.with(|slot| match slot.get() {
        Some(state) if state.point == point => state.remaining,
        _ => 0,
    })
}

pub(crate) fn take(point: FaultPoint) -> bool {
    NEXT_FAULT.with(|slot| match slot.get() {
        Some(mut state) if state.point == point && state.remaining > 0 => {
            state.remaining -= 1;
            slot.set((state.remaining > 0).then_some(state));
            true
        }
        _ => false,
    })
}
