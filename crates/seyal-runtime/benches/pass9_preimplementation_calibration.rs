use std::time::Instant;

#[cfg(target_os = "macos")]
use std::{
    fs,
    io::{Read, Write},
    net::Shutdown,
    os::unix::net::UnixStream,
    path::PathBuf,
    process::{self, Command},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[cfg(target_os = "macos")]
use seyal_exec::{CommandSpec, WindowSize};
#[cfg(target_os = "macos")]
use seyal_protocol::pass8::CAP_BLOCK_METADATA;
#[cfg(target_os = "macos")]
use seyal_runtime::{
    AttachmentId, ExecutionId, LocalIpcMode, Runtime, RuntimeConfig, RuntimeId,
    display::decode_chunk,
    local_ipc::framing::{
        Attach, Attached, CAP_BINARY_DISPLAY, ClientHello, Detach, Detached, FrameHeader,
        HEADER_LEN, MessageType, Role, ServerHello, encode_frame,
    },
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
const QUIESCENT_SAMPLE_COUNT: usize = 5;
#[cfg(target_os = "macos")]
const QUIESCENT_SAMPLE_INTERVAL: Duration = Duration::from_millis(10);
#[cfg(target_os = "macos")]
const ATTACHMENT_SETTLE_TIMEOUT: Duration = Duration::from_secs(1);
#[cfg(target_os = "macos")]
static HARNESS_COUNTER: AtomicU64 = AtomicU64::new(0);

fn main() {
    let _contract_clock = Instant::now();

    #[cfg(not(target_os = "macos"))]
    println!(
        "pass9_preimplementation_calibration PLATFORM_LIMITED target_os!=macos {PERFORMANCE_CLAIM}"
    );

    #[cfg(target_os = "macos")]
    run_macos();
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug)]
enum LossMode {
    Graceful,
    Abrupt,
}

#[cfg(target_os = "macos")]
impl LossMode {
    fn label(self) -> &'static str {
        match self {
            Self::Graceful => "graceful_detach",
            Self::Abrupt => "abrupt_socket_loss",
        }
    }
}

#[cfg(target_os = "macos")]
fn run_macos() {
    println!(
        "pass9_preimplementation_calibration architecture=existing_production_Runtime_local_IPC_PTY geometry={}x{} warmup_cycles={} measured_cycles={} cohorts={} rss_samples={} rss_interval_ms={} percentile_method=nearest_rank {}",
        COLUMNS,
        ROWS,
        WARMUP_CYCLES,
        MEASURED_CYCLES,
        COHORTS,
        QUIESCENT_SAMPLE_COUNT,
        QUIESCENT_SAMPLE_INTERVAL.as_millis(),
        PERFORMANCE_CLAIM,
    );
    print_host_metadata();

    for mode in [LossMode::Graceful, LossMode::Abrupt] {
        let mut reconnect_p99 = Vec::with_capacity(COHORTS);
        let mut cleanup_p99 = Vec::with_capacity(COHORTS);
        let mut rss_deltas = Vec::with_capacity(COHORTS);

        for cohort in 0..COHORTS {
            let result = measure_cohort(mode, cohort + 1);
            reconnect_p99.push(result.reconnect.p99_us);
            cleanup_p99.push(result.cleanup.p99_us);
            rss_deltas.push(result.rss_delta_kib);
        }

        reconnect_p99.sort_by(|a, b| a.total_cmp(b));
        cleanup_p99.sort_by(|a, b| a.total_cmp(b));
        rss_deltas.sort_unstable();
        println!(
            "pass9_calibration_summary mode={} reconnect_boundary=connect_hello_attach_to_complete_authoritative_snapshot median_cohort_p99_us={:.3} cleanup_boundary=client_loss_dispatch_to_runtime_attachment_zero median_cohort_p99_us={:.3} median_cohort_rss_delta_kib={} cohorts={} cycles_per_cohort={} {}",
            mode.label(),
            reconnect_p99[COHORTS / 2],
            cleanup_p99[COHORTS / 2],
            rss_deltas[COHORTS / 2],
            COHORTS,
            MEASURED_CYCLES,
            PERFORMANCE_CLAIM,
        );
    }
}

