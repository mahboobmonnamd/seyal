use std::time::Instant;

#[cfg(target_os = "macos")]
use std::{
    env, fs,
    io::{BufRead, BufReader, Read, Write},
    net::Shutdown,
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
    process::{self, Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[cfg(target_os = "macos")]
use seyal_exec::{CommandSpec, WindowSize};
#[cfg(target_os = "macos")]
use seyal_protocol::pass8::{BLOCK_STATE_MESSAGE_TYPE, CAP_BLOCK_METADATA};
#[cfg(target_os = "macos")]
use seyal_render::{
    CellSource, CommittedDisplay, CursorState, PreparedSurface, RenderAttributes, RenderCell,
    RenderColor, RowDamage,
};
#[cfg(target_os = "macos")]
use seyal_runtime::{
    AttachmentId, ExecutionId, LocalIpcMode, Runtime, RuntimeConfig,
    display::{DecodedDisplayChunk, DisplayAttributes, DisplayCache, DisplayCell, DisplayColor, decode_chunk, empty_cache},
    local_ipc::framing::{
        Attach, Attached, CAP_COMMAND_BLOCKS, ClientHello, Detach, Detached, ErrorMessage,
        ExecutionList, FrameHeader, HEADER_LEN, MessageType, Role, ServerHello, encode_frame,
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
const QUIESCENT_SAMPLE_COUNT: usize = 5;
#[cfg(target_os = "macos")]
const QUIESCENT_SAMPLE_INTERVAL: Duration = Duration::from_millis(10);
#[cfg(target_os = "macos")]
const SETTLE_TIMEOUT: Duration = Duration::from_secs(2);

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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

    fn parse(value: &str) -> Self {
        match value {
            "graceful_detach" => Self::Graceful,
            "abrupt_socket_loss" => Self::Abrupt,
            _ => panic!("unknown Pass 9 loss mode: {value}"),
        }
    }
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Geometry {
    columns: u16,
    rows: u16,
}

#[cfg(target_os = "macos")]
impl Geometry {
    const PRIMARY: Self = Self {
        columns: 120,
        rows: 40,
    };
    const REPRESENTATIVE: Self = Self {
        columns: 80,
        rows: 24,
    };

    fn label(self) -> String {
        format!("{}x{}", self.columns, self.rows)
    }

    fn parse(value: &str) -> Self {
        let (columns, rows) = value.split_once('x').expect("geometry columnsxrows");
        Self {
            columns: columns.parse().expect("geometry columns"),
            rows: rows.parse().expect("geometry rows"),
        }
    }
}

#[cfg(target_os = "macos")]
fn run_macos() {
    let args = env::args().collect::<Vec<_>>();
    if args.get(1).is_some_and(|arg| arg == "--runtime-worker") {
        let geometry = Geometry::parse(args.get(2).expect("worker geometry"));
        run_runtime_worker(geometry);
        return;
    }
    if args.get(1).is_some_and(|arg| arg == "--cohort") {
        let mode = LossMode::parse(args.get(2).expect("cohort mode"));
        let geometry = Geometry::parse(args.get(3).expect("cohort geometry"));
        let cohort: usize = args
            .get(4)
            .expect("cohort number")
            .parse()
            .expect("cohort integer");
        run_cohort(mode, geometry, cohort);
        return;
    }
    run_orchestrator();
}

#[cfg(target_os = "macos")]
fn run_orchestrator() {
    println!(
        "pass9_preimplementation_calibration_v2 architecture=separate_client_cohort_process_plus_fresh_Runtime_worker_process production_Runtime_local_IPC_PTY warmup_cycles={} measured_cycles={} cohorts={} geometries=120x40,80x24 percentile_method=nearest_rank rss_samples={} rss_interval_ms={} cleanup_measurement=runtime_poll_dispatch_window_upper_bound exact_dispatch_timer=false {}",
        WARMUP_CYCLES,
        MEASURED_CYCLES,
        COHORTS,
        QUIESCENT_SAMPLE_COUNT,
        QUIESCENT_SAMPLE_INTERVAL.as_millis(),
        PERFORMANCE_CLAIM,
    );
    print_host_metadata();

    for geometry in [Geometry::PRIMARY, Geometry::REPRESENTATIVE] {
        for mode in [LossMode::Graceful, LossMode::Abrupt] {
            let mut reconnect_p99 = Vec::with_capacity(COHORTS);
            let mut renderer_ready_p99 = Vec::with_capacity(COHORTS);
            let mut cleanup_window_p99 = Vec::with_capacity(COHORTS);
            let mut runtime_rss_delta = Vec::with_capacity(COHORTS);
            let mut client_rss_delta = Vec::with_capacity(COHORTS);

            for cohort in 1..=COHORTS {
                let output = Command::new(env::current_exe().expect("benchmark executable"))
                    .arg("--cohort")
                    .arg(mode.label())
                    .arg(geometry.label())
                    .arg(cohort.to_string())
                    .output()
                    .expect("spawn isolated client cohort process");
                print!("{}", String::from_utf8_lossy(&output.stdout));
                eprint!("{}", String::from_utf8_lossy(&output.stderr));
                assert!(
                    output.status.success(),
                    "Pass 9 isolated cohort failed: mode={} geometry={} cohort={cohort}",
                    mode.label(),
                    geometry.label()
                );
                let result = parse_result_line(&String::from_utf8_lossy(&output.stdout));
                reconnect_p99.push(result.reconnect_p99_us);
                renderer_ready_p99.push(result.renderer_ready_p99_us);
                cleanup_window_p99.push(result.cleanup_window_p99_us);
                runtime_rss_delta.push(result.runtime_rss_delta_kib);
                client_rss_delta.push(result.client_rss_delta_kib);
            }

            reconnect_p99.sort_by(|a, b| a.total_cmp(b));
            renderer_ready_p99.sort_by(|a, b| a.total_cmp(b));
            cleanup_window_p99.sort_by(|a, b| a.total_cmp(b));
            runtime_rss_delta.sort_unstable();
            client_rss_delta.sort_unstable();
            println!(
                "pass9_calibration_summary mode={} geometry={} reconnect_boundary=local_connect_hello_resolve_attach_to_complete_authoritative_client_commit median_cohort_p99_us={:.3} renderer_ready_boundary=committed_client_state_to_PreparedSurface_ready median_renderer_cohort_p99_us={:.3} cleanup_boundary=runtime_poll_dispatch_window_containing_loss_to_attachment_cleanup median_cleanup_window_cohort_p99_us={:.3} cleanup_classification=UPPER_BOUND_NOT_EXACT_DISPATCH runtime_median_cohort_rss_delta_kib={} client_median_cohort_rss_delta_kib={} cohorts={} cycles_per_cohort={} {}",
                mode.label(),
                geometry.label(),
                reconnect_p99[COHORTS / 2],
                renderer_ready_p99[COHORTS / 2],
                cleanup_window_p99[COHORTS / 2],
                runtime_rss_delta[COHORTS / 2],
                client_rss_delta[COHORTS / 2],
                COHORTS,
                MEASURED_CYCLES,
                PERFORMANCE_CLAIM,
            );
        }
    }
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy)]
struct CohortResult {
    reconnect_p99_us: f64,
    renderer_ready_p99_us: f64,
    cleanup_window_p99_us: f64,
    runtime_rss_delta_kib: i64,
    client_rss_delta_kib: i64,
}

#[cfg(target_os = "macos")]
fn parse_result_line(output: &str) -> CohortResult {
    let line = output
        .lines()
        .find(|line| line.starts_with("PASS9_RESULT "))
        .expect("cohort result line");
    let field = |name: &str| -> &str {
        line.split_whitespace()
            .find_map(|part| part.strip_prefix(&format!("{name}=")))
            .unwrap_or_else(|| panic!("missing cohort result field {name}"))
    };
    CohortResult {
        reconnect_p99_us: field("reconnect_p99_us").parse().expect("reconnect p99"),
        renderer_ready_p99_us: field("renderer_ready_p99_us")
            .parse()
            .expect("renderer p99"),
        cleanup_window_p99_us: field("cleanup_window_p99_us")
            .parse()
            .expect("cleanup p99"),
        runtime_rss_delta_kib: field("runtime_rss_delta_kib")
            .parse()
            .expect("runtime RSS delta"),
        client_rss_delta_kib: field("client_rss_delta_kib")
            .parse()
            .expect("client RSS delta"),
    }
}

#[cfg(target_os = "macos")]
fn run_cohort(mode: LossMode, geometry: Geometry, cohort: usize) {
    let mut worker = RuntimeWorker::start(geometry);
    let client_baseline = median_self_metrics();
    let runtime_baseline = worker.median_metrics();
    assert_quiescent(&mut worker, runtime_baseline, client_baseline);

    let mut previous_attachment = None;
    for _ in 0..WARMUP_CYCLES {
        let attachment = open_attachment(&worker, geometry);
        assert_fresh_attachment(&mut previous_attachment, attachment.attachment_id);
        cleanup_attachment(mode, &mut worker, attachment);
        assert_quiescent(&mut worker, runtime_baseline, client_baseline);
    }

    let runtime_rss_baseline = worker.median_metrics().rss_kib;
    let client_rss_baseline = median_self_metrics().rss_kib;
    let mut reconnect_samples = Vec::with_capacity(MEASURED_CYCLES);
    let mut renderer_ready_samples = Vec::with_capacity(MEASURED_CYCLES);
    let mut cleanup_window_samples = Vec::with_capacity(MEASURED_CYCLES);
    let mut runtime_rss_cycles = Vec::with_capacity(MEASURED_CYCLES);
    let mut client_rss_cycles = Vec::with_capacity(MEASURED_CYCLES);

    for _ in 0..MEASURED_CYCLES {
        let attachment = open_attachment(&worker, geometry);
        reconnect_samples.push(attachment.reconnect_ns);
        renderer_ready_samples.push(attachment.renderer_ready_ns);
        assert_fresh_attachment(&mut previous_attachment, attachment.attachment_id);

        let attached = query_execution_status(&worker.socket_path, worker.execution_id);
        assert_eq!(attached.attachment_count, 1);
        assert!(attached.has_controller);

        cleanup_attachment(mode, &mut worker, attachment);
        let cleanup_window = worker.read_cleanup_window();
        cleanup_window_samples.push(cleanup_window);
        assert_quiescent(&mut worker, runtime_baseline, client_baseline);
        runtime_rss_cycles.push(worker.metrics().rss_kib);
        client_rss_cycles.push(self_metrics().rss_kib);
    }

    let runtime_final = worker.median_metrics();
    let client_final = median_self_metrics();
    let idle_cpu = worker.measure_idle_cpu();
    let reconnect = stats(&mut reconnect_samples);
    let renderer_ready = stats(&mut renderer_ready_samples);
    let cleanup_window = stats(&mut cleanup_window_samples);
    let runtime_rss_delta_kib = runtime_final.rss_kib as i64 - runtime_rss_baseline as i64;
    let client_rss_delta_kib = client_final.rss_kib as i64 - client_rss_baseline as i64;
    let runtime_cycle_growth = signed_growth(&runtime_rss_cycles);
    let client_cycle_growth = signed_growth(&client_rss_cycles);

    println!(
        "pass9_calibration_cohort mode={} geometry={} cohort={} runtime_pid={} runtime_id={} execution_id={} reconnect_boundary=local_connect_hello_resolve_attach_to_complete_authoritative_client_commit reconnect_p50_us={:.3} reconnect_p95_us={:.3} reconnect_p99_us={:.3} reconnect_max_us={:.3} renderer_ready_boundary=committed_client_state_to_PreparedSurface_ready renderer_p50_us={:.3} renderer_p95_us={:.3} renderer_p99_us={:.3} renderer_max_us={:.3} cleanup_boundary=runtime_poll_dispatch_window_containing_loss_to_attachment_cleanup cleanup_classification=UPPER_BOUND_NOT_EXACT_DISPATCH cleanup_p50_us={:.3} cleanup_p95_us={:.3} cleanup_p99_us={:.3} cleanup_max_us={:.3} runtime_rss_baseline_kib={} runtime_rss_final_kib={} runtime_rss_delta_kib={} runtime_cycle_rss_growth_kib={} client_rss_baseline_kib={} client_rss_final_kib={} client_rss_delta_kib={} client_cycle_rss_growth_kib={} runtime_fds_baseline={} runtime_fds_final={} runtime_threads_baseline={} runtime_threads_final={} client_fds_baseline={} client_fds_final={} client_threads_baseline={} client_threads_final={} idle_runtime_cpu_percent={:.3} attachment_controller_fd_thread_return_each_cycle=true client_socket_closed_each_cycle=true retry_work=NOT_IMPLEMENTED_IN_PRE_PASS9_BASELINE sample_count={} {}",
        mode.label(),
        geometry.label(),
        cohort,
        worker.pid,
        worker.runtime_id,
        u128::from_le_bytes(worker.execution_id.to_bytes()),
        reconnect.p50_us,
        reconnect.p95_us,
        reconnect.p99_us,
        reconnect.max_us,
        renderer_ready.p50_us,
        renderer_ready.p95_us,
        renderer_ready.p99_us,
        renderer_ready.max_us,
        cleanup_window.p50_us,
        cleanup_window.p95_us,
        cleanup_window.p99_us,
        cleanup_window.max_us,
        runtime_rss_baseline,
        runtime_final.rss_kib,
        runtime_rss_delta_kib,
        runtime_cycle_growth,
        client_rss_baseline,
        client_final.rss_kib,
        client_rss_delta_kib,
        client_cycle_growth,
        runtime_baseline.fds,
        runtime_final.fds,
        runtime_baseline.threads,
        runtime_final.threads,
        client_baseline.fds,
        client_final.fds,
        client_baseline.threads,
        client_final.threads,
        idle_cpu,
        MEASURED_CYCLES,
        PERFORMANCE_CLAIM,
    );
    println!(
        "PASS9_RESULT reconnect_p99_us={:.3} renderer_ready_p99_us={:.3} cleanup_window_p99_us={:.3} runtime_rss_delta_kib={} client_rss_delta_kib={}",
        reconnect.p99_us,
        renderer_ready.p99_us,
        cleanup_window.p99_us,
        runtime_rss_delta_kib,
        client_rss_delta_kib,
    );
    worker.finish();
}

#[cfg(target_os = "macos")]
fn signed_growth(samples: &[usize]) -> i64 {
    match (samples.first(), samples.last()) {
        (Some(first), Some(last)) => *last as i64 - *first as i64,
        _ => 0,
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
fn assert_quiescent(
    worker: &mut RuntimeWorker,
    runtime_baseline: ProcessMetrics,
    client_baseline: ProcessMetrics,
) {
    let deadline = Instant::now() + SETTLE_TIMEOUT;
    loop {
        let status = query_execution_status(&worker.socket_path, worker.execution_id);
        assert_eq!(status.execution_id, worker.execution_id);
        if status.attachment_count == 0 && !status.has_controller {
            let first = worker.metrics();
            let client = self_metrics();
            if first.attachment_count == 0
                && first.fds == runtime_baseline.fds
                && first.threads == runtime_baseline.threads
                && client.fds == client_baseline.fds
                && client.threads == client_baseline.threads
            {
                thread::yield_now();
                let second = worker.metrics();
                if second.attachment_count == 0
                    && second.fds == runtime_baseline.fds
                    && second.threads == runtime_baseline.threads
                {
                    return;
                }
            }
        }
        assert!(Instant::now() < deadline, "Pass 9 lifecycle failed to quiesce");
        thread::yield_now();
    }
}

#[cfg(target_os = "macos")]
struct RawAttachment {
    stream: UnixStream,
    attachment_id: AttachmentId,
    reconnect_ns: u64,
    renderer_ready_ns: u64,
}

#[cfg(target_os = "macos")]
fn open_attachment(worker: &RuntimeWorker, geometry: Geometry) -> RawAttachment {
    let reconnect_started = Instant::now();
    let mut stream = UnixStream::connect(&worker.socket_path).expect("connect Runtime socket");
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("read timeout");
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .expect("write timeout");

    send_frame(
        &mut stream,
        MessageType::ClientHello,
        &ClientHello {
            client_capabilities: CAP_COMMAND_BLOCKS | CAP_BLOCK_METADATA,
        }
        .encode(),
    );
    let hello_payload = read_until(&mut stream, MessageType::ServerHello as u16);
    let hello = ServerHello::decode(&hello_payload).expect("ServerHello decode");
    assert_eq!(hello.runtime_id, worker.runtime_id);

    send_frame(&mut stream, MessageType::ListExecutions, &[]);
    let list_payload = read_until(&mut stream, MessageType::ExecutionList as u16);
    let list = ExecutionList::decode(&list_payload).expect("ExecutionList decode");
    assert!(
        list.entries
            .iter()
            .any(|entry| entry.execution_id == worker.execution_id),
        "target execution missing during reconnect resolve"
    );

    send_frame(
        &mut stream,
        MessageType::Attach,
        &Attach {
            execution_id: worker.execution_id,
            requested_role: Role::Controller,
        }
        .encode(),
    );

    let mut attached = None;
    let mut chunks = Vec::<DecodedDisplayChunk>::new();
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        assert!(Instant::now() < deadline, "authoritative snapshot timed out");
        let (kind, payload) = read_frame(&mut stream);
        if kind == MessageType::Attached as u16 {
            attached = Some(Attached::decode(&payload).expect("Attached decode"));
        } else if kind == MessageType::DisplaySnapshot as u16 {
            let chunk = decode_chunk(&payload).expect("DisplaySnapshot decode");
            assert_eq!(chunk.rows, geometry.rows);
            assert_eq!(chunk.columns, geometry.columns);
            let complete = chunk.chunk_index + 1 == chunk.chunk_count;
            chunks.push(chunk);
            if complete && attached.is_some() {
                break;
            }
        } else if kind == BLOCK_STATE_MESSAGE_TYPE {
            continue;
        } else if kind == MessageType::Error as u16 {
            panic_server_error(&payload, "attach/snapshot");
        } else {
            panic!("unexpected frame type {kind} during authoritative reconnect");
        }
    }

    let attached = attached.expect("Attached frame");
    assert_eq!(attached.execution_id, worker.execution_id);
    assert_eq!(attached.granted_role, Role::Controller);
    let mut cache = empty_cache();
    cache
        .apply_chunks(&chunks)
        .expect("authoritative client cache commit");
    assert_eq!(cache.rows, geometry.rows);
    assert_eq!(cache.columns, geometry.columns);
    assert_eq!(cache.generation, attached.current_generation);
    let committed_at = Instant::now();
    let reconnect_ns = elapsed_ns(reconnect_started);

    let mut prepared = PreparedSurface::default();
    prepare_surface(&mut prepared, &cache);
    assert_eq!(prepared.rows(), geometry.rows);
    assert_eq!(prepared.columns(), geometry.columns);
    assert_eq!(prepared.generation(), Some(cache.generation));
    let renderer_ready_ns = elapsed_ns(committed_at);

    RawAttachment {
        stream,
        attachment_id: attached.attachment_id,
        reconnect_ns,
        renderer_ready_ns,
    }
}

#[cfg(target_os = "macos")]
fn cleanup_attachment(mode: LossMode, worker: &mut RuntimeWorker, mut attachment: RawAttachment) {
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
            let _ = attachment.stream.shutdown(Shutdown::Both);
        }
        LossMode::Abrupt => {
            let _ = attachment.stream.shutdown(Shutdown::Both);
        }
    }
    drop(attachment);
    worker.expect_cleanup_transition = true;
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy)]
struct ExecutionStatus {
    execution_id: ExecutionId,
    attachment_count: usize,
    has_controller: bool,
}

#[cfg(target_os = "macos")]
fn query_execution_status(socket_path: &Path, execution_id: ExecutionId) -> ExecutionStatus {
    let mut stream = UnixStream::connect(socket_path).expect("status connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("status read timeout");
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .expect("status write timeout");
    send_frame(
        &mut stream,
        MessageType::ClientHello,
        &ClientHello {
            client_capabilities: CAP_COMMAND_BLOCKS | CAP_BLOCK_METADATA,
        }
        .encode(),
    );
    let hello = read_until(&mut stream, MessageType::ServerHello as u16);
    ServerHello::decode(&hello).expect("status ServerHello");
    send_frame(&mut stream, MessageType::ListExecutions, &[]);
    let payload = read_until(&mut stream, MessageType::ExecutionList as u16);
    let list = ExecutionList::decode(&payload).expect("status ExecutionList");
    let entry = list
        .entries
        .into_iter()
        .find(|entry| entry.execution_id == execution_id)
        .expect("status execution");
    ExecutionStatus {
        execution_id,
        attachment_count: usize::from(entry.attachment_count),
        has_controller: entry.has_controller,
    }
}

#[cfg(target_os = "macos")]
struct RuntimeCells<'a>(&'a [DisplayCell]);

#[cfg(target_os = "macos")]
impl CellSource for RuntimeCells<'_> {
    fn len(&self) -> usize {
        self.0.len()
    }

    fn cell(&self, index: usize) -> Option<RenderCell> {
        self.0.get(index).copied().map(runtime_cell_to_render)
    }
}

