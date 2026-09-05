//! Runtime-owned per-execution composer command Block timeline.
//!
//! Owns `CAP_COMMAND_BLOCKS` anchors and lifecycle truth only. It never
//! receives terminal cells, parses prompts, or owns a PTY/renderer.
//! Distinct from `activity_block_timeline` (Pass-8 TerminalActivity metadata).

use std::collections::VecDeque;

// Keep the replacement projection within one MAX_FRAME_PAYLOAD frame while
// retaining the full bounded command text for every record.
pub(crate) const MAX_BLOCKS_PER_EXECUTION: usize = 128;
pub(crate) const MAX_COMMAND_BYTES: usize = 16 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct CommandBlockId(u64);

impl CommandBlockId {
    pub(crate) const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CommandBlockLifecycle {
    Running,
    Completed { exit_status: i32 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CommandBlockRecord {
    pub(crate) id: CommandBlockId,
    pub(crate) command: String,
    pub(crate) start_line: u64,
    pub(crate) end_line: Option<u64>,
    pub(crate) lifecycle: CommandBlockLifecycle,
}

/// Bounded, append-only logical timeline for one `ExecutionId`.
///
/// Callers must invoke `start` only after trusted shell integration accepts a
/// complete composer command and returns its canonical primary-history anchor.
#[derive(Default)]
pub(crate) struct CommandBlockTimeline {
    next_id: u64,
    records: VecDeque<CommandBlockRecord>,
}

impl CommandBlockTimeline {
    pub(crate) fn start(
        &mut self,
        command: String,
        start_line: u64,
    ) -> Result<CommandBlockId, CommandBlockTimelineError> {
        if command.is_empty() || command.len() > MAX_COMMAND_BYTES {
            return Err(CommandBlockTimelineError::InvalidCommand);
        }
        if self.records.len() == MAX_BLOCKS_PER_EXECUTION {
            // Retain active work and roll the oldest completed projection out
            // of the disposable timeline. Runtime lifecycle truth remains in
            // the execution, so eviction never invalidates a running block.
            let Some(index) = self
                .records
                .iter()
                .position(|record| matches!(record.lifecycle, CommandBlockLifecycle::Completed { .. }))
            else {
                return Err(CommandBlockTimelineError::Capacity);
            };
            self.records.remove(index);
        }
        let id = CommandBlockId(
            self.next_id
                .checked_add(1)
                .ok_or(CommandBlockTimelineError::Exhausted)?,
        );
        self.next_id = id.raw();
        self.records.push_back(CommandBlockRecord {
            id,
            command,
            start_line,
            end_line: None,
            lifecycle: CommandBlockLifecycle::Running,
        });
        Ok(id)
    }

    pub(crate) fn complete(
        &mut self,
        id: CommandBlockId,
        end_line: u64,
        exit_status: i32,
    ) -> Result<(), CommandBlockTimelineError> {
        let record = self
            .records
            .iter_mut()
            .find(|record| record.id == id)
            .ok_or(CommandBlockTimelineError::UnknownBlock)?;
        if record.lifecycle != CommandBlockLifecycle::Running || end_line < record.start_line {
            return Err(CommandBlockTimelineError::InvalidCompletion);
        }
        record.end_line = Some(end_line);
        record.lifecycle = CommandBlockLifecycle::Completed { exit_status };
        Ok(())
    }

    pub(crate) fn records(&self) -> impl ExactSizeIterator<Item = &CommandBlockRecord> {
        self.records.iter()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CommandBlockTimelineError {
    InvalidCommand,
    Capacity,
    Exhausted,
    UnknownBlock,
    InvalidCompletion,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_one_ordered_record_per_accepted_command() {
        let mut timeline = CommandBlockTimeline::default();
        let first = timeline.start("printf one".into(), 41).unwrap();
        let second = timeline.start("printf two".into(), 44).unwrap();

        assert_ne!(first, second);
        assert_eq!(
            timeline
                .records()
                .map(|record| record.command.as_str())
                .collect::<Vec<_>>(),
            ["printf one", "printf two"]
        );
        assert!(
            timeline
                .records()
                .all(|record| record.lifecycle == CommandBlockLifecycle::Running)
        );
    }

    #[test]
    fn only_runtime_completion_can_close_the_matching_running_record() {
        let mut timeline = CommandBlockTimeline::default();
        let id = timeline.start("false".into(), 5).unwrap();
        assert_eq!(
            timeline.complete(id, 4, 1),
            Err(CommandBlockTimelineError::InvalidCompletion)
        );
        timeline.complete(id, 7, 1).unwrap();
        let record = timeline.records().next().unwrap();
        assert_eq!(record.end_line, Some(7));
        assert_eq!(
            record.lifecycle,
            CommandBlockLifecycle::Completed { exit_status: 1 }
        );
    }

    #[test]
    fn completed_records_roll_forward_without_evicting_active_work() {
        let mut timeline = CommandBlockTimeline::default();
        let mut first = None;
        for index in 0..MAX_BLOCKS_PER_EXECUTION {
            let id = timeline
                .start(format!("printf {index}"), index as u64 + 1)
                .unwrap();
            timeline.complete(id, index as u64 + 2, 0).unwrap();
            first.get_or_insert(id);
        }
        let active = timeline.start("printf active".into(), 10_000).unwrap();
        assert!(timeline.records().any(|record| record.id == active));
        assert!(!timeline.records().any(|record| Some(record.id) == first));
    }
}