#[cfg(target_os = "macos")]
struct CohortResult {
    reconnect: Stats,
    cleanup: Stats,
    rss_delta_kib: i64,
}

#[cfg(target_os = "macos")]
fn measure_cohort(mode: LossMode, cohort: usize) -> CohortResult {
    let harness = RuntimeHarness::start();
    let initial = harness.snapshot();
    assert_eq!(initial.runtime_id, harness.runtime_id);
    assert_eq!(initial.execution_id, harness.execution_id);
    assert_eq!(initial.attachment_count, 0);

    let mut previous_attachment = None;
    for _ in 0..WARMUP_CYCLES {
        let attachment = open_attachment(&harness);
        assert_fresh_attachment(&mut previous_attachment, attachment.attachment_id);
        cleanup_attachment(mode, &harness, attachment);
    }

    let baseline = median_quiescent_metrics(&harness);
    let mut reconnect_samples = Vec::with_capacity(MEASURED_CYCLES);
    let mut cleanup_samples = Vec::with_capacity(MEASURED_CYCLES);

    for _ in 0..MEASURED_CYCLES {
        let reconnect_start = Instant::now();
        let attachment = open_attachment(&harness);
        reconnect_samples.push(elapsed_ns(reconnect_start));
        assert_fresh_attachment(&mut previous_attachment, attachment.attachment_id);

        let attached = harness.snapshot();
        assert_eq!(attached.runtime_id, harness.runtime_id);
        assert_eq!(attached.execution_id, harness.execution_id);
        assert_eq!(attached.attachment_count, 1);

        let cleanup_start = Instant::now();
        cleanup_attachment(mode, &harness, attachment);
        cleanup_samples.push(elapsed_ns(cleanup_start));

        let quiescent = harness.snapshot();
        assert_eq!(quiescent.runtime_id, harness.runtime_id);
        assert_eq!(quiescent.execution_id, harness.execution_id);
        assert_eq!(quiescent.attachment_count, 0);
    }

    let final_metrics = median_quiescent_metrics(&harness);
    assert_eq!(
        baseline.fds,
        final_metrics.fds,
        "Pass 9 calibration detected FD growth in {} cohort {}",
        mode.label(),
        cohort
    );
    assert_eq!(
        baseline.threads,
        final_metrics.threads,
        "Pass 9 calibration detected thread growth in {} cohort {}",
        mode.label(),
        cohort
    );

    let idle_cpu = measure_idle_cpu();
    let reconnect = stats(&mut reconnect_samples);
    let cleanup = stats(&mut cleanup_samples);
    let rss_delta_kib = final_metrics.rss_kib as i64 - baseline.rss_kib as i64;

    println!(
        "pass9_calibration_cohort mode={} cohort={} runtime_id={:?} execution_id={:?} reconnect_boundary=connect_hello_attach_to_complete_authoritative_snapshot reconnect_p50_us={:.3} reconnect_p95_us={:.3} reconnect_p99_us={:.3} reconnect_max_us={:.3} cleanup_boundary=client_loss_dispatch_to_runtime_attachment_zero cleanup_p50_us={:.3} cleanup_p95_us={:.3} cleanup_p99_us={:.3} cleanup_max_us={:.3} rss_baseline_kib={} rss_final_kib={} rss_delta_kib={} fds_baseline={} fds_final={} threads_baseline={} threads_final={} idle_cpu_percent={:.3} sample_count={} {}",
        mode.label(),
        cohort,
        harness.runtime_id,
        harness.execution_id,
        reconnect.p50_us,
        reconnect.p95_us,
        reconnect.p99_us,
        reconnect.max_us,
        cleanup.p50_us,
        cleanup.p95_us,
        cleanup.p99_us,
        cleanup.max_us,
        baseline.rss_kib,
        final_metrics.rss_kib,
        rss_delta_kib,
        baseline.fds,
        final_metrics.fds,
        baseline.threads,
        final_metrics.threads,
        idle_cpu,
        MEASURED_CYCLES,
        PERFORMANCE_CLAIM,
    );

    harness.finish();
    CohortResult {
        reconnect,
        cleanup,
        rss_delta_kib,
    }
}

