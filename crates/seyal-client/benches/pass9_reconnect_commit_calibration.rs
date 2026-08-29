use std::time::Instant;

#[cfg(target_os = "macos")]
use std::{
    process::{self, Command},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[cfg(target_os = "macos")]
use seyal_client::LocalDisplayClient;
#[cfg(target_os = "macos")]
use seyal_exec::{CommandSpec, WindowSize};
#[cfg(target_os = "macos")]
use seyal_runtime::{
    ExecutionId, LocalIpcMode, Runtime, RuntimeConfig, RuntimeId,
    local_ipc::framing::Role,
};

const PERFORMANCE_CLAIM: &str = "performance_claim=false";
#[cfg(target_os = "macos")]
const WARMUP_CYCLES: usize = 20;
#[cfg(target_os = "macos")]
const MEASURED_CYCLES: usize = 100;
#[cfg(target_os = "macos")]
const COHORTS: usize = 5;
#[cfg(target_os = "macos")]
const COLUMNS: u16 = 120;
#[cfg(target_os = "macos")]
const ROWS: u16 = 40;
#[cfg(target_os = "macos")]
const CLEANUP_TIMEOUT: Duration = Duration::from_secs(1);
#[cfg(target_os = "macos")]
static HARNESS_COUNTER: AtomicU64 = AtomicU64::new(0);

fn main() {
    let _contract_clock = Instant::now();

    #[cfg(not(target_os = "macos"))]
    println!(
        "pass9_reconnect_commit_calibration PLATFORM_LIMITED target_os!=macos {PERFORMANCE_CLAIM}"
    );

    #[cfg(target_os = "macos")]
    run_macos();
}

#[cfg(target_os = "macos")]
fn run_macos() {
    println!(
        "pass9_reconnect_commit_calibration architecture=production_LocalDisplayClient_Runtime_UDS_PTY geometry={}x{} warmup_cycles={} measured_cycles={} cohorts={} percentile_method=nearest_rank reconnect_boundary=connect_hello_attach_snapshot_cache_commit_prepare_surface_return {}",
        COLUMNS,
        ROWS,
        WARMUP_CYCLES,
        MEASURED_CYCLES,
        COHORTS,
        PERFORMANCE_CLAIM,
    );
    print_host_metadata();

    let mut cohort_p99 = Vec::with_capacity(COHORTS);
    for cohort in 0..COHORTS {
        let result = measure_cohort(cohort + 1);
        cohort_p99.push(result.p99_us);
    }
    cohort_p99.sort_by(|a, b| a.total_cmp(b));
    println!(
        "pass9_reconnect_commit_summary boundary=connect_hello_attach_snapshot_cache_commit_prepare_surface_return median_cohort_p99_us={:.3} cohorts={} cycles_per_cohort={} {}",
        cohort_p99[COHORTS / 2],
        COHORTS,
        MEASURED_CYCLES,
        PERFORMANCE_CLAIM,
    );
}

#[cfg(target_os = "macos")]
fn measure_cohort(cohort: usize) -> Stats {
    let harness = RuntimeHarness::start();
    let initial = harness.snapshot();
    assert_eq!(initial.runtime_id, harness.runtime_id);
    assert_eq!(initial.execution_id, harness.execution_id);
    assert_eq!(initial.attachment_count, 0);

    for _ in 0..WARMUP_CYCLES {
        let client = connect_committed_client(&harness);
        drop(client);
        wait_for_attachment_count(&harness, 0);
    }

    let mut samples = Vec::with_capacity(MEASURED_CYCLES);
    for _ in 0..MEASURED_CYCLES {
        let started = Instant::now();
        let client = connect_committed_client(&harness);
        samples.push(elapsed_ns(started));

        let attached = harness.snapshot();
        assert_eq!(attached.runtime_id, harness.runtime_id);
        assert_eq!(attached.execution_id, harness.execution_id);
        assert_eq!(attached.attachment_count, 1);

        drop(client);
        wait_for_attachment_count(&harness, 0);
    }

    let stats = stats(&mut samples);
    println!(
        "pass9_reconnect_commit_cohort cohort={} runtime_id={} execution_id={} boundary=connect_hello_attach_snapshot_cache_commit_prepare_surface_return p50_us={:.3} p95_us={:.3} p99_us={:.3} max_us={:.3} sample_count={} {}",
        cohort,
        harness.runtime_id,
        harness.execution_id,
        stats.p50_us,
        stats.p95_us,
        stats.p99_us,
        stats.max_us,
        MEASURED_CYCLES,
        PERFORMANCE_CLAIM,
    );

    harness.finish();
    stats
}

#[cfg(target_os = "macos")]
fn connect_committed_client(harness: &RuntimeHarness) -> LocalDisplayClient {
    let client = LocalDisplayClient::connect_execution(
        &harness.socket_path,
        harness.execution_id,
        Role::Controller,
    )
    .expect("production client reconnect");
    assert_eq!(client.execution_id(), harness.execution_id);
    assert_eq!(client.cache().rows, ROWS);
    assert_eq!(client.cache().columns, COLUMNS);
    assert_eq!(client.prepared_surface().rows(), ROWS);
    assert_eq!(client.prepared_surface().columns(), COLUMNS);
    assert_eq!(
        client.prepared_surface().generation(),
        Some(client.cache().generation)
    );
    client
}

#[cfg(target_os = "macos")]
fn wait_for_attachment_count(harness: &RuntimeHarness, expected: usize) {
    let deadline = Instant::now() + CLEANUP_TIMEOUT;
    loop {
        let snapshot = harness.snapshot();
        if snapshot.attachment_count == expected {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "client reconnect calibration cleanup failed: expected {expected}, observed {}",
            snapshot.attachment_count
        );
        thread::yield_now();
    }
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy)]
struct HarnessSnapshot {
    runtime_id: RuntimeId,
    execution_id: ExecutionId,
    attachment_count: usize,
}

