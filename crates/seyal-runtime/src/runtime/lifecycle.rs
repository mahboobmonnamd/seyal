use std::time::{Duration, Instant};

use seyal_exec::ChildExit;

use super::config::{TERMINATION_FAILED_REAP_LIMIT, TERMINATION_FAILED_RETRY_INITIAL, TERMINATION_FAILED_RETRY_MAX};
use crate::BlockSummary;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutionLifecycle {
    Running,
    TerminatingGraceful,
    TerminatingForced,
    /// The kernel has confirmed the primary process exited (`NOTE_EXIT`),
    /// but the reap (`waitpid`) has not yet completed. Retried on a short
    /// deadline with a hard attempt bound; never an unbounded terminal state.
    PrimaryExitPending,
    DrainingAfterPrimaryExit,
    /// Forced reap deadline was exceeded. Ownership is retained and a bounded
    /// retry/signalling path remains until reap succeeds or the operator
    /// re-arms termination.
    TerminationFailed,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum BlockCompletion {
    None,
    Completed(BlockSummary),
    Failed,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Lifecycle {
    Running,
    TerminatingGraceful {
        deadline: Instant,
    },
    TerminatingForced {
        deadline: Instant,
    },
    /// See `ExecutionLifecycle::PrimaryExitPending`.
    PrimaryExitPending {
        deadline: Instant,
        remaining: u8,
    },
    DrainingAfterPrimaryExit {
        deadline: Instant,
        exit: ChildExit,
    },
    /// See `ExecutionLifecycle::TerminationFailed`.
    TerminationFailed {
        deadline: Instant,
        delay: Duration,
        remaining: u8,
    },
}

impl Lifecycle {
    pub(super) fn public(self) -> ExecutionLifecycle {
        match self {
            Self::Running => ExecutionLifecycle::Running,
            Self::TerminatingGraceful { .. } => ExecutionLifecycle::TerminatingGraceful,
            Self::TerminatingForced { .. } => ExecutionLifecycle::TerminatingForced,
            Self::PrimaryExitPending { .. } => ExecutionLifecycle::PrimaryExitPending,
            Self::DrainingAfterPrimaryExit { .. } => ExecutionLifecycle::DrainingAfterPrimaryExit,
            Self::TerminationFailed { .. } => ExecutionLifecycle::TerminationFailed,
        }
    }

    pub(super) fn deadline(self) -> Option<Instant> {
        match self {
            Self::TerminatingGraceful { deadline }
            | Self::TerminatingForced { deadline }
            | Self::PrimaryExitPending { deadline, .. }
            | Self::DrainingAfterPrimaryExit { deadline, .. }
            | Self::TerminationFailed { deadline, .. } => Some(deadline),
            Self::Running => None,
        }
    }

    pub(super) fn accepts_input(self) -> bool {
        matches!(self, Self::Running)
    }

    pub(super) fn enter_termination_failed(now: Instant) -> Self {
        Self::TerminationFailed {
            deadline: now + TERMINATION_FAILED_RETRY_INITIAL,
            delay: TERMINATION_FAILED_RETRY_INITIAL,
            remaining: TERMINATION_FAILED_REAP_LIMIT,
        }
    }

    pub(super) fn advance_termination_failed(self, now: Instant) -> Self {
        match self {
            Self::TerminationFailed {
                delay, remaining, ..
            } => {
                let next_delay = delay.saturating_mul(2).min(TERMINATION_FAILED_RETRY_MAX);
                let next_remaining = remaining.saturating_sub(1);
                Self::TerminationFailed {
                    deadline: now + next_delay,
                    delay: next_delay,
                    // When the attempt budget is exhausted, keep ownership and
                    // continue at the max backoff so shutdown cannot hot-spin
                    // and `request_termination` can still re-arm recovery.
                    remaining: if next_remaining == 0 {
                        0
                    } else {
                        next_remaining
                    },
                }
            }
            other => other,
        }
    }
}