#[cfg(target_os = "macos")]
fn assert_fresh_attachment(previous: &mut Option<AttachmentId>, current: AttachmentId) {
    if let Some(previous) = previous {
        assert_ne!(*previous, current, "AttachmentId reused across reconnect");
    }
    *previous = Some(current);
}

#[cfg(target_os = "macos")]
fn cleanup_attachment(mode: LossMode, harness: &RuntimeHarness, mut attachment: RawAttachment) {
    match mode {
        LossMode::Graceful => {
            send_frame(
                &mut attachment.stream,
                MessageType::Detach,
                &Detach {
                    attachment_id: attachment.attachment_id,
                }
                .encode(),
            );
            let payload = read_until(&mut attachment.stream, MessageType::Detached as u16);
            let detached = Detached::decode(&payload).expect("Detached decode");
            assert_eq!(detached.attachment_id, attachment.attachment_id);
        }
        LossMode::Abrupt => {
            let _ = attachment.stream.shutdown(Shutdown::Both);
        }
    }
    drop(attachment);
    wait_for_attachment_count(harness, 0);
}

#[cfg(target_os = "macos")]
struct RawAttachment {
    stream: UnixStream,
    attachment_id: AttachmentId,
}

#[cfg(target_os = "macos")]
fn open_attachment(harness: &RuntimeHarness) -> RawAttachment {
    let mut stream = UnixStream::connect(&harness.socket_path).expect("connect Runtime socket");
    stream
        .set_read_timeout(Some(Duration::from_secs(1)))
        .expect("read timeout");
    stream
        .set_write_timeout(Some(Duration::from_secs(1)))
        .expect("write timeout");

    send_frame(
        &mut stream,
        MessageType::ClientHello,
        &ClientHello {
            client_capabilities: CAP_BINARY_DISPLAY | CAP_BLOCK_METADATA,
        }
        .encode(),
    );
    let hello_payload = read_until(&mut stream, MessageType::ServerHello as u16);
    let hello = ServerHello::decode(&hello_payload).expect("ServerHello decode");
    assert_eq!(
        hello.runtime_id,
        u128::from_le_bytes(harness.runtime_id.to_bytes())
    );

    send_frame(
        &mut stream,
        MessageType::Attach,
        &Attach {
            execution_id: harness.execution_id,
            requested_role: Role::Controller,
        }
        .encode(),
    );

    let mut attached = None;
    let mut snapshot_complete = false;
    let deadline = Instant::now() + Duration::from_secs(1);
    while attached.is_none() || !snapshot_complete {
        assert!(
            Instant::now() < deadline,
            "initial authoritative snapshot timed out"
        );
        let (kind, payload) = read_frame(&mut stream);
        if kind == MessageType::Attached as u16 {
            attached = Some(Attached::decode(&payload).expect("Attached decode"));
        } else if kind == MessageType::DisplaySnapshot as u16 {
            let chunk = decode_chunk(&payload).expect("DisplaySnapshot decode");
            assert_eq!(chunk.rows, ROWS);
            assert_eq!(chunk.columns, COLUMNS);
            snapshot_complete = chunk.chunk_index + 1 == chunk.chunk_count;
        }
    }

    let attached = attached.expect("Attached frame");
    assert_eq!(attached.execution_id, harness.execution_id);
    RawAttachment {
        stream,
        attachment_id: attached.attachment_id,
    }
}

#[cfg(target_os = "macos")]
fn send_frame(stream: &mut UnixStream, kind: MessageType, payload: &[u8]) {
    stream
        .write_all(&encode_frame(kind, payload))
        .expect("write local IPC frame");
}

