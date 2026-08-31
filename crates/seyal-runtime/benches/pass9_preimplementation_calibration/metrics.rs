use std::{
    fs::File,
    mem::{MaybeUninit, size_of},
    process, thread,
    time::Instant,
};

use super::config::{QUIESCENT_SAMPLE_COUNT, QUIESCENT_SAMPLE_INTERVAL};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ProcessMetrics {
    pub(crate) rss_kib: usize,
    pub(crate) threads: usize,
    pub(crate) fds: usize,
    pub(crate) attachment_count: usize,
    pub(crate) has_controller: bool,
    pub(crate) local_connection_count: usize,
    pub(crate) pending_resync_count: usize,
    pub(crate) pending_resync_set_count: usize,
    pub(crate) listener_backoff_active: bool,
}

pub(crate) fn self_metrics() -> ProcessMetrics {
    metrics_for_pid(process::id())
}

// The calibration samples quiescent Runtime and client RSS after every
// lifecycle cycle. Spawning `/bin/ps` from either measured process changes its
// allocator high-water mark and turns the observer into a source of apparent
// RSS growth. `proc_pidinfo` reads the target task directly without launching
// a child or allocating command/output buffers in that target.
const MAX_SAMPLED_FDS: usize = 1024;

pub(crate) fn metrics_for_pid(pid: u32) -> ProcessMetrics {
    let task = task_info(pid);
    let threads = usize::try_from(task.pti_threadnum).expect("target thread count");
    assert!(
        threads > 0,
        "proc_pidinfo reported no threads for pid {pid}"
    );
    ProcessMetrics {
        rss_kib: (task.pti_resident_size / 1024) as usize,
        threads,
        fds: target_fd_count(pid),
        attachment_count: 0,
        has_controller: false,
        local_connection_count: 0,
        pending_resync_count: 0,
        pending_resync_set_count: 0,
        listener_backoff_active: false,
    }
}

pub(crate) fn median_self_metrics() -> ProcessMetrics {
    let mut rss = Vec::with_capacity(QUIESCENT_SAMPLE_COUNT);
    let mut threads = Vec::with_capacity(QUIESCENT_SAMPLE_COUNT);
    let mut fds = Vec::with_capacity(QUIESCENT_SAMPLE_COUNT);
    for sample in 0..QUIESCENT_SAMPLE_COUNT {
        let metrics = self_metrics();
        rss.push(metrics.rss_kib);
        threads.push(metrics.threads);
        fds.push(metrics.fds);
        if sample + 1 != QUIESCENT_SAMPLE_COUNT {
            thread::sleep(QUIESCENT_SAMPLE_INTERVAL);
        }
    }
    rss.sort_unstable();
    threads.sort_unstable();
    fds.sort_unstable();
    ProcessMetrics {
        rss_kib: rss[QUIESCENT_SAMPLE_COUNT / 2],
        threads: threads[QUIESCENT_SAMPLE_COUNT / 2],
        fds: fds[QUIESCENT_SAMPLE_COUNT / 2],
        attachment_count: 0,
        has_controller: false,
        local_connection_count: 0,
        pending_resync_count: 0,
        pending_resync_set_count: 0,
        listener_backoff_active: false,
    }
}

pub(crate) fn process_cpu_seconds(pid: u32) -> f64 {
    let task = task_info(pid);
    (task.pti_total_user.saturating_add(task.pti_total_system)) as f64 / 1_000_000_000.0
}

pub(crate) fn assert_measurement_integrity() {
    let pid = process::id();
    let baseline = metrics_for_pid(pid);
    assert!(baseline.rss_kib > 0);
    assert!(baseline.threads > 0);
    assert!(baseline.fds > 0);
    assert!(process_cpu_seconds(pid) >= 0.0);

    let held = File::open("/dev/null").expect("open measurement-integrity descriptor");
    assert_eq!(metrics_for_pid(pid).fds, baseline.fds + 1);
    drop(held);
    assert_eq!(metrics_for_pid(pid).fds, baseline.fds);
}

#[allow(unsafe_code)]
fn task_info(pid: u32) -> libc::proc_taskinfo {
    let mut task = MaybeUninit::<libc::proc_taskinfo>::uninit();
    // SAFETY: libc declares `proc_pidinfo`; `task` provides exactly the
    // PROC_PIDTASKINFO buffer requested, remains valid for the call, and is
    // initialized only after the kernel reports a full structure write.
    let bytes = unsafe {
        libc::proc_pidinfo(
            pid as libc::c_int,
            libc::PROC_PIDTASKINFO,
            0,
            task.as_mut_ptr().cast(),
            size_of::<libc::proc_taskinfo>() as libc::c_int,
        )
    };
    assert_eq!(
        bytes,
        size_of::<libc::proc_taskinfo>() as libc::c_int,
        "proc_pidinfo task info for pid {pid}: {}",
        std::io::Error::last_os_error()
    );
    // SAFETY: the equality above proves the kernel wrote the complete C
    // structure into `task`.
    unsafe { task.assume_init() }
}

#[allow(unsafe_code)]
fn target_fd_count(pid: u32) -> usize {
    let mut fds = MaybeUninit::<[libc::proc_fdinfo; MAX_SAMPLED_FDS]>::uninit();
    // SAFETY: the stack buffer is valid for the advertised byte count. The
    // result is used only as a byte count, so uninitialized entries are never
    // read by Rust.
    let bytes = unsafe {
        libc::proc_pidinfo(
            pid as libc::c_int,
            libc::PROC_PIDLISTFDS,
            0,
            fds.as_mut_ptr().cast(),
            size_of::<[libc::proc_fdinfo; MAX_SAMPLED_FDS]>() as libc::c_int,
        )
    };
    assert!(
        bytes >= 0,
        "proc_pidinfo FD list for pid {pid}: {}",
        std::io::Error::last_os_error()
    );
    let bytes = bytes as usize;
    assert_eq!(
        bytes % size_of::<libc::proc_fdinfo>(),
        0,
        "proc_pidinfo returned partial FD entries for pid {pid}"
    );
    assert!(
        bytes < size_of::<[libc::proc_fdinfo; MAX_SAMPLED_FDS]>(),
        "proc_pidinfo FD list reached the {}-descriptor calibration limit for pid {pid}",
        MAX_SAMPLED_FDS
    );
    bytes / size_of::<libc::proc_fdinfo>()
}

#[derive(Clone, Copy)]
pub(crate) struct Stats {
    pub(crate) p50_us: f64,
    pub(crate) p95_us: f64,
    pub(crate) p99_us: f64,
    pub(crate) max_us: f64,
}

pub(crate) fn stats(samples: &mut [u64]) -> Stats {
    samples.sort_unstable();
    Stats {
        p50_us: percentile_ns(samples, 50) as f64 / 1_000.0,
        p95_us: percentile_ns(samples, 95) as f64 / 1_000.0,
        p99_us: percentile_ns(samples, 99) as f64 / 1_000.0,
        max_us: samples.last().copied().unwrap_or(0) as f64 / 1_000.0,
    }
}

fn percentile_ns(sorted: &[u64], percentile: usize) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let rank = (percentile * sorted.len()).div_ceil(100).max(1);
    sorted[rank.saturating_sub(1).min(sorted.len() - 1)]
}

pub(crate) fn elapsed_ns(start: Instant) -> u64 {
    start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
}
