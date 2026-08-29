use std::{
    collections::HashSet,
    sync::{Mutex, OnceLock},
};

use seyal_protocol::{
    ExecutionId,
    pass8::{BlockLifecycle, BlockState},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BlockApply {
    Applied,
    Duplicate,
    Stale,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BlockCacheError {
    Quarantined,
    Conflict,
}

#[derive(Debug, Default)]
pub(crate) struct BlockCache {
    accepted: Option<BlockState>,
    quarantined: bool,
}

impl BlockCache {
    pub(crate) fn visible(&self) -> Option<BlockState> {
        (!self.quarantined).then_some(self.accepted).flatten()
    }

    pub(crate) fn apply(
        &mut self,
        expected_execution: ExecutionId,
        incoming: BlockState,
    ) -> Result<BlockApply, BlockCacheError> {
        if self.quarantined {
            return Err(BlockCacheError::Quarantined);
        }
        if incoming.execution_id != expected_execution {
            return self.conflict();
        }

        let Some(current) = self.accepted else {
            if incoming.state != BlockLifecycle::Current || incoming.revision != 1 {
                return self.conflict();
            }
            self.accepted = Some(incoming);
            return Ok(BlockApply::Applied);
        };

        if incoming.block_id != current.block_id
            || incoming.execution_id != current.execution_id
            || incoming.start_line_id != current.start_line_id
            || incoming.kind != current.kind
        {
            return self.conflict();
        }
        if current.state == BlockLifecycle::Completed && incoming.state == BlockLifecycle::Current {
            return self.conflict();
        }
        if incoming.revision < current.revision {
            return Ok(BlockApply::Stale);
        }
        if incoming.revision == current.revision {
            if incoming == current {
                return Ok(BlockApply::Duplicate);
            }
            return self.conflict();
        }
        if current.state == BlockLifecycle::Current
            && current.revision == 1
            && incoming.state == BlockLifecycle::Completed
            && incoming.revision == 2
        {
            self.accepted = Some(incoming);
            return Ok(BlockApply::Applied);
        }
        self.conflict()
    }

    pub(crate) fn quarantine(&mut self) {
        self.quarantined = true;
    }

    fn conflict<T>(&mut self) -> Result<T, BlockCacheError> {
        self.quarantine();
        Err(BlockCacheError::Conflict)
    }
}

fn quarantine_set() -> &'static Mutex<HashSet<(u128, ExecutionId)>> {
    static QUARANTINED: OnceLock<Mutex<HashSet<(u128, ExecutionId)>>> = OnceLock::new();
    QUARANTINED.get_or_init(|| Mutex::new(HashSet::new()))
}

pub(crate) fn quarantine_epoch(runtime_id: u128, execution_id: ExecutionId) {
    if let Ok(mut values) = quarantine_set().lock() {
        values.insert((runtime_id, execution_id));
    }
}

pub(crate) fn is_epoch_quarantined(runtime_id: u128, execution_id: ExecutionId) -> bool {
    quarantine_set()
        .lock()
        .is_ok_and(|values| values.contains(&(runtime_id, execution_id)))
}

#[cfg(test)]
mod tests {
    use seyal_protocol::{
        BlockId,
        pass8::{BlockKind, BlockLifecycle},
    };

    use super::*;

    fn execution(value: u128) -> ExecutionId {
        ExecutionId::from_bytes(value.to_le_bytes())
    }

    fn block(value: u128) -> BlockId {
        BlockId::from_bytes(value.to_le_bytes())
    }

    fn current() -> BlockState {
        BlockState {
            execution_id: execution(1),
            block_id: block(2),
            revision: 1,
            start_line_id: 3,
            kind: BlockKind::TerminalActivity,
            state: BlockLifecycle::Current,
        }
    }

    fn completed() -> BlockState {
        BlockState {
            revision: 2,
            state: BlockLifecycle::Completed,
            ..current()
        }
    }

    #[test]
    fn cache_accepts_current_completed_and_idempotent_duplicates() {
        let mut cache = BlockCache::default();
        assert_eq!(
            cache.apply(execution(1), current()),
            Ok(BlockApply::Applied)
        );
        assert_eq!(
            cache.apply(execution(1), current()),
            Ok(BlockApply::Duplicate)
        );
        assert_eq!(
            cache.apply(execution(1), completed()),
            Ok(BlockApply::Applied)
        );
        assert_eq!(cache.visible(), Some(completed()));
        assert_eq!(
            cache.apply(execution(1), completed()),
            Ok(BlockApply::Duplicate)
        );
    }

    #[test]
    fn stale_metadata_does_not_replace_committed_cache() {
        let mut cache = BlockCache::default();
        cache.apply(execution(1), current()).unwrap();
        cache.apply(execution(1), completed()).unwrap();
        let mut stale = completed();
        stale.revision = 1;
        assert_eq!(cache.apply(execution(1), stale), Ok(BlockApply::Stale));
        assert_eq!(cache.visible(), Some(completed()));
    }

    #[test]
    fn completed_to_current_regression_quarantines_even_when_revision_is_lower() {
        let mut cache = BlockCache::default();
        cache.apply(execution(1), current()).unwrap();
        cache.apply(execution(1), completed()).unwrap();
        assert_eq!(
            cache.apply(execution(1), current()),
            Err(BlockCacheError::Conflict)
        );
        assert_eq!(cache.visible(), None);
    }

    #[test]
    fn identity_anchor_revision_and_execution_conflicts_never_partially_mutate() {
        for conflict in [
            BlockState {
                block_id: block(99),
                ..current()
            },
            BlockState {
                start_line_id: 99,
                ..current()
            },
            BlockState {
                execution_id: execution(99),
                ..current()
            },
            BlockState {
                revision: 3,
                state: BlockLifecycle::Completed,
                ..current()
            },
        ] {
            let mut cache = BlockCache::default();
            cache.apply(execution(1), current()).unwrap();
            assert_eq!(
                cache.apply(execution(1), conflict),
                Err(BlockCacheError::Conflict)
            );
            assert_eq!(cache.visible(), None);
        }
    }

    #[test]
    fn first_record_must_be_current_revision_one() {
        let mut cache = BlockCache::default();
        assert_eq!(
            cache.apply(execution(1), completed()),
            Err(BlockCacheError::Conflict)
        );
        assert_eq!(cache.visible(), None);
    }

    #[test]
    fn quarantine_is_scoped_to_runtime_and_execution_epoch() {
        let execution = execution(0xabc);
        quarantine_epoch(10, execution);
        assert!(is_epoch_quarantined(10, execution));
        assert!(!is_epoch_quarantined(11, execution));
        assert!(!is_epoch_quarantined(10, execution(0xdef)));
    }
}
