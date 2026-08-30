use std::{
    fs,
    process::{self, Command},
    thread,
    time::Instant,
};

use super::config::{QUIESCENT_SAMPLE_COUNT, QUIESCENT_SAMPLE_INTERVAL};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ProcessMetrics {
    pub(crate) rss_kib: usize,
    pub(crate) threads: usize,
    pub(crate) fds: usize,
    pub(crate) attachment_count: usize,
}

pub(crate) fn self_metrics() -> ProcessMetrics {
    metrics_for_pid(process::id())
}

// `ps -o thcount=`/`nlwp=` are not stable across macOS releases: an
// unrecognized keyword makes `ps` drop that column silently instead of
// failing, which previously produced a truncated line and a parse panic.
// `-M` reliably emits one row per thread on every macOS `ps` implementation,
// so thread count is derived from row count instead of a named column.
pub(crate) fn metrics_for_pid(pid: u32) -> ProcessMetrics {
    let rss_output = Command::new("/bin/ps")
        .args(["-o", "rss=", "-p", &pid.to_string()])
        .output()
        .expect("ps rss");
    let rss_kib = String::from_utf8_lossy(&rss_output.stdout)
        .trim()
        .parse()
        .expect("RSS metric");

    let thread_output = Command::new("/bin/ps")
        .args(["-M", "-p", &pid.to_string()])
        .output()
        .expect("ps thread count");
    let threads = String::from_utf8_lossy(&thread_output.stdout)
        .lines()
        .skip(1)
        .count();
    assert!(threads > 0, "ps -M reported no threads for pid {pid}");

    let fds = fs::read_dir("/dev/fd").expect("/dev/fd").count();
    ProcessMetrics {
        rss_kib,
        threads,
        fds,
        attachment_count: 0,
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
    }
}

pub(crate) fn process_cpu_seconds(pid: u32) -> f64 {
    let output = Command::new("/bin/ps")
        .args(["-o", "time=", "-p", &pid.to_string()])
        .output()
        .expect("ps cpu time");
    parse_cpu_time(String::from_utf8_lossy(&output.stdout).trim())
}

fn parse_cpu_time(value: &str) -> f64 {
    let fields = value.split(':').collect::<Vec<_>>();
    match fields.as_slice() {
        [minutes, seconds] => {
            minutes.parse::<f64>().unwrap_or(0.0) * 60.0 + seconds.parse::<f64>().unwrap_or(0.0)
        }
        [hours, minutes, seconds] => {
            hours.parse::<f64>().unwrap_or(0.0) * 3600.0
                + minutes.parse::<f64>().unwrap_or(0.0) * 60.0
                + seconds.parse::<f64>().unwrap_or(0.0)
        }
        _ => 0.0,
    }
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