#[cfg(target_os = "macos")]
enum HarnessCommand {
    Snapshot(mpsc::Sender<HarnessSnapshot>),
    Stop,
}

#[cfg(target_os = "macos")]
struct RuntimeHarness {
    socket_path: std::path::PathBuf,
    runtime_id: RuntimeId,
    execution_id: ExecutionId,
    control: mpsc::Sender<HarnessCommand>,
    join: thread::JoinHandle<()>,
}

#[cfg(target_os = "macos")]
impl RuntimeHarness {
    fn start() -> Self {
        let suffix = HARNESS_COUNTER.fetch_add(1, Ordering::Relaxed);
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let (ready_tx, ready_rx) = mpsc::channel();
        let (control_tx, control_rx) = mpsc::channel();

        let join = thread::spawn(move || {
            let mut config = RuntimeConfig::m001().expect("M001 Runtime config");
            config.singleton_path = std::env::temp_dir()
                .join(format!("s9cc-{}-{suffix:x}-{nonce:x}.lock", process::id()));
            let runtime_dir = std::env::temp_dir()
                .join(format!("s9ccd-{}-{suffix:x}-{nonce:x}", process::id()));
            config.local_ipc = LocalIpcMode::Enabled {
                runtime_dir_override: Some(runtime_dir),
            };
            config.graceful_termination = Duration::from_millis(50);
            config.forced_reap = Duration::from_millis(250);
            config.final_drain = Duration::from_millis(100);

            let mut runtime = Runtime::new(config).expect("Runtime");
            let execution_id = runtime
                .create_execution(
                    CommandSpec::new("/bin/cat"),
                    WindowSize::cells(COLUMNS, ROWS).expect("120x40 geometry"),
                )
                .expect("execution");
            let socket_path = runtime
                .local_ipc_socket_path()
                .expect("Runtime socket path")
                .to_path_buf();
            let runtime_id = runtime.id();
            ready_tx
                .send((socket_path, runtime_id, execution_id))
                .expect("calibration ready receiver");

            let mut stop = false;
            while !stop {
                while let Ok(command) = control_rx.try_recv() {
                    match command {
                        HarnessCommand::Snapshot(reply) => {
                            let summary = runtime.lookup(execution_id).expect("live execution");
                            let _ = reply.send(HarnessSnapshot {
                                runtime_id: runtime.id(),
                                execution_id,
                                attachment_count: summary.attachment_count,
                            });
                        }
                        HarnessCommand::Stop => {
                            stop = true;
                            break;
                        }
                    }
                }
                if !stop {
                    runtime
                        .poll_once(Some(Duration::from_millis(2)))
                        .expect("Runtime poll");
                }
            }

            runtime.begin_shutdown().expect("begin Runtime shutdown");
            runtime
                .run_until_empty(Instant::now() + Duration::from_secs(3))
                .expect("Runtime shutdown");
        });

        let (socket_path, runtime_id, execution_id) = ready_rx
            .recv_timeout(Duration::from_secs(3))
            .expect("Runtime ready");
        Self {
            socket_path,
            runtime_id,
            execution_id,
            control: control_tx,
            join,
        }
    }

    fn snapshot(&self) -> HarnessSnapshot {
        let (tx, rx) = mpsc::channel();
        self.control
            .send(HarnessCommand::Snapshot(tx))
            .expect("Runtime calibration control channel");
        rx.recv_timeout(Duration::from_secs(1))
            .expect("Runtime calibration snapshot")
    }

    fn finish(self) {
        let _ = self.control.send(HarnessCommand::Stop);
        self.join.join().expect("Runtime calibration thread");
    }
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy)]
struct Stats {
    p50_us: f64,
    p95_us: f64,
    p99_us: f64,
    max_us: f64,
}

#[cfg(target_os = "macos")]
fn stats(samples: &mut [u64]) -> Stats {
    samples.sort_unstable();
    Stats {
        p50_us: percentile_ns(samples, 50) as f64 / 1_000.0,
        p95_us: percentile_ns(samples, 95) as f64 / 1_000.0,
        p99_us: percentile_ns(samples, 99) as f64 / 1_000.0,
        max_us: samples.last().copied().unwrap_or(0) as f64 / 1_000.0,
    }
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
fn elapsed_ns(start: Instant) -> u64 {
    start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
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
        "pass9_reconnect_commit_host macos_version={} macos_build={} model={:?} hardware={:?} arch={} rust={:?} build_mode=release commit={} master_baseline=efa365d48565fb09452b683577700a8e5e267fcb pass8_baseline=d9d21187e8429bbd3dbeb3e1c7cc4d05c1d147e6 {}",
        product,
        build,
        model,
        hardware,
        std::env::consts::ARCH,
        rust,
        commit,
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
