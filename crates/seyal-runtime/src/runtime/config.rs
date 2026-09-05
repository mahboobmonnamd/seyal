use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

#[cfg(feature = "benchmark-instrumentation")]
use std::collections::HashMap;

use crate::{CapabilityPolicy, RuntimeError};

#[cfg(feature = "benchmark-instrumentation")]
use crate::ExecutionId;

pub(super) const EVENT_CAPACITY: usize = 128;
pub(super) const READ_BUFFER_SIZE: usize = 16 * 1024;
pub(super) const CONTROL_DISPATCH_QUANTUM: usize = 64;
pub(super) const ROLLBACK_REAP_TICK: Duration = Duration::from_millis(10);
/// A `PrimaryExited` reactor event is kernel-confirmed (`NOTE_EXIT`) but
/// `waitpid(WNOHANG)` can still momentarily report the child as running — the
/// exit notification and reap-ability are not the same instant. This is the
/// retry cadence for re-attempting the reap rather than discarding the
/// one-shot notification and stranding the execution with no deadline.
pub(super) const PRIMARY_EXIT_REAP_RETRY: Duration = Duration::from_millis(10);
/// Bound `PrimaryExitPending` the same way PTY EOF probes are bounded: a
/// fixed number of short retries, then escalate into recoverable
/// `TerminationFailed` rather than spinning forever at 10 ms.
pub(super) const PRIMARY_EXIT_REAP_LIMIT: u8 = 6;
/// After forced-reap deadline, keep a bounded signalling/reap path so
/// `TerminationFailed` remains recoverable instead of a silent registry sink.
pub(super) const TERMINATION_FAILED_RETRY_INITIAL: Duration = Duration::from_millis(10);
pub(super) const TERMINATION_FAILED_RETRY_MAX: Duration = Duration::from_millis(250);
pub(super) const TERMINATION_FAILED_REAP_LIMIT: u8 = 8;
/// PTY EOF is terminal-I/O state, not process-exit truth. A short bounded
/// exponential probe covers the narrow race where a process exits around
/// NOTE_EXIT registration and the first `try_wait` has not become reapable
/// yet. If the child is genuinely still alive, probing stops completely and
/// the still-armed process-exit knote remains authoritative.
pub(super) const PTY_EOF_REAP_PROBE_INITIAL: Duration = Duration::from_millis(10);
pub(super) const PTY_EOF_REAP_PROBE_MAX: Duration = Duration::from_millis(320);
pub(super) const PTY_EOF_REAP_PROBE_LIMIT: u8 = 6;

#[derive(Clone, Copy, Debug)]
pub(super) struct PtyEofReapProbe {
    pub(super) deadline: Instant,
    delay: Duration,
    remaining: u8,
}

impl PtyEofReapProbe {
    pub(super) fn new(now: Instant) -> Self {
        Self {
            deadline: now + PTY_EOF_REAP_PROBE_INITIAL,
            delay: PTY_EOF_REAP_PROBE_INITIAL,
            remaining: PTY_EOF_REAP_PROBE_LIMIT,
        }
    }

    pub(super) fn next(self, now: Instant) -> Option<Self> {
        if self.remaining <= 1 {
            return None;
        }
        let delay = self.delay.saturating_mul(2).min(PTY_EOF_REAP_PROBE_MAX);
        Some(Self {
            deadline: now + delay,
            delay,
            remaining: self.remaining - 1,
        })
    }
}

#[cfg(feature = "benchmark-instrumentation")]
#[derive(Clone, Copy, Debug, Default)]
pub struct BenchmarkRuntimeDiagnostics {
    pub pty_bytes_read: u64,
    pub pty_read_calls: u64,
    pub source_timestamp_samples: usize,
    pub latest_damage_generation: u64,
}

#[cfg(feature = "benchmark-instrumentation")]
#[derive(Default)]
pub(super) struct BenchmarkRuntimeState {
    pub(super) pty_bytes_read: u64,
    pub(super) pty_read_calls: u64,
    pub(super) source_times: HashMap<(ExecutionId, u64), Instant>,
}

#[derive(Clone, Debug)]
pub enum LocalIpcMode {
    Disabled,
    Enabled {
        runtime_dir_override: Option<PathBuf>,
    },
}

#[derive(Clone, Debug)]
pub struct RuntimeConfig {
    pub singleton_path: PathBuf,
    pub max_executions: usize,
    pub control_queue_capacity: usize,
    pub per_execution_input_bytes: usize,
    pub aggregate_input_bytes: usize,
    pub read_dispatch_bytes: usize,
    pub write_dispatch_bytes: usize,
    pub graceful_termination: Duration,
    pub forced_reap: Duration,
    pub final_drain: Duration,
    pub capability_policy: CapabilityPolicy,
    pub local_ipc: LocalIpcMode,
}

impl RuntimeConfig {
    pub fn m001() -> Result<Self, RuntimeError> {
        Ok(Self {
            singleton_path: std::env::temp_dir().join("seyal").join("runtime.lock"),
            max_executions: 512,
            control_queue_capacity: 1024,
            per_execution_input_bytes: 256 * 1024,
            aggregate_input_bytes: 8 * 1024 * 1024,
            read_dispatch_bytes: 64 * 1024,
            write_dispatch_bytes: 64 * 1024,
            graceful_termination: Duration::from_secs(1),
            forced_reap: Duration::from_secs(1),
            final_drain: Duration::from_millis(250),
            capability_policy: CapabilityPolicy::bundled()?,
            local_ipc: LocalIpcMode::Enabled {
                runtime_dir_override: None,
            },
        })
    }
}
