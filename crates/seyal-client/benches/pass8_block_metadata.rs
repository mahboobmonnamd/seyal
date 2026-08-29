use std::time::Instant;

#[cfg(target_os = "macos")]
use std::{
    fs,
    hint::black_box,
    process::{self, Command},
    thread,
    time::Duration,
};

#[cfg(target_os = "macos")]
use seyal_client::pass8_benchmark::BenchmarkBlockCache;
#[cfg(target_os = "macos")]
use seyal_protocol::{
    BlockId, ExecutionId,
    pass8::{BlockKind, BlockLifecycle, BlockState},
};
#[cfg(target_os = "macos")]
use seyal_runtime::pass8_benchmark::BenchmarkBlockTimeline;

const PERFORMANCE_CLAIM: &str = "performance_claim=false";
#[cfg(target_os = "macos")]
const REPETITIONS: usize = 20_000;
#[cfg(target_os = "macos")]
const LIVE_RECORDS: usize = 512;
#[cfg(target_os = "macos")]
const LATENCY_GATE_NS: u64 = 250_000;
#[cfg(target_os = "macos")]
const RSS_GATE_KIB: usize = 1024;

fn main() {
    // Repository benchmark contract requires a monotonic clock in every
    // production benchmark target.
    let _contract_clock = Instant::now();

    #[cfg(not(target_os = "macos"))]
    println!("pass8_block_metadata PLATFORM_LIMITED target_os!=macos {PERFORMANCE_CLAIM}");

    #[cfg(target_os = "macos")]
    run_macos();
}

#[cfg(target_os = "macos")]
fn run_macos() {
    println!(
        "pass8_block_metadata architecture=production_BlockState_BlockCache_Runtime_BlockTimeline {PERFORMANCE_CLAIM} percentile_method=nearest_rank repetitions={REPETITIONS} live_records={LIVE_RECORDS}"
    );
    print_host_metadata();
    measure_encode_apply_latency();
    measure_retained_metadata_resources();
}

#[cfg(target_os = "macos")]
fn measure_encode_apply_latency() {
    let mut samples = Vec::with_capacity(REPETITIONS);

    // Warm code/data pages before timing and before the separate retained-RSS
    // sample so page faults are not attributed to live Block records.
    for ordinal in 0..256 {
        run_latency_sample(ordinal);
    }

    for ordinal in 0..REPETITIONS {
        let start = Instant::now();
        run_latency_sample(ordinal + 1024);
        samples.push(start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64);
    }
    samples.sort_unstable();
    let p50 = percentile_ns(&samples, 50);
    let p95 = percentile_ns(&samples, 95);
    let p99 = percentile_ns(&samples, 99);
    let max = samples.last().copied().unwrap_or(0);
    println!(
        "pass8_latency boundary=BlockState_encode_decode_client_apply classification=MEASURED sample_count={} p50_us={:.3} p95_us={:.3} p99_us={:.3} max_us={:.3} gate_us=250.000 {}",
        samples.len(),
        p50 as f64 / 1_000.0,
        p95 as f64 / 1_000.0,
        p99 as f64 / 1_000.0,
        max as f64 / 1_000.0,
        PERFORMANCE_CLAIM,
    );
    assert!(
        p99 <= LATENCY_GATE_NS,
        "Pass 8 BlockState encode/decode/client apply p99 exceeded 250 us: {:.3} us",
        p99 as f64 / 1_000.0
    );
}

#[cfg(target_os = "macos")]
fn run_latency_sample(ordinal: usize) {
    let execution_id = ExecutionId::from_bytes((ordinal as u128 + 1).to_le_bytes());
    let block_id = BlockId::from_bytes((ordinal as u128 + 0x10000).to_le_bytes());
    let state = BlockState {
        execution_id,
        block_id,
        revision: 1,
        start_line_id: ordinal as u64 + 1,
        kind: BlockKind::TerminalActivity,
        state: BlockLifecycle::Current,
    };
    let mut cache = BenchmarkBlockCache::default();
    let encoded = black_box(state)
        .encode()
        .expect("benchmark BlockState encode");
    let decoded = BlockState::decode(black_box(&encoded)).expect("benchmark BlockState decode");
    assert!(cache.apply(execution_id, decoded));
    black_box(cache.visible());
}

