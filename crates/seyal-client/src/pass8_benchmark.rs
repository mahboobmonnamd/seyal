//! Benchmark-only access to the exact disposable Pass 8 client cache.
//!
//! This module exists only with `benchmark-instrumentation`; production code
//! does not gain a second cache or alternate metadata path.

use seyal_protocol::{ExecutionId, pass8::BlockState};

use crate::block_cache::BlockCache;

#[doc(hidden)]
#[derive(Default)]
pub struct BenchmarkBlockCache {
    inner: BlockCache,
}

impl BenchmarkBlockCache {
    pub fn apply(&mut self, execution_id: ExecutionId, state: BlockState) -> bool {
        self.inner.apply(execution_id, state).is_ok()
    }

    pub fn visible(&self) -> Option<BlockState> {
        self.inner.visible()
    }
}
