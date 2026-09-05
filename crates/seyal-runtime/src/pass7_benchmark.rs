//! Benchmark-only Pass 7 timing marks.
//!
//! This module is compiled only with `benchmark-instrumentation`. It records
//! monotonic timing/counter metadata only: never input bytes, marked text,
//! terminal contents, environment values, or credentials.

use std::{
    sync::{
        Mutex, MutexGuard, OnceLock,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    time::Instant,
};

/// Workspace tests unify `benchmark-instrumentation` through `seyal-client`, so
/// Pass 7 mark atomics are process-global across parallel unit tests. Serialize
/// mark/reset/observe mutations so metadata assertions are not racing other
/// Runtime tests that also touch input/resize instrumentation.
fn marks_lock() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

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

fn reset_pass7_runtime_benchmark_marks_locked() {
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

fn pass7_runtime_benchmark_marks_locked() -> Pass7RuntimeBenchmarkMarks {
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

fn mark_pass7_input_admission_locked(queue_bytes: usize) {
    INPUT_ADMISSION_NS.store(pass7_benchmark_now_ns(), Ordering::SeqCst);
    observe_runtime_queue_locked(queue_bytes);
    INPUT_ADMISSION_COUNT.fetch_add(1, Ordering::SeqCst);
}

fn mark_pass7_pty_write_locked(bytes: usize) {
    PTY_WRITE_NS.store(pass7_benchmark_now_ns(), Ordering::SeqCst);
    PTY_WRITE_COUNT.fetch_add(1, Ordering::SeqCst);
    PTY_WRITE_BYTES.fetch_add(u64::try_from(bytes).unwrap_or(u64::MAX), Ordering::SeqCst);
}

fn mark_pass7_resize_receipt_locked() {
    RESIZE_RECEIPT_NS.store(pass7_benchmark_now_ns(), Ordering::SeqCst);
    RESIZE_RECEIPT_COUNT.fetch_add(1, Ordering::SeqCst);
}

fn mark_pass7_resize_commit_locked() {
    RESIZE_COMMIT_NS.store(pass7_benchmark_now_ns(), Ordering::SeqCst);
    RESIZE_COMMIT_COUNT.fetch_add(1, Ordering::SeqCst);
}

pub fn reset_pass7_runtime_benchmark_marks() {
    let _guard = marks_lock();
    reset_pass7_runtime_benchmark_marks_locked();
}

pub fn pass7_runtime_benchmark_marks() -> Pass7RuntimeBenchmarkMarks {
    let _guard = marks_lock();
    pass7_runtime_benchmark_marks_locked()
}

pub(crate) fn mark_pass7_input_admission(queue_bytes: usize) {
    let _guard = marks_lock();
    mark_pass7_input_admission_locked(queue_bytes);
}

pub(crate) fn mark_pass7_pty_write(bytes: usize) {
    let _guard = marks_lock();
    mark_pass7_pty_write_locked(bytes);
}

pub(crate) fn mark_pass7_resize_receipt() {
    let _guard = marks_lock();
    mark_pass7_resize_receipt_locked();
}

pub(crate) fn mark_pass7_resize_commit() {
    let _guard = marks_lock();
    mark_pass7_resize_commit_locked();
}

fn observe_runtime_queue_locked(queue_bytes: usize) {
    RUNTIME_QUEUE_HIGH_WATER.fetch_max(queue_bytes, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn benchmark_marks_are_metadata_only_and_resettable() {
        // Prime the origin clock and advance past a zero-elapsed sample so
        // timestamp marks are observably non-zero on fast hosts.
        let _ = pass7_benchmark_now_ns();
        std::thread::sleep(std::time::Duration::from_micros(50));

        // Hold the process-global marks lock for the whole assertion window so
        // parallel Runtime tests cannot reset mid-check.
        let _guard = marks_lock();
        reset_pass7_runtime_benchmark_marks_locked();
        mark_pass7_input_admission_locked(17);
        mark_pass7_pty_write_locked(5);
        mark_pass7_pty_write_locked(7);
        mark_pass7_resize_receipt_locked();
        mark_pass7_resize_commit_locked();
        let marks = pass7_runtime_benchmark_marks_locked();
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

        reset_pass7_runtime_benchmark_marks_locked();
        assert_eq!(
            pass7_runtime_benchmark_marks_locked(),
            Pass7RuntimeBenchmarkMarks::default()
        );
    }
}
