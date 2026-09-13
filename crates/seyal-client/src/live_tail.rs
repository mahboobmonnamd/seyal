//! Flow live-tail history projection (#865).
//!
//! Runtime `BlockTimeline` remains the Block authority. This module only
//! derives the inclusive canonical history span a Flow compositor may request.
//! Open-ended running tails use `end_line == u64::MAX`. Hosts must not invent a
//! second range (for example `start + 511`).

use crate::presentation::PresentationMode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HistorySpan {
    pub start_line: u64,
    pub end_line: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiveTailProjection {
    /// Inclusive canonical history span. `end_line == u64::MAX` is an open
    /// running tail; completed Blocks use a trusted finite end.
    Span(HistorySpan),
    /// Invalid, stale, or non-Flow evidence. Draw nothing for this Block.
    FailClosed,
}

/// Project one Block's history request for the current presentation.
///
/// `end_line` is the Runtime-trusted completed end, or `None` while running.
pub fn project_block_history(
    mode: PresentationMode,
    start_line: u64,
    end_line: Option<u64>,
    running: bool,
) -> LiveTailProjection {
    if mode != PresentationMode::Flow || start_line == 0 {
        return LiveTailProjection::FailClosed;
    }
    match (running, end_line) {
        (true, None) => LiveTailProjection::Span(HistorySpan {
            start_line,
            end_line: u64::MAX,
        }),
        (false, Some(end)) if end >= start_line => LiveTailProjection::Span(HistorySpan {
            start_line,
            end_line: end,
        }),
        _ => LiveTailProjection::FailClosed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn running_flow_block_is_open_ended() {
        assert_eq!(
            project_block_history(PresentationMode::Flow, 10, None, true),
            LiveTailProjection::Span(HistorySpan {
                start_line: 10,
                end_line: u64::MAX,
            })
        );
    }

    #[test]
    fn seq_one_to_one_thousand_stays_one_open_span() {
        let first = project_block_history(PresentationMode::Flow, 20, None, true);
        let after_many_lines = project_block_history(PresentationMode::Flow, 20, None, true);
        assert_eq!(first, after_many_lines);
        assert_eq!(
            first,
            LiveTailProjection::Span(HistorySpan {
                start_line: 20,
                end_line: u64::MAX,
            })
        );
    }

    #[test]
    fn completion_hands_off_to_trusted_end() {
        let running = project_block_history(PresentationMode::Flow, 20, None, true);
        let completed = project_block_history(PresentationMode::Flow, 20, Some(1019), false);
        assert_ne!(running, completed);
        assert_eq!(
            completed,
            LiveTailProjection::Span(HistorySpan {
                start_line: 20,
                end_line: 1019,
            })
        );
    }

    #[test]
    fn raw_and_tui_fail_closed() {
        assert_eq!(
            project_block_history(PresentationMode::Raw, 10, None, true),
            LiveTailProjection::FailClosed
        );
        assert_eq!(
            project_block_history(PresentationMode::Tui, 10, Some(12), false),
            LiveTailProjection::FailClosed
        );
    }

    #[test]
    fn stale_or_conflicting_evidence_fail_closed() {
        assert_eq!(
            project_block_history(PresentationMode::Flow, 0, None, true),
            LiveTailProjection::FailClosed
        );
        assert_eq!(
            project_block_history(PresentationMode::Flow, 10, Some(9), false),
            LiveTailProjection::FailClosed
        );
        assert_eq!(
            project_block_history(PresentationMode::Flow, 10, Some(15), true),
            LiveTailProjection::FailClosed
        );
        assert_eq!(
            project_block_history(PresentationMode::Flow, 10, None, false),
            LiveTailProjection::FailClosed
        );
    }

    #[test]
    fn identical_running_projections_coalesce() {
        let a = project_block_history(PresentationMode::Flow, 3, None, true);
        let b = project_block_history(PresentationMode::Flow, 3, None, true);
        assert_eq!(a, b);
    }
}