#[cfg(target_os = "macos")]
fn measure_retained_metadata_resources() {
    // Latency warmup above already touched production Block code. The delta
    // below therefore measures the retained production metadata map rather than
    // first-use code pages. No PTY/process is created for this attribution test.
    let baseline = process_metrics();
    let mut timeline = BenchmarkBlockTimeline::with_live_records(LIVE_RECORDS);
    black_box(timeline.len());
    assert_eq!(timeline.len(), LIVE_RECORDS);
    let populated = process_metrics();
    let incremental_rss_kib = populated.rss_kib.saturating_sub(baseline.rss_kib);

    // Holding Block metadata has no task/timer/FD owner. Leave the exact
    // production map untouched for an idle window and prove resource counts do
    // not grow while it remains live.
    let idle_cpu_start = process_cpu_seconds();
    let idle_started = Instant::now();
    thread::sleep(Duration::from_millis(500));
    let idle_elapsed = idle_started.elapsed().as_secs_f64();
    let idle_cpu_end = process_cpu_seconds();
    let idle_cpu_percent = if idle_elapsed > 0.0 {
        ((idle_cpu_end - idle_cpu_start).max(0.0) / idle_elapsed) * 100.0
    } else {
        0.0
    };
    let idle = process_metrics();
    assert_eq!(
        populated.threads, idle.threads,
        "Pass 8 metadata introduced an idle thread source"
    );
    assert_eq!(
        populated.fds, idle.fds,
        "Pass 8 metadata introduced an idle fd/timer source"
    );
    assert!(
        incremental_rss_kib <= RSS_GATE_KIB,
        "512 live Pass 8 Block records exceeded 1 MiB attributable RSS: {incremental_rss_kib} KiB"
    );

    timeline.complete_and_retire_all();
    black_box(timeline.len());
    assert!(
        timeline.is_empty(),
        "completed Pass 8 records remained after retirement"
    );

    println!(
        "pass8_resource classification=MEASURED live_records={} rss_baseline_kib={} rss_populated_kib={} attributable_live_rss_kib={} rss_gate_kib={} idle_window_ms=500 cpu_percent_idle={} threads_populated={} threads_idle={} fds_populated={} fds_idle={} records_after_retire={} {}",
        LIVE_RECORDS,
        baseline.rss_kib,
        populated.rss_kib,
        incremental_rss_kib,
        RSS_GATE_KIB,
        idle_cpu_percent,
        populated.threads,
        idle.threads,
        populated.fds,
        idle.fds,
        timeline.len(),
        PERFORMANCE_CLAIM,
    );
}

#[cfg(target_os = "macos")]
fn percentile_ns(sorted: &[u64], percentile: usize) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let rank = (percentile * sorted.len()).div_ceil(100).max(1);
    sorted[rank.saturating_sub(1).min(sorted.len() - 1)]
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy)]
struct Metrics {
    rss_kib: usize,
    cpu_percent: f32,
    threads: usize,
    fds: usize,
}

#[cfg(target_os = "macos")]
fn process_metrics() -> Metrics {
    let pid = process::id();
    let output = Command::new("/bin/ps")
        .args(["-o", "rss=,%cpu=,thcount=", "-p", &pid.to_string()])
        .output()
        .expect("ps metrics");
    let line = String::from_utf8_lossy(&output.stdout);
    let mut fields = line.split_whitespace();
    let rss_kib = fields
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    let cpu_percent = fields
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0.0);
    let parsed_threads = fields
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    let threads = if parsed_threads == 0 {
        Command::new("/bin/ps")
            .args(["-M", "-p", &pid.to_string()])
            .output()
            .ok()
            .map(|output| {
                String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .skip(1)
                    .count()
            })
            .unwrap_or(0)
    } else {
        parsed_threads
    };
    let fds = fs::read_dir("/dev/fd")
        .map(|entries| entries.count())
        .unwrap_or(0);
    Metrics {
        rss_kib,
        cpu_percent,
        threads,
        fds,
    }
}

#[cfg(target_os = "macos")]
fn process_cpu_seconds() -> f64 {
    let pid = process::id();
    let output = Command::new("/bin/ps")
        .args(["-o", "time=", "-p", &pid.to_string()])
        .output()
        .expect("ps cpu time");
    parse_cpu_time(String::from_utf8_lossy(&output.stdout).trim())
}

#[cfg(target_os = "macos")]
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

#[cfg(target_os = "macos")]
fn print_host_metadata() {
    let product = command_text("/usr/bin/sw_vers", &["-productVersion"]);
    let build = command_text("/usr/bin/sw_vers", &["-buildVersion"]);
    let model = command_text("/usr/sbin/sysctl", &["-n", "hw.model"]);
    let hardware = command_text("/usr/sbin/sysctl", &["-n", "machdep.cpu.brand_string"]);
    let rust = command_text("rustc", &["--version"]);
    let commit = command_text("git", &["rev-parse", "HEAD"]);
    println!(
        "pass8_host macos_version={} macos_build={} model={:?} hardware={:?} arch={} rust={:?} build_mode=release commit={} repetitions={} percentile_method=nearest_rank rss_source=ps {}",
        product,
        build,
        model,
        hardware,
        std::env::consts::ARCH,
        rust,
        commit,
        REPETITIONS,
        PERFORMANCE_CLAIM,
    );
}

#[cfg(target_os = "macos")]
fn command_text(program: &str, args: &[&str]) -> String {
    Command::new(program)
        .args(args)
        .output()
        .ok()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_else(|| "unavailable".to_owned())
}
