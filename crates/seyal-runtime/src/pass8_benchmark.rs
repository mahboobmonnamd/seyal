//! Benchmark-only access to the exact Runtime-owned Pass 8 metadata map.
//!
//! The wrapper owns the production `BlockTimeline` directly. It creates no PTY,
//! VT, terminal grid, renderer, transcript or background task, so retained RSS
//! can be attributed to the metadata structure itself.

use crate::{ExecutionId, WorkspaceId, block::BlockTimeline};

#[doc(hidden)]
#[derive(Default)]
pub struct BenchmarkBlockTimeline {
    inner: BlockTimeline,
    count: usize,
}

impl BenchmarkBlockTimeline {
    pub fn with_live_records(count: usize) -> Self {
        let mut value = Self::default();
        let workspace = WorkspaceId::m001_default();
        for ordinal in 0..count {
            let id = execution_id(ordinal);
            value
                .inner
                .admit(workspace, id, ordinal as u64 + 1)
                .expect("benchmark execution identity must be unique");
        }
        value.count = count;
        value
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn complete_and_retire_all(&mut self) {
        let workspace = WorkspaceId::m001_default();
        for ordinal in 0..self.count {
            let id = execution_id(ordinal);
            self.inner
                .complete(workspace, id)
                .expect("benchmark completion must be valid")
                .expect("benchmark record must exist");
            self.inner
                .retire(id)
                .expect("completed benchmark record must retire");
        }
        self.count = 0;
    }
}

fn execution_id(ordinal: usize) -> ExecutionId {
    // Production-generated ExecutionIds are opaque. The benchmark requires only
    // deterministic unique keys so it can measure the exact production map
    // without paying for unrelated PTY/process resources.
    ExecutionId::from_bytes((ordinal as u128 + 1).to_le_bytes())
}
