use std::collections::HashMap;

use seyal_protocol::pass8::{
    BlockKind as WireBlockKind, BlockLifecycle as WireBlockLifecycle, BlockState as WireBlockState,
};

#[cfg(feature = "test-fault-injection")]
use crate::test_fault::{self, FaultPoint};
use crate::{BlockId, ExecutionId, WorkspaceId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockLifecycle {
    Current,
    Completed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockSummary {
    pub id: BlockId,
    pub workspace_id: WorkspaceId,
    pub execution_id: ExecutionId,
    pub start_line_id: u64,
    pub lifecycle: BlockLifecycle,
    pub revision: u64,
}

impl BlockSummary {
    pub(crate) fn to_wire(self) -> WireBlockState {
        #[cfg(feature = "test-fault-injection")]
        let revision = if self.lifecycle == BlockLifecycle::Completed
            && test_fault::take(FaultPoint::BlockCompletionEncode)
        {
            0
        } else {
            self.revision
        };
        #[cfg(not(feature = "test-fault-injection"))]
        let revision = self.revision;

        WireBlockState {
            execution_id: self.execution_id,
            block_id: self.id,
            revision,
            start_line_id: self.start_line_id,
            kind: WireBlockKind::TerminalActivity,
            state: match self.lifecycle {
                BlockLifecycle::Current => WireBlockLifecycle::Current,
                BlockLifecycle::Completed => WireBlockLifecycle::Completed,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BlockTimelineError {
    DuplicateExecution,
    InvalidAnchor,
    OwnershipMismatch,
    InvalidTransition,
    InjectedFailure,
}

#[derive(Default)]
pub(crate) struct BlockTimeline {
    records: HashMap<ExecutionId, BlockSummary>,
}

impl BlockTimeline {
    pub(crate) fn len(&self) -> usize {
        self.records.len()
    }

    pub(crate) fn get(&self, execution_id: ExecutionId) -> Option<BlockSummary> {
        self.records.get(&execution_id).copied()
    }

    pub(crate) fn admit(
        &mut self,
        workspace_id: WorkspaceId,
        execution_id: ExecutionId,
        start_line_id: u64,
    ) -> Result<BlockSummary, BlockTimelineError> {
        #[cfg(feature = "test-fault-injection")]
        if test_fault::take(FaultPoint::BlockAdmission) {
            return Err(BlockTimelineError::InjectedFailure);
        }
        if start_line_id == 0 {
            return Err(BlockTimelineError::InvalidAnchor);
        }
        if self.records.contains_key(&execution_id) {
            return Err(BlockTimelineError::DuplicateExecution);
        }
        let record = BlockSummary {
            id: BlockId::new(),
            workspace_id,
            execution_id,
            start_line_id,
            lifecycle: BlockLifecycle::Current,
            revision: 1,
        };
        self.records.insert(execution_id, record);
        Ok(record)
    }

    pub(crate) fn complete(
        &mut self,
        workspace_id: WorkspaceId,
        execution_id: ExecutionId,
    ) -> Result<Option<BlockSummary>, BlockTimelineError> {
        #[cfg(feature = "test-fault-injection")]
        if test_fault::take(FaultPoint::BlockCompletionMutation) {
            return Err(BlockTimelineError::InjectedFailure);
        }
        let Some(record) = self.records.get_mut(&execution_id) else {
            return Ok(None);
        };
        if record.workspace_id != workspace_id || record.execution_id != execution_id {
            return Err(BlockTimelineError::OwnershipMismatch);
        }
        if record.lifecycle != BlockLifecycle::Current || record.revision != 1 {
            return Err(BlockTimelineError::InvalidTransition);
        }
        record.lifecycle = BlockLifecycle::Completed;
        record.revision = 2;
        Ok(Some(*record))
    }

    pub(crate) fn retire(&mut self, execution_id: ExecutionId) -> Option<BlockSummary> {
        self.records.remove(&execution_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn execution(value: u128) -> ExecutionId {
        ExecutionId::from_bytes(value.to_le_bytes())
    }

    #[test]
    fn timeline_admits_one_workspace_owned_block_and_completes_monotonically() {
        let workspace = WorkspaceId::m001_default();
        let execution = execution(7);
        let mut timeline = BlockTimeline::default();

        let current = timeline.admit(workspace, execution, 1).unwrap();
        assert_eq!(timeline.len(), 1);
        assert_eq!(current.workspace_id, workspace);
        assert_eq!(current.execution_id, execution);
        assert_eq!(current.start_line_id, 1);
        assert_eq!(current.lifecycle, BlockLifecycle::Current);
        assert_eq!(current.revision, 1);
        assert_ne!(current.id.to_bytes(), [0; 16]);

        let completed = timeline.complete(workspace, execution).unwrap().unwrap();
        assert_eq!(completed.id, current.id);
        assert_eq!(completed.start_line_id, current.start_line_id);
        assert_eq!(completed.lifecycle, BlockLifecycle::Completed);
        assert_eq!(completed.revision, 2);
        assert_eq!(
            timeline.complete(workspace, execution),
            Err(BlockTimelineError::InvalidTransition)
        );
    }

    #[test]
    fn timeline_rejects_duplicate_execution_invalid_anchor_and_wrong_workspace() {
        let workspace = WorkspaceId::m001_default();
        let execution = execution(9);
        let mut timeline = BlockTimeline::default();

        assert_eq!(
            timeline.admit(workspace, execution, 0),
            Err(BlockTimelineError::InvalidAnchor)
        );
        timeline.admit(workspace, execution, 3).unwrap();
        assert_eq!(
            timeline.admit(workspace, execution, 3),
            Err(BlockTimelineError::DuplicateExecution)
        );
        let wrong_workspace = WorkspaceId::from_bytes(99u128.to_le_bytes());
        assert_eq!(
            timeline.complete(wrong_workspace, execution),
            Err(BlockTimelineError::OwnershipMismatch)
        );
    }

    #[test]
    fn retirement_removes_completed_history_immediately() {
        let workspace = WorkspaceId::m001_default();
        let execution = execution(11);
        let mut timeline = BlockTimeline::default();
        let current = timeline.admit(workspace, execution, 4).unwrap();
        let completed = timeline.complete(workspace, execution).unwrap().unwrap();
        assert_eq!(completed.id, current.id);
        assert_eq!(timeline.retire(execution), Some(completed));
        assert_eq!(timeline.len(), 0);
        assert_eq!(timeline.get(execution), None);
    }
}