#[cfg(target_os = "macos")]
fn read_until(stream: &mut UnixStream, wanted: u16) -> Vec<u8> {
    loop {
        let (kind, payload) = read_frame(stream);
        if kind == wanted {
            return payload;
        }
        assert!(
            kind == MessageType::DisplaySnapshot as u16
                || kind == MessageType::DisplayDelta as u16
                || kind == MessageType::Attached as u16
                || kind == MessageType::ServerHello as u16
                || kind == seyal_protocol::pass8::BLOCK_STATE_MESSAGE_TYPE,
            "unexpected local IPC frame type {kind} while waiting for {wanted}"
        );
    }
}

#[cfg(target_os = "macos")]
fn read_frame(stream: &mut UnixStream) -> (u16, Vec<u8>) {
    let mut header_bytes = [0u8; HEADER_LEN];
    stream
        .read_exact(&mut header_bytes)
        .expect("read local IPC frame header");
    let header = FrameHeader::decode(&header_bytes).expect("decode local IPC frame header");
    let mut payload = vec![0u8; header.payload_len as usize];
    stream
        .read_exact(&mut payload)
        .expect("read local IPC frame payload");
    (header.message_type, payload)
}

#[cfg(target_os = "macos")]
fn wait_for_attachment_count(harness: &RuntimeHarness, expected: usize) {
    let deadline = Instant::now() + ATTACHMENT_SETTLE_TIMEOUT;
    loop {
        let snapshot = harness.snapshot();
        if snapshot.attachment_count == expected {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "attachment count failed to settle: expected {expected}, observed {}",
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
    socket_path: PathBuf,
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
                .join(format!("s9c-{}-{suffix:x}-{nonce:x}.lock", process::id()));
            let runtime_dir =
                std::env::temp_dir().join(format!("s9cd-{}-{suffix:x}-{nonce:x}", process::id()));
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
struct ProcessMetrics {
    rss_kib: usize,
    threads: usize,
    fds: usize,
}

#[cfg(target_os = "macos")]
fn median_quiescent_metrics(harness: &RuntimeHarness) -> ProcessMetrics {
    let snapshot = harness.snapshot();
    assert_eq!(snapshot.attachment_count, 0);
    let mut rss = Vec::with_capacity(QUIESCENT_SAMPLE_COUNT);
    let mut threads = Vec::with_capacity(QUIESCENT_SAMPLE_COUNT);
    let mut fds = Vec::with_capacity(QUIESCENT_SAMPLE_COUNT);
    for sample in 0..QUIESCENT_SAMPLE_COUNT {
        let metrics = process_metrics();
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
    }
}

#[cfg(target_os = "macos")]
fn process_metrics() -> ProcessMetrics {
    let pid = process::id();
    let output = Command::new("/bin/ps")
        .args(["-o", "rss=,thcount=", "-p", &pid.to_string()])
        .output()
        .expect("ps metrics");
    let line = String::from_utf8_lossy(&output.stdout);
    let mut fields = line.split_whitespace();
    let rss_kib = fields
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    let threads = fields
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    let fds = fs::read_dir("/dev/fd")
        .map(|entries| entries.count())
        .unwrap_or(0);
    ProcessMetrics {
        rss_kib,
        threads,
        fds,
    }
}

#[cfg(target_os = "macos")]
fn measure_idle_cpu() -> f64 {
    let started_cpu = process_cpu_seconds();
    let started = Instant::now();
    thread::sleep(Duration::from_millis(250));
    let elapsed = started.elapsed().as_secs_f64();
    let cpu = (process_cpu_seconds() - started_cpu).max(0.0);
    if elapsed == 0.0 {
        0.0
    } else {
        cpu / elapsed * 100.0
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
        "pass9_calibration_host macos_version={} macos_build={} model={:?} hardware={:?} arch={} rust={:?} build_mode=release commit={} master_baseline=efa365d48565fb09452b683577700a8e5e267fcb pass8_baseline=d9d21187e8429bbd3dbeb3e1c7cc4d05c1d147e6 {}",
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
