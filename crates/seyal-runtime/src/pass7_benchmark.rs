//! Benchmark-only Pass 7 timing marks.
//!
//! This module is compiled only with `benchmark-instrumentation`. It records
//! monotonic timing/counter metadata only: never input bytes, marked text,
//! terminal contents, environment values, or credentials.

use std::{
    sync::{
        OnceLock,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    time::Instant,
};

static ORIGIN: OnceLock<Instant> = OnceLock::new();
static INPUT_ADMISSION_NS: AtomicU64 = AtomicU64::new(0);
static PTY_WRITE_NS: AtomicU64 = AtomicU64::new(0);
static RESIZE_RECEIPT_NS: AtomicU64 = AtomicU64::new(0);
static RESIZE_COMMIT_NS: AtomicU64 = AtomicU64::new(0);
static INPUT_ADMISSION_COUNT: AtomicU64 = AtomicU64::new(0);
static PTY_WRITE_COUNT: AtomicU64 = AtomicU64::new(0);
static PTY_WRITE_BYTES: AtomicU64 = AtomicU64::new(0);
static RESIZE_RECEIPT_COUNT: AtomicU64 = AtomicU64::new(0);
static RESIZE_COMMIT_COUNT: AtomicU64 = AtomicU64::new(0);
static RUNTIME_QUEUE_HIGH_WATER: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pass7RuntimeBenchmarkMarks {
    pub input_admission_ns: u64,
    pub pty_write_ns: u64,
    pub resize_receipt_ns: u64,
    pub resize_commit_ns: u64,
    pub input_admission_count: u64,
    pub pty_write_count: u64,
    pub pty_write_bytes: u64,
    pub resize_receipt_count: u64,
    pub resize_commit_count: u64,
    pub runtime_queue_high_water_bytes: usize,
}

pub fn pass7_benchmark_now_ns() -> u64 {
    let elapsed = ORIGIN.get_or_init(Instant::now).elapsed().as_nanos();
    u64::try_from(elapsed).unwrap_or(u64::MAX)
}

pub fn reset_pass7_runtime_benchmark_marks() {
    INPUT_ADMISSION_NS.store(0, Ordering::SeqCst);
    PTY_WRITE_NS.store(0, Ordering::SeqCst);
    RESIZE_RECEIPT_NS.store(0, Ordering::SeqCst);
    RESIZE_COMMIT_NS.store(0, Ordering::SeqCst);
    INPUT_ADMISSION_COUNT.store(0, Ordering::SeqCst);
    PTY_WRITE_COUNT.store(0, Ordering::SeqCst);
    PTY_WRITE_BYTES.store(0, Ordering::SeqCst);
    RESIZE_RECEIPT_COUNT.store(0, Ordering::SeqCst);
    RESIZE_COMMIT_COUNT.store(0, Ordering::SeqCst);
    RUNTIME_QUEUE_HIGH_WATER.store(0, Ordering::SeqCst);
}

pub fn pass7_runtime_benchmark_marks() -> Pass7RuntimeBenchmarkMarks {
    Pass7RuntimeBenchmarkMarks {
        input_admission_ns: INPUT_ADMISSION_NS.load(Ordering::SeqCst),
        pty_write_ns: PTY_WRITE_NS.load(Ordering::SeqCst),
        resize_receipt_ns: RESIZE_RECEIPT_NS.load(Ordering::SeqCst),
        resize_commit_ns: RESIZE_COMMIT_NS.load(Ordering::SeqCst),
        input_admission_count: INPUT_ADMISSION_COUNT.load(Ordering::SeqCst),
        pty_write_count: PTY_WRITE_COUNT.load(Ordering::SeqCst),
        pty_write_bytes: PTY_WRITE_BYTES.load(Ordering::SeqCst),
        resize_receipt_count: RESIZE_RECEIPT_COUNT.load(Ordering::SeqCst),
        resize_commit_count: RESIZE_COMMIT_COUNT.load(Ordering::SeqCst),
        runtime_queue_high_water_bytes: RUNTIME_QUEUE_HIGH_WATER.load(Ordering::SeqCst),
    }
}

pub(crate) fn mark_pass7_input_admission(queue_bytes: usize) {
    INPUT_ADMISSION_NS.store(pass7_benchmark_now_ns(), Ordering::SeqCst);
    observe_runtime_queue(queue_bytes);
    INPUT_ADMISSION_COUNT.fetch_add(1, Ordering::SeqCst);
}

pub(crate) fn mark_pass7_pty_write(bytes: usize) {
    PTY_WRITE_NS.store(pass7_benchmark_now_ns(), Ordering::SeqCst);
    PTY_WRITE_COUNT.fetch_add(1, Ordering::SeqCst);
    PTY_WRITE_BYTES.fetch_add(u64::try_from(bytes).unwrap_or(u64::MAX), Ordering::SeqCst);
}

pub(crate) fn mark_pass7_resize_receipt() {
    RESIZE_RECEIPT_NS.store(pass7_benchmark_now_ns(), Ordering::SeqCst);
    RESIZE_RECEIPT_COUNT.fetch_add(1, Ordering::SeqCst);
}

pub(crate) fn mark_pass7_resize_commit() {
    RESIZE_COMMIT_NS.store(pass7_benchmark_now_ns(), Ordering::SeqCst);
    RESIZE_COMMIT_COUNT.fetch_add(1, Ordering::SeqCst);
}

pub(crate) fn observe_runtime_queue(queue_bytes: usize) {
    RUNTIME_QUEUE_HIGH_WATER.fetch_max(queue_bytes, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn benchmark_marks_are_metadata_only_and_resettable() {
        reset_pass7_runtime_benchmark_marks();
        mark_pass7_input_admission(17);
        mark_pass7_pty_write(5);
        mark_pass7_pty_write(7);
        mark_pass7_resize_receipt();
        mark_pass7_resize_commit();
        let marks = pass7_runtime_benchmark_marks();
        assert_eq!(marks.input_admission_count, 1);
        assert_eq!(marks.pty_write_count, 2);
        assert_eq!(marks.pty_write_bytes, 12);
        assert_eq!(marks.resize_receipt_count, 1);
        assert_eq!(marks.resize_commit_count, 1);
        assert_eq!(marks.runtime_queue_high_water_bytes, 17);
        assert!(marks.input_admission_ns > 0);
        assert!(marks.pty_write_ns > 0);
        assert!(marks.resize_receipt_ns > 0);
        assert!(marks.resize_commit_ns > 0);

        reset_pass7_runtime_benchmark_marks();
        assert_eq!(
            pass7_runtime_benchmark_marks(),
            Pass7RuntimeBenchmarkMarks::default()
        );
    }
}