#[cfg(target_os = "macos")]
fn prepare_surface(prepared: &mut PreparedSurface, cache: &DisplayCache) {
    let source = RuntimeCells(&cache.cells);
    prepared
        .prepare(
            CommittedDisplay {
                generation: cache.generation,
                rows: cache.rows,
                columns: cache.columns,
                cursor: CursorState::new(cache.cursor_row, cache.cursor_col, cache.cursor_visible),
                alternate_screen: cache.alternate_screen,
                cells: &source,
            },
            RowDamage::full(cache.rows),
            true,
        )
        .expect("PreparedSurface commit");
}

#[cfg(target_os = "macos")]
fn runtime_cell_to_render(cell: DisplayCell) -> RenderCell {
    RenderCell {
        scalar: cell.scalar,
        foreground: match cell.foreground {
            DisplayColor::Default => RenderColor::Default,
            DisplayColor::Indexed(index) => RenderColor::Indexed(index),
            DisplayColor::Rgb { r, g, b } => RenderColor::Rgb { r, g, b },
        },
        background: match cell.background {
            DisplayColor::Default => RenderColor::Default,
            DisplayColor::Indexed(index) => RenderColor::Indexed(index),
            DisplayColor::Rgb { r, g, b } => RenderColor::Rgb { r, g, b },
        },
        attributes: runtime_attributes_to_render(cell.attributes),
    }
}

