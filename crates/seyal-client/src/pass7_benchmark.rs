//! Benchmark-only Pass 7 client timing marks.
//!
//! Compiled only for the dedicated performance harness. The marks contain only
//! monotonic timestamps, counters, and queue sizes; secret-bearing input and
//! terminal/IME content are never retained.

use std::{
    sync::{
        OnceLock,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    time::Instant,
};

static ORIGIN: OnceLock<Instant> = OnceLock::new();
static ADMISSION_NS: AtomicU64 = AtomicU64::new(0);
static SOCKET_COMPLETE_NS: AtomicU64 = AtomicU64::new(0);
static ADMISSION_COUNT: AtomicU64 = AtomicU64::new(0);
static SOCKET_COMPLETE_COUNT: AtomicU64 = AtomicU64::new(0);
static QUEUE_HIGH_WATER: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pass7ClientBenchmarkMarks {
    pub admission_ns: u64,
    pub socket_complete_ns: u64,
    pub admission_count: u64,
    pub socket_complete_count: u64,
    pub queue_high_water_bytes: usize,
}

pub fn pass7_client_benchmark_now_ns() -> u64 {
    let elapsed = ORIGIN.get_or_init(Instant::now).elapsed().as_nanos();
    u64::try_from(elapsed).unwrap_or(u64::MAX)
}

pub fn reset_pass7_client_benchmark_marks() {
    ADMISSION_NS.store(0, Ordering::SeqCst);
    SOCKET_COMPLETE_NS.store(0, Ordering::SeqCst);
    ADMISSION_COUNT.store(0, Ordering::SeqCst);
    SOCKET_COMPLETE_COUNT.store(0, Ordering::SeqCst);
    QUEUE_HIGH_WATER.store(0, Ordering::SeqCst);
}

pub fn pass7_client_benchmark_marks() -> Pass7ClientBenchmarkMarks {
    Pass7ClientBenchmarkMarks {
        admission_ns: ADMISSION_NS.load(Ordering::SeqCst),
        socket_complete_ns: SOCKET_COMPLETE_NS.load(Ordering::SeqCst),
        admission_count: ADMISSION_COUNT.load(Ordering::SeqCst),
        socket_complete_count: SOCKET_COMPLETE_COUNT.load(Ordering::SeqCst),
        queue_high_water_bytes: QUEUE_HIGH_WATER.load(Ordering::SeqCst),
    }
}

pub(crate) fn mark_pass7_client_admission(queue_bytes: usize) {
    ADMISSION_NS.store(pass7_client_benchmark_now_ns(), Ordering::SeqCst);
    QUEUE_HIGH_WATER.fetch_max(queue_bytes, Ordering::SeqCst);
    ADMISSION_COUNT.fetch_add(1, Ordering::SeqCst);
}

pub(crate) fn mark_pass7_client_socket_complete(queue_bytes: usize) {
    SOCKET_COMPLETE_NS.store(pass7_client_benchmark_now_ns(), Ordering::SeqCst);
    QUEUE_HIGH_WATER.fetch_max(queue_bytes, Ordering::SeqCst);
    SOCKET_COMPLETE_COUNT.fetch_add(1, Ordering::SeqCst);
}

pub(crate) fn observe_pass7_client_queue(queue_bytes: usize) {
    QUEUE_HIGH_WATER.fetch_max(queue_bytes, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_marks_are_bounded_metadata_and_resettable() {
        reset_pass7_client_benchmark_marks();
        mark_pass7_client_admission(41);
        observe_pass7_client_queue(73);
        mark_pass7_client_socket_complete(0);
        let marks = pass7_client_benchmark_marks();
        assert_eq!(marks.admission_count, 1);
        assert_eq!(marks.socket_complete_count, 1);
        assert_eq!(marks.queue_high_water_bytes, 73);
        assert!(marks.admission_ns > 0);
        assert!(marks.socket_complete_ns >= marks.admission_ns);

        reset_pass7_client_benchmark_marks();
        assert_eq!(
            pass7_client_benchmark_marks(),
            Pass7ClientBenchmarkMarks::default()
        );
    }
}