#[cfg(target_os = "macos")]
fn runtime_attributes_to_render(attributes: DisplayAttributes) -> RenderAttributes {
    RenderAttributes {
        bold: attributes.bold,
        underline: attributes.underline,
        inverse: attributes.inverse,
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
        if kind == MessageType::Error as u16 {
            panic_server_error(&payload, &format!("waiting for {wanted}"));
        }
        assert!(
            kind == MessageType::DisplaySnapshot as u16
                || kind == MessageType::DisplayDelta as u16
                || kind == MessageType::Attached as u16
                || kind == MessageType::ServerHello as u16
                || kind == BLOCK_STATE_MESSAGE_TYPE,
            "unexpected local IPC frame type {kind} while waiting for {wanted}"
        );
    }
}

#[cfg(target_os = "macos")]
fn panic_server_error(payload: &[u8], context: &str) -> ! {
    let error = ErrorMessage::decode(payload).expect("Error frame decode");
    panic!(
        "Runtime Error during {context}: code={} offending_type={} detail={}",
        error.error_code, error.offending_message_type, error.detail_code
    );
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
struct RuntimeWorker {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    pid: u32,
    socket_path: PathBuf,
    runtime_id: u128,
    execution_id: ExecutionId,
    expect_cleanup_transition: bool,
}

#[cfg(target_os = "macos")]
impl RuntimeWorker {
    fn start(geometry: Geometry) -> Self {
        let mut child = Command::new(env::current_exe().expect("benchmark executable"))
            .arg("--runtime-worker")
            .arg(geometry.label())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("spawn fresh Runtime worker process");
        let stdin = child.stdin.take().expect("worker stdin");
        let mut stdout = BufReader::new(child.stdout.take().expect("worker stdout"));
        let mut line = String::new();
        stdout.read_line(&mut line).expect("worker READY line");
        let fields = line.trim_end().split('\t').collect::<Vec<_>>();
        assert_eq!(fields.first().copied(), Some("READY"));
        let pid: u32 = fields[1].parse().expect("worker pid");
        assert_eq!(pid, child.id());
        let socket_path = PathBuf::from(fields[2]);
        let runtime_id = fields[3].parse().expect("RuntimeId u128");
        let execution_raw: u128 = fields[4].parse().expect("ExecutionId u128");
        Self {
            child,
            stdin,
            stdout,
            pid,
            socket_path,
            runtime_id,
            execution_id: ExecutionId::from_bytes(execution_raw.to_le_bytes()),
            expect_cleanup_transition: false,
        }
    }

    fn send_command(&mut self, command: &str) {
        writeln!(self.stdin, "{command}").expect("worker command write");
        self.stdin.flush().expect("worker command flush");
    }

    fn read_line_with_prefix(&mut self, prefix: &str) -> String {
        loop {
            let mut line = String::new();
            let count = self.stdout.read_line(&mut line).expect("worker response");
            assert_ne!(count, 0, "Runtime worker exited before {prefix}");
            let trimmed = line.trim_end();
            if trimmed.starts_with(prefix) {
                return trimmed.to_owned();
            }
            assert!(
                trimmed.starts_with("CLEANUP\t"),
                "unexpected Runtime worker output: {trimmed}"
            );
            if self.expect_cleanup_transition {
                panic!("cleanup transition consumed before explicit read: {trimmed}");
            }
        }
    }

    fn metrics(&mut self) -> ProcessMetrics {
        self.send_command("metrics");
        let line = self.read_line_with_prefix("METRICS\t");
        parse_worker_metrics(&line)
    }

    fn median_metrics(&mut self) -> ProcessMetrics {
        let mut rss = Vec::with_capacity(QUIESCENT_SAMPLE_COUNT);
        let mut threads = Vec::with_capacity(QUIESCENT_SAMPLE_COUNT);
        let mut fds = Vec::with_capacity(QUIESCENT_SAMPLE_COUNT);
        let mut attachment_count = 0;
        for sample in 0..QUIESCENT_SAMPLE_COUNT {
            let metrics = self.metrics();
            rss.push(metrics.rss_kib);
            threads.push(metrics.threads);
            fds.push(metrics.fds);
            attachment_count = metrics.attachment_count;
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
            attachment_count,
        }
    }

    fn read_cleanup_window(&mut self) -> u64 {
        let line = self.read_line_with_prefix("CLEANUP\t");
        self.expect_cleanup_transition = false;
        line.split('\t')
            .nth(1)
            .expect("cleanup ns")
            .parse()
            .expect("cleanup integer")
    }

    fn measure_idle_cpu(&mut self) -> f64 {
        let started_cpu = process_cpu_seconds(self.pid);
        let started = Instant::now();
        thread::sleep(Duration::from_millis(250));
        let elapsed = started.elapsed().as_secs_f64();
        let cpu = (process_cpu_seconds(self.pid) - started_cpu).max(0.0);
        if elapsed == 0.0 { 0.0 } else { cpu / elapsed * 100.0 }
    }

    fn finish(mut self) {
        self.send_command("stop");
        let status = self.child.wait().expect("Runtime worker wait");
        assert!(status.success(), "Runtime worker shutdown failed");
    }
}

#[cfg(target_os = "macos")]
fn parse_worker_metrics(line: &str) -> ProcessMetrics {
    let fields = line.split('\t').collect::<Vec<_>>();
    assert_eq!(fields[0], "METRICS");
    ProcessMetrics {
        rss_kib: fields[1].parse().expect("worker RSS"),
        threads: fields[2].parse().expect("worker threads"),
        fds: fields[3].parse().expect("worker fds"),
        attachment_count: fields[4].parse().expect("worker attachments"),
    }
}

#[cfg(target_os = "macos")]
fn run_runtime_worker(geometry: Geometry) {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let mut config = RuntimeConfig::m001().expect("M001 Runtime config");
    config.singleton_path = env::temp_dir().join(format!("s9w-{}-{nonce:x}.lock", process::id()));
    let runtime_dir = env::temp_dir().join(format!("s9wd-{}-{nonce:x}", process::id()));
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
            WindowSize::cells(geometry.columns, geometry.rows).expect("benchmark geometry"),
        )
        .expect("execution");
    let socket_path = runtime
        .local_ipc_socket_path()
        .expect("Runtime socket path")
        .to_path_buf();
    println!(
        "READY\t{}\t{}\t{}\t{}",
        process::id(),
        socket_path.display(),
        u128::from_le_bytes(runtime.id().to_bytes()),
        u128::from_le_bytes(execution_id.to_bytes())
    );
    std::io::stdout().flush().expect("READY flush");

    let (command_tx, command_rx) = mpsc::channel::<String>();
    let command_thread = thread::spawn(move || {
        let stdin = std::io::stdin();
        for line in stdin.lock().lines() {
            match line {
                Ok(line) => {
                    let stop = line == "stop";
                    if command_tx.send(line).is_err() || stop {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    let mut stop = false;
    while !stop {
        let before = runtime
            .lookup(execution_id)
            .expect("live execution")
            .attachment_count;
        let poll_started = Instant::now();
        runtime
            .poll_once(Some(Duration::from_millis(2)))
            .expect("Runtime poll");
        let poll_ns = elapsed_ns(poll_started);
        let after = runtime
            .lookup(execution_id)
            .expect("live execution")
            .attachment_count;
        if before > 0 && after == 0 {
            println!("CLEANUP\t{poll_ns}");
            std::io::stdout().flush().expect("cleanup flush");
        }

        while let Ok(command) = command_rx.try_recv() {
            match command.as_str() {
                "metrics" => {
                    let metrics = self_metrics();
                    let attachment_count = runtime
                        .lookup(execution_id)
                        .expect("live execution")
                        .attachment_count;
                    println!(
                        "METRICS\t{}\t{}\t{}\t{}",
                        metrics.rss_kib, metrics.threads, metrics.fds, attachment_count
                    );
                    std::io::stdout().flush().expect("metrics flush");
                }
                "stop" => {
                    stop = true;
                    break;
                }
                other => panic!("unknown Runtime worker command: {other}"),
            }
        }
    }

    runtime.begin_shutdown().expect("begin Runtime shutdown");
    runtime
        .run_until_empty(Instant::now() + Duration::from_secs(3))
        .expect("Runtime shutdown");
    drop(runtime);
    command_thread.join().expect("worker command thread");
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ProcessMetrics {
    rss_kib: usize,
    threads: usize,
    fds: usize,
    attachment_count: usize,
}

#[cfg(target_os = "macos")]
fn median_self_metrics() -> ProcessMetrics {
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

#[cfg(target_os = "macos")]
fn self_metrics() -> ProcessMetrics {
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
        .expect("RSS metric");
    let threads = fields
        .next()
        .and_then(|value| value.parse().ok())
        .expect("thread metric");
    let fds = fs::read_dir("/dev/fd")
        .expect("/dev/fd")
        .count();
    ProcessMetrics {
        rss_kib,
        threads,
        fds,
        attachment_count: 0,
    }
}

#[cfg(target_os = "macos")]
fn process_cpu_seconds(pid: u32) -> f64 {
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
        env::consts::ARCH,
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
