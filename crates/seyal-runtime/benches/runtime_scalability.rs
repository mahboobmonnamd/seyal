#[cfg(target_os = "macos")]
use std::{
    env, fs,
    io::{Read, Write},
    os::fd::{AsRawFd, OwnedFd},
    os::unix::net::UnixStream,
    process::{self, Command},
    thread,
    time::{Duration, Instant},
};

#[cfg(target_os = "macos")]
use seyal_exec::{CommandSpec, WindowSize};
#[cfg(target_os = "macos")]
use seyal_runtime::{
    ExecutionId, LocalIpcMode, Runtime, RuntimeConfig,
    local_ipc::{
        fd_transfer::{self, RecvFd},
        framing::{
            Attach, Attached, ClientHello, FrameHeader, GenerationWake, HEADER_LEN, MessageType,
            Resync, Role, ServerHello, encode_frame,
        },
    },
    projection::{
        layout::{CELL_LEN, CellRecord, DAMAGE_LEN, DamageRecord, ModeFlags, RegionHeader},
        lifecycle::ReadOnlyMapping,
        producer::{self, OwnedSnapshot},
        writer::{SnapshotRead, read_latest, read_region_header},
    },
};

#[cfg(target_os = "macos")]
const MAX_VISIBLE_ATTACHMENTS: usize = 16;
#[cfg(target_os = "macos")]
const REFERENCE_HEADER_LEN: usize = 40;
#[cfg(target_os = "macos")]
const REFERENCE_MAGIC: [u8; 8] = *b"SYLSOCK1";

#[cfg(target_os = "macos")]
#[derive(Clone, Copy)]
enum TransportMode {
    SocketOnly,
    HybridProjection,
}

#[cfg(target_os = "macos")]
impl TransportMode {
    fn from_arg(value: &str) -> Self {
        match value {
            "socket-only" => Self::SocketOnly,
            "hybrid" => Self::HybridProjection,
            _ => panic!("unknown transport mode {value}"),
        }
    }

    fn as_arg(self) -> &'static str {
        match self {
            Self::SocketOnly => "socket-only",
            Self::HybridProjection => "hybrid",
        }
    }
}

fn main() {
    #[cfg(not(target_os = "macos"))]
    {
        println!(
            "seyal-runtime scalability: PLATFORM_LIMITED target_os!=macos; no performance claim"
        );
    }

    #[cfg(target_os = "macos")]
    run_macos();
}

#[cfg(target_os = "macos")]
fn run_macos() {
    if env::args().nth(1).as_deref() == Some("--worker") {
        worker();
        return;
    }

    println!("seyal-runtime Pass 5 equivalent display-transport comparator; performance_claim=false");
    println!(
        "method=fresh_worker_per_population_and_transport socket_only=full_fixed_width_snapshot_over_unix_stream hybrid=production_uds_plus_readonly_shm"
    );
    println!(
        "semantic_rule=delivered_snapshot_must_equal_same_worker_canonical_projection visible_attachment_cap={MAX_VISIBLE_ATTACHMENTS}"
    );
    print_host_metadata();

    let executable = env::current_exe().expect("current benchmark executable");
    for transport in [TransportMode::SocketOnly, TransportMode::HybridProjection] {
        for population in [1usize, 10, 50, 100] {
            run_worker_process(&executable, population, 80, 24, "primary", transport);
        }
    }

    for transport in [TransportMode::SocketOnly, TransportMode::HybridProjection] {
        for (columns, rows, screen) in [
            (120, 40, "primary"),
            (200, 60, "primary"),
            (80, 24, "alternate"),
        ] {
            run_worker_process(&executable, 1, columns, rows, screen, transport);
        }
    }
}

#[cfg(target_os = "macos")]
fn run_worker_process(
    executable: &std::path::Path,
    population: usize,
    columns: u16,
    rows: u16,
    screen: &str,
    transport: TransportMode,
) {
    let status = Command::new(executable)
        .args([
            "--worker",
            &population.to_string(),
            &columns.to_string(),
            &rows.to_string(),
            screen,
            transport.as_arg(),
        ])
        .status()
        .expect("launch fresh benchmark worker");
    assert!(status.success(), "runtime benchmark worker failed");
}

#[cfg(target_os = "macos")]
fn worker() {
    let args = env::args().collect::<Vec<_>>();
    let requested = args[2].parse::<usize>().expect("population");
    let columns = args[3].parse::<u16>().expect("columns");
    let rows = args[4].parse::<u16>().expect("rows");
    let alternate = args[5] == "alternate";
    let transport = TransportMode::from_arg(&args[6]);

    let mut config = RuntimeConfig::m001().expect("bundled capability policy");
    config.singleton_path = env::temp_dir().join(format!(
        "s5b-{}-{requested}-{columns}x{rows}.lock",
        process::id()
    ));
    let local_ipc_runtime_dir = match transport {
        TransportMode::SocketOnly => None,
        TransportMode::HybridProjection => {
            Some(env::temp_dir().join(format!("s5bd-{}-{requested}", process::id())))
        }
    };
    config.local_ipc = match &local_ipc_runtime_dir {
        None => LocalIpcMode::Disabled,
        Some(path) => LocalIpcMode::Enabled {
            runtime_dir_override: Some(path.clone()),
        },
    };
    config.max_executions = requested.max(1);
    let mut runtime = Runtime::new(config).expect("headless Runtime");
    let baseline = process_metrics(&runtime);

    let creation_start = Instant::now();
    let mut ids = Vec::with_capacity(requested);
    let mut platform_limit = None;
    for index in 0..requested {
        let command = if alternate && index == 0 {
            CommandSpec::new("/bin/sh").args(["-c", "printf '\x1b[?1049hALT'; exec /bin/cat"])
        } else {
            CommandSpec::new("/bin/cat")
        };
        match runtime.create_execution(
            command,
            WindowSize::new(columns, rows, 0, 0).expect("valid geometry"),
        ) {
            Ok(id) => ids.push(id),
            Err(error) => {
                platform_limit = Some(error.to_string());
                break;
            }
        }
    }
    let creation_us = creation_start.elapsed().as_micros();
    settle_runtime(&mut runtime);

    let visible_count = ids.len().min(MAX_VISIBLE_ATTACHMENTS);
    let setup_start = Instant::now();
    let mut display = match transport {
        TransportMode::SocketOnly => {
            DisplayClients::Socket(setup_socket_clients(&runtime, &ids[..visible_count]))
        }
        TransportMode::HybridProjection => DisplayClients::Hybrid(setup_hybrid_clients(
            &mut runtime,
            &ids[..visible_count],
        )),
    };
    let display_setup_us = setup_start.elapsed().as_micros();
    let initial_semantic_match = display.semantic_match(&runtime, &ids[..visible_count]);
    assert!(
        initial_semantic_match,
        "initial delivered display state diverged from canonical projection"
    );

    thread::sleep(Duration::from_millis(100));
    let populated = process_metrics(&runtime);

    let registry_start = Instant::now();
    for _ in 0..100 {
        let summaries = runtime.list();
        for summary in &summaries {
            std::hint::black_box(runtime.lookup(summary.id));
        }
    }
    let registry_us = registry_start.elapsed().as_micros();

    let progress = ids.first().and_then(|id| match &mut display {
        DisplayClients::Socket(clients) => clients
            .first_mut()
            .and_then(|client| measure_socket_progress(&mut runtime, *id, client)),
        DisplayClients::Hybrid(clients) => clients
            .first_mut()
            .and_then(|client| measure_hybrid_progress(&mut runtime, *id, client)),
    });
    if let Some(progress) = &progress {
        assert!(progress.semantic_match, "updated display state was not canonical");
    }

    let resync = ids.first().and_then(|id| match &mut display {
        DisplayClients::Socket(clients) => clients
            .first_mut()
            .map(|client| measure_socket_resync(&runtime, *id, client)),
        DisplayClients::Hybrid(clients) => clients
            .first_mut()
            .map(|client| measure_hybrid_resync(&mut runtime, client)),
    });
    if let Some(result) = &resync {
        assert!(result.semantic_match, "resync display state was not canonical");
    }

    let reconnect = ids.first().map(|id| match &mut display {
        DisplayClients::Socket(clients) => measure_socket_reconnect(&runtime, *id, clients),
        DisplayClients::Hybrid(clients) => measure_hybrid_reconnect(&mut runtime, *id, clients),
    });
    if let Some(result) = &reconnect {
        assert!(
            result.semantic_match,
            "reconnected display state was not canonical"
        );
    }

    let display_bytes = display.display_bytes();
    let region_bytes = display.region_bytes();
    drop(display);
    settle_runtime(&mut runtime);

    let teardown_start = Instant::now();
    runtime.begin_shutdown().expect("begin shutdown");
    let shutdown = runtime.run_until_empty(Instant::now() + Duration::from_secs(8));
    let teardown_us = teardown_start.elapsed().as_micros();
    let final_metrics = process_metrics(&runtime);

    let classification = if ids.len() == requested {
        "MEASURED"
    } else {
        "PLATFORM_LIMITED"
    };
    println!(
        "runtime_resource transport={} population_requested={requested} population_created={} visible_attached={visible_count} geometry={}x{} screen={} classification={classification} create_us={creation_us} display_setup_us={display_setup_us} registry_100x_us={registry_us} update_to_readable_us={:?} signal_to_readable_us={:?} update_transfer_bytes={:?} update_write_calls={:?} resync_us={:?} reconnect_us={:?} semantic_match={} display_bytes={} projection_region_bytes={} rss_baseline_kib={} rss_populated_kib={} rss_final_kib={} incremental_runtime_kib={} child_rss_kib={} idle_cpu_percent={} threads_baseline={} threads_populated={} threads_final={} fd_baseline={} fd_populated={} fd_final={} teardown_us={teardown_us} pending_final={} shutdown_ok={} platform_error={:?}",
        transport.as_arg(),
        ids.len(),
        columns,
        rows,
        if alternate { "alternate" } else { "primary" },
        progress.as_ref().map(|value| value.total_us),
        progress.as_ref().map(|value| value.signal_to_readable_us),
        progress.as_ref().map(|value| value.transfer_bytes),
        progress.as_ref().map(|value| value.write_calls),
        resync.as_ref().map(|value| value.total_us),
        reconnect.as_ref().map(|value| value.total_us),
        initial_semantic_match
            && progress.as_ref().is_none_or(|value| value.semantic_match)
            && resync.as_ref().is_none_or(|value| value.semantic_match)
            && reconnect.as_ref().is_none_or(|value| value.semantic_match),
        display_bytes,
        region_bytes,
        baseline.rss_kib,
        populated.rss_kib,
        final_metrics.rss_kib,
        populated.rss_kib.saturating_sub(baseline.rss_kib),
        populated.child_rss_kib,
        populated.cpu_percent,
        baseline.threads,
        populated.threads,
        final_metrics.threads,
        baseline.fds,
        populated.fds,
        final_metrics.fds,
        runtime.aggregate_accepted_but_unwritten_bytes(),
        shutdown.is_ok(),
        platform_limit,
    );
    shutdown.expect("controlled Runtime teardown");
    drop(runtime);
    if let Some(runtime_dir) = local_ipc_runtime_dir {
        let _ = fs::remove_dir_all(runtime_dir);
    }
}

#[cfg(target_os = "macos")]
fn settle_runtime(runtime: &mut Runtime) {
    for _ in 0..12 {
        let _ = runtime.poll_once(Some(Duration::from_millis(2)));
    }
}

#[cfg(target_os = "macos")]
struct ProgressResult {
    total_us: u128,
    signal_to_readable_us: u128,
    transfer_bytes: usize,
    write_calls: usize,
    semantic_match: bool,
}

#[cfg(target_os = "macos")]
struct RoundTripResult {
    total_us: u128,
    semantic_match: bool,
}

#[cfg(target_os = "macos")]
enum DisplayClients {
    Socket(Vec<SocketClient>),
    Hybrid(Vec<HybridClient>),
}

#[cfg(target_os = "macos")]
impl DisplayClients {
    fn semantic_match(&self, runtime: &Runtime, ids: &[ExecutionId]) -> bool {
        match self {
            Self::Socket(clients) => clients.iter().zip(ids).all(|(client, id)| {
                let expected = canonical_snapshot(runtime, *id);
                owned_snapshot_matches(&client.last_snapshot, &expected)
            }),
            Self::Hybrid(clients) => clients.iter().zip(ids).all(|(client, id)| {
                let expected = canonical_snapshot(runtime, *id);
                read_latest(&client.mapping.memory(), &client.region)
                    .is_ok_and(|read| snapshot_read_matches(&read, &expected))
            }),
        }
    }

    fn display_bytes(&self) -> usize {
        match self {
            Self::Socket(clients) => clients.iter().map(|client| client.last_frame_bytes).sum(),
            Self::Hybrid(clients) => clients
                .iter()
                .map(|client| projection_payload_bytes(&canonical_shape(client)))
                .sum(),
        }
    }

    fn region_bytes(&self) -> usize {
        match self {
            Self::Socket(_) => 0,
            Self::Hybrid(clients) => clients.iter().map(|client| client.region_bytes).sum(),
        }
    }
}

#[cfg(target_os = "macos")]
fn canonical_shape(client: &HybridClient) -> OwnedSnapshot {
    let read = read_latest(&client.mapping.memory(), &client.region).expect("read hybrid snapshot");
    OwnedSnapshot {
        rows: read.header.rows,
        columns: read.header.columns,
        cursor_row: read.header.cursor_row,
        cursor_col: read.header.cursor_col,
        cursor_visible: read.header.cursor_visible,
        mode_flags: read.header.mode_flags,
        cells: read.cells,
        damages: read.damages,
        full_snapshot: read.header.full_snapshot,
        source_damage_generation: read.header.source_damage_generation,
    }
}

#[cfg(target_os = "macos")]
struct SocketClient {
    tx: UnixStream,
    rx: UnixStream,
    last_snapshot: OwnedSnapshot,
    last_frame_bytes: usize,
}

#[cfg(target_os = "macos")]
fn setup_socket_clients(runtime: &Runtime, ids: &[ExecutionId]) -> Vec<SocketClient> {
    ids.iter()
        .map(|id| {
            let (mut tx, mut rx) = UnixStream::pair().expect("reference socket pair");
            tx.set_nonblocking(true).expect("nonblocking reference tx");
            rx.set_nonblocking(true).expect("nonblocking reference rx");
            let expected = canonical_snapshot(runtime, *id);
            let frame = encode_reference_snapshot(&expected);
            let transfer = transfer_reference_frame(&mut tx, &mut rx, &frame);
            let decoded = decode_reference_snapshot(&transfer.bytes);
            assert!(owned_snapshot_matches(&decoded, &expected));
            SocketClient {
                tx,
                rx,
                last_snapshot: decoded,
                last_frame_bytes: frame.len(),
            }
        })
        .collect()
}

#[cfg(target_os = "macos")]
struct HybridClient {
    stream: UnixStream,
    attached: Attached,
    mapping: ReadOnlyMapping,
    region: RegionHeader,
    region_bytes: usize,
}

#[cfg(target_os = "macos")]
fn setup_hybrid_clients(runtime: &mut Runtime, ids: &[ExecutionId]) -> Vec<HybridClient> {
    ids.iter()
        .map(|id| connect_hybrid_client(runtime, *id))
        .collect()
}

#[cfg(target_os = "macos")]
fn connect_hybrid_client(runtime: &mut Runtime, execution_id: ExecutionId) -> HybridClient {
    let path = runtime
        .local_ipc_socket_path()
        .expect("hybrid runtime socket")
        .to_path_buf();
    let mut stream = UnixStream::connect(path).expect("connect hybrid client");
    stream
        .set_nonblocking(true)
        .expect("nonblocking hybrid client");

    send_hybrid_frame(
        runtime,
        &mut stream,
        MessageType::ClientHello,
        &ClientHello {
            client_capabilities: 0,
        }
        .encode(),
    );
    let (message_type, payload) = wait_plain_frame(runtime, &mut stream);
    assert_eq!(message_type, MessageType::ServerHello as u16);
    ServerHello::decode(&payload).expect("valid benchmark ServerHello");

    send_hybrid_frame(
        runtime,
        &mut stream,
        MessageType::Attach,
        &Attach {
            execution_id,
            requested_role: Role::Observer,
        }
        .encode(),
    );
    let (message_type, payload, fd) = wait_fd_frame(runtime, &stream);
    assert_eq!(message_type, MessageType::Attached as u16);
    let attached = Attached::decode(&payload).expect("valid benchmark Attached");
    let region_bytes = attached.region_bytes as usize;
    let mapping = ReadOnlyMapping::new(fd, region_bytes).expect("map benchmark projection");
    let region = read_region_header(&mapping.memory()).expect("benchmark region header");
    assert_eq!(region.execution_id, u128::from_le_bytes(execution_id.to_bytes()));
    assert_eq!(region.region_bytes as usize, region_bytes);
    HybridClient {
        stream,
        attached,
        mapping,
        region,
        region_bytes,
    }
}

#[cfg(target_os = "macos")]
fn measure_socket_progress(
    runtime: &mut Runtime,
    execution_id: ExecutionId,
    client: &mut SocketClient,
) -> Option<ProgressResult> {
    let ingress = runtime.input_ingress(execution_id).ok()?;
    let before_generation = runtime.execution(execution_id)?.terminal().damage_generation();
    let start = Instant::now();
    ingress.try_submit(b"z".to_vec()).ok()?;
    let deadline = Instant::now() + Duration::from_secs(2);
    while runtime.execution(execution_id)?.terminal().damage_generation() <= before_generation {
        runtime.poll_once(Some(Duration::from_millis(2))).ok()?;
        if Instant::now() >= deadline {
            return None;
        }
    }
    let expected = canonical_snapshot(runtime, execution_id);
    let frame = encode_reference_snapshot(&expected);
    let transfer = transfer_reference_frame(&mut client.tx, &mut client.rx, &frame);
    let decoded = decode_reference_snapshot(&transfer.bytes);
    let semantic_match = owned_snapshot_matches(&decoded, &expected);
    client.last_frame_bytes = frame.len();
    client.last_snapshot = decoded;
    Some(ProgressResult {
        total_us: start.elapsed().as_micros(),
        signal_to_readable_us: transfer.first_read_to_complete_us,
        transfer_bytes: transfer.bytes.len(),
        write_calls: transfer.write_calls,
        semantic_match,
    })
}

#[cfg(target_os = "macos")]
fn measure_hybrid_progress(
    runtime: &mut Runtime,
    execution_id: ExecutionId,
    client: &mut HybridClient,
) -> Option<ProgressResult> {
    let ingress = runtime.input_ingress(execution_id).ok()?;
    let before = read_latest(&client.mapping.memory(), &client.region).ok()?;
    let start = Instant::now();
    ingress.try_submit(b"z".to_vec()).ok()?;
    let (wake, wake_observed) = wait_generation_wake(
        runtime,
        &mut client.stream,
        client.attached.attachment_id,
        before.generation + 1,
    )?;
    let read = wait_projection_generation(runtime, client, wake.committed_generation)?;
    let signal_to_readable_us = wake_observed.elapsed().as_micros();
    let expected = canonical_snapshot(runtime, execution_id);
    let transfer_bytes = projection_payload_bytes(&expected);
    Some(ProgressResult {
        total_us: start.elapsed().as_micros(),
        signal_to_readable_us,
        transfer_bytes,
        write_calls: 1,
        semantic_match: snapshot_read_matches(&read, &expected),
    })
}

#[cfg(target_os = "macos")]
fn measure_socket_resync(
    runtime: &Runtime,
    execution_id: ExecutionId,
    client: &mut SocketClient,
) -> RoundTripResult {
    let expected = canonical_snapshot(runtime, execution_id);
    let frame = encode_reference_snapshot(&expected);
    let start = Instant::now();
    let transfer = transfer_reference_frame(&mut client.tx, &mut client.rx, &frame);
    let decoded = decode_reference_snapshot(&transfer.bytes);
    let semantic_match = owned_snapshot_matches(&decoded, &expected);
    client.last_frame_bytes = frame.len();
    client.last_snapshot = decoded;
    RoundTripResult {
        total_us: start.elapsed().as_micros(),
        semantic_match,
    }
}

#[cfg(target_os = "macos")]
fn measure_hybrid_resync(runtime: &mut Runtime, client: &mut HybridClient) -> RoundTripResult {
    let start = Instant::now();
    send_hybrid_frame(
        runtime,
        &mut client.stream,
        MessageType::Resync,
        &Resync {
            attachment_id: client.attached.attachment_id,
        }
        .encode(),
    );
    let (wake, _) = wait_generation_wake(
        runtime,
        &mut client.stream,
        client.attached.attachment_id,
        1,
    )
    .expect("hybrid resync wake");
    let read = wait_projection_generation(runtime, client, wake.committed_generation)
        .expect("hybrid resync projection");
    let expected = canonical_snapshot(runtime, client.attached.execution_id);
    RoundTripResult {
        total_us: start.elapsed().as_micros(),
        semantic_match: snapshot_read_matches(&read, &expected),
    }
}

#[cfg(target_os = "macos")]
fn measure_socket_reconnect(
    runtime: &Runtime,
    execution_id: ExecutionId,
    clients: &mut Vec<SocketClient>,
) -> RoundTripResult {
    if !clients.is_empty() {
        clients.remove(0);
    }
    let start = Instant::now();
    let mut replacement = setup_socket_clients(runtime, &[execution_id])
        .pop()
        .expect("socket reconnect client");
    let expected = canonical_snapshot(runtime, execution_id);
    let semantic_match = owned_snapshot_matches(&replacement.last_snapshot, &expected);
    replacement.last_frame_bytes = encode_reference_snapshot(&expected).len();
    clients.insert(0, replacement);
    RoundTripResult {
        total_us: start.elapsed().as_micros(),
        semantic_match,
    }
}

#[cfg(target_os = "macos")]
fn measure_hybrid_reconnect(
    runtime: &mut Runtime,
    execution_id: ExecutionId,
    clients: &mut Vec<HybridClient>,
) -> RoundTripResult {
    if !clients.is_empty() {
        clients.remove(0);
        settle_runtime(runtime);
    }
    let start = Instant::now();
    let replacement = connect_hybrid_client(runtime, execution_id);
    let expected = canonical_snapshot(runtime, execution_id);
    let read = read_latest(&replacement.mapping.memory(), &replacement.region)
        .expect("reconnected hybrid snapshot");
    let semantic_match = snapshot_read_matches(&read, &expected);
    clients.insert(0, replacement);
    RoundTripResult {
        total_us: start.elapsed().as_micros(),
        semantic_match,
    }
}

#[cfg(target_os = "macos")]
fn send_hybrid_frame(
    runtime: &mut Runtime,
    stream: &mut UnixStream,
    message_type: MessageType,
    payload: &[u8],
) {
    let frame = encode_frame(message_type, payload);
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut sent = 0usize;
    while sent < frame.len() {
        match stream.write(&frame[sent..]) {
            Ok(0) => panic!("hybrid client socket closed while writing"),
            Ok(count) => sent += count,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                runtime
                    .poll_once(Some(Duration::from_millis(2)))
                    .expect("pump hybrid write");
            }
            Err(error) => panic!("hybrid client write failed: {error}"),
        }
        assert!(Instant::now() < deadline, "hybrid client write timed out");
    }
    runtime
        .poll_once(Some(Duration::from_millis(2)))
        .expect("pump sent hybrid frame");
}

#[cfg(target_os = "macos")]
fn wait_plain_frame(runtime: &mut Runtime, stream: &mut UnixStream) -> (u16, Vec<u8>) {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut buffer = Vec::new();
    loop {
        let mut chunk = [0u8; 4096];
        match stream.read(&mut chunk) {
            Ok(0) => panic!("hybrid connection closed while awaiting frame"),
            Ok(count) => buffer.extend_from_slice(&chunk[..count]),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("hybrid client read failed: {error}"),
        }
        if let Some(frame) = complete_frame(&buffer) {
            return frame;
        }
        assert!(Instant::now() < deadline, "hybrid frame timed out");
        runtime
            .poll_once(Some(Duration::from_millis(2)))
            .expect("pump hybrid frame wait");
    }
}

#[cfg(target_os = "macos")]
fn wait_fd_frame(runtime: &mut Runtime, stream: &UnixStream) -> (u16, Vec<u8>, OwnedFd) {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut buffer = Vec::new();
    let mut captured_fd = None;
    loop {
        if let Some((message_type, payload)) = complete_frame(&buffer) {
            return (
                message_type,
                payload,
                captured_fd.expect("fd-bearing frame completed without descriptor"),
            );
        }
        let mut chunk = [0u8; 4096];
        match fd_transfer::recv_with_fd(stream.as_raw_fd(), &mut chunk) {
            Ok((0, _)) => panic!("hybrid connection closed while awaiting fd frame"),
            Ok((count, RecvFd::One(fd))) => {
                assert!(
                    captured_fd.replace(fd).is_none(),
                    "hybrid frame carried multiple descriptors"
                );
                buffer.extend_from_slice(&chunk[..count]);
            }
            Ok((count, RecvFd::None)) => buffer.extend_from_slice(&chunk[..count]),
            Ok((_, RecvFd::Malformed)) => panic!("hybrid descriptor transfer was malformed"),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("hybrid recvmsg failed: {error}"),
        }
        assert!(Instant::now() < deadline, "hybrid fd frame timed out");
        runtime
            .poll_once(Some(Duration::from_millis(2)))
            .expect("pump hybrid fd frame wait");
    }
}

#[cfg(target_os = "macos")]
fn complete_frame(buffer: &[u8]) -> Option<(u16, Vec<u8>)> {
    if buffer.len() < HEADER_LEN {
        return None;
    }
    let header = FrameHeader::decode(&buffer[..HEADER_LEN]).expect("benchmark wire header");
    let total = HEADER_LEN + header.payload_len as usize;
    (buffer.len() >= total).then(|| (header.message_type, buffer[HEADER_LEN..total].to_vec()))
}

#[cfg(target_os = "macos")]
fn wait_generation_wake(
    runtime: &mut Runtime,
    stream: &mut UnixStream,
    attachment_id: seyal_runtime::AttachmentId,
    minimum_generation: u64,
) -> Option<(GenerationWake, Instant)> {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut buffer = Vec::new();
    loop {
        let mut chunk = [0u8; 4096];
        match stream.read(&mut chunk) {
            Ok(0) => return None,
            Ok(count) => buffer.extend_from_slice(&chunk[..count]),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(_) => return None,
        }
        if buffer.len() >= HEADER_LEN {
            let header = FrameHeader::decode(&buffer[..HEADER_LEN]).ok()?;
            let total = HEADER_LEN + header.payload_len as usize;
            if buffer.len() >= total {
                let payload = &buffer[HEADER_LEN..total];
                buffer.drain(..total);
                if header.message_type == MessageType::GenerationWake as u16 {
                    let wake = GenerationWake::decode(payload).ok()?;
                    if wake.attachment_id == attachment_id
                        && wake.committed_generation >= minimum_generation
                    {
                        return Some((wake, Instant::now()));
                    }
                }
            }
        }
        if Instant::now() >= deadline {
            return None;
        }
        runtime.poll_once(Some(Duration::from_millis(2))).ok()?;
    }
}

#[cfg(target_os = "macos")]
fn wait_projection_generation(
    runtime: &mut Runtime,
    client: &HybridClient,
    generation: u64,
) -> Option<SnapshotRead> {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Ok(read) = read_latest(&client.mapping.memory(), &client.region)
            && read.generation >= generation
        {
            return Some(read);
        }
        if Instant::now() >= deadline {
            return None;
        }
        runtime.poll_once(Some(Duration::from_millis(2))).ok()?;
    }
}

#[cfg(target_os = "macos")]
fn canonical_snapshot(runtime: &Runtime, execution_id: ExecutionId) -> OwnedSnapshot {
    producer::from_execution(
        runtime
            .execution(execution_id)
            .expect("benchmark execution")
            .projection_snapshot(),
    )
}

#[cfg(target_os = "macos")]
fn projection_payload_bytes(snapshot: &OwnedSnapshot) -> usize {
    64 + snapshot.cells.len() * CELL_LEN + snapshot.damages.len() * DAMAGE_LEN
}

#[cfg(target_os = "macos")]
fn snapshot_read_matches(read: &SnapshotRead, expected: &OwnedSnapshot) -> bool {
    read.header.rows == expected.rows
        && read.header.columns == expected.columns
        && read.header.cursor_row == expected.cursor_row
        && read.header.cursor_col == expected.cursor_col
        && read.header.cursor_visible == expected.cursor_visible
        && read.header.mode_flags == expected.mode_flags
        && read.header.full_snapshot == expected.full_snapshot
        && read.header.source_damage_generation == expected.source_damage_generation
        && read.cells == expected.cells
        && read.damages == expected.damages
}

#[cfg(target_os = "macos")]
fn owned_snapshot_matches(actual: &OwnedSnapshot, expected: &OwnedSnapshot) -> bool {
    actual.rows == expected.rows
        && actual.columns == expected.columns
        && actual.cursor_row == expected.cursor_row
        && actual.cursor_col == expected.cursor_col
        && actual.cursor_visible == expected.cursor_visible
        && actual.mode_flags == expected.mode_flags
        && actual.full_snapshot == expected.full_snapshot
        && actual.source_damage_generation == expected.source_damage_generation
        && actual.cells == expected.cells
        && actual.damages == expected.damages
}

#[cfg(target_os = "macos")]
fn encode_reference_snapshot(snapshot: &OwnedSnapshot) -> Vec<u8> {
    let cells_bytes = snapshot.cells.len() * CELL_LEN;
    let damages_bytes = snapshot.damages.len() * DAMAGE_LEN;
    let total = REFERENCE_HEADER_LEN + cells_bytes + damages_bytes;
    let mut bytes = vec![0u8; total];
    bytes[0..8].copy_from_slice(&REFERENCE_MAGIC);
    bytes[8..12].copy_from_slice(&(total as u32).to_le_bytes());
    bytes[12..14].copy_from_slice(&snapshot.rows.to_le_bytes());
    bytes[14..16].copy_from_slice(&snapshot.columns.to_le_bytes());
    bytes[16..18].copy_from_slice(&snapshot.cursor_row.to_le_bytes());
    bytes[18..20].copy_from_slice(&snapshot.cursor_col.to_le_bytes());
    bytes[20] = snapshot.cursor_visible as u8;
    bytes[21] = snapshot.mode_flags.alternate_screen as u8;
    bytes[22] = snapshot.full_snapshot as u8;
    bytes[24..28].copy_from_slice(&(snapshot.cells.len() as u32).to_le_bytes());
    bytes[28..30].copy_from_slice(&(snapshot.damages.len() as u16).to_le_bytes());
    bytes[32..40].copy_from_slice(&snapshot.source_damage_generation.to_le_bytes());
    let mut offset = REFERENCE_HEADER_LEN;
    for cell in &snapshot.cells {
        cell.encode(&mut bytes[offset..offset + CELL_LEN])
            .expect("encode reference cell");
        offset += CELL_LEN;
    }
    for damage in &snapshot.damages {
        damage
            .encode(&mut bytes[offset..offset + DAMAGE_LEN])
            .expect("encode reference damage");
        offset += DAMAGE_LEN;
    }
    bytes
}

#[cfg(target_os = "macos")]
fn decode_reference_snapshot(bytes: &[u8]) -> OwnedSnapshot {
    assert!(bytes.len() >= REFERENCE_HEADER_LEN);
    assert_eq!(bytes[0..8], REFERENCE_MAGIC);
    let total = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    assert_eq!(total, bytes.len());
    assert!(matches!(bytes[20], 0 | 1));
    assert!(matches!(bytes[21], 0 | 1));
    assert!(matches!(bytes[22], 0 | 1));
    assert_eq!(bytes[23], 0);
    assert_eq!(bytes[30..32], [0, 0]);
    let rows = u16::from_le_bytes(bytes[12..14].try_into().unwrap());
    let columns = u16::from_le_bytes(bytes[14..16].try_into().unwrap());
    let cursor_row = u16::from_le_bytes(bytes[16..18].try_into().unwrap());
    let cursor_col = u16::from_le_bytes(bytes[18..20].try_into().unwrap());
    let cell_count = u32::from_le_bytes(bytes[24..28].try_into().unwrap()) as usize;
    let damage_count = u16::from_le_bytes(bytes[28..30].try_into().unwrap()) as usize;
    let source_damage_generation = u64::from_le_bytes(bytes[32..40].try_into().unwrap());
    assert_eq!(cell_count, rows as usize * columns as usize);
    let expected_len = REFERENCE_HEADER_LEN + cell_count * CELL_LEN + damage_count * DAMAGE_LEN;
    assert_eq!(bytes.len(), expected_len);

    let cells_end = REFERENCE_HEADER_LEN + cell_count * CELL_LEN;
    let (cell_chunks, cell_remainder) =
        bytes[REFERENCE_HEADER_LEN..cells_end].as_chunks::<CELL_LEN>();
    assert!(cell_remainder.is_empty());
    let cells = cell_chunks
        .iter()
        .map(|chunk| CellRecord::decode(chunk).expect("decode reference cell"))
        .collect();
    let (damage_chunks, damage_remainder) = bytes[cells_end..].as_chunks::<DAMAGE_LEN>();
    assert!(damage_remainder.is_empty());
    let damages = damage_chunks
        .iter()
        .map(|chunk| DamageRecord::decode(chunk, rows).expect("decode reference damage"))
        .collect();
    let cursor_visible = bytes[20] != 0;
    OwnedSnapshot {
        rows,
        columns,
        cursor_row,
        cursor_col,
        cursor_visible,
        mode_flags: ModeFlags {
            alternate_screen: bytes[21] != 0,
            cursor_visible,
        },
        cells,
        damages,
        full_snapshot: bytes[22] != 0,
        source_damage_generation,
    }
}

#[cfg(target_os = "macos")]
struct ReferenceTransfer {
    bytes: Vec<u8>,
    write_calls: usize,
    first_read_to_complete_us: u128,
}

#[cfg(target_os = "macos")]
fn transfer_reference_frame(
    tx: &mut UnixStream,
    rx: &mut UnixStream,
    frame: &[u8],
) -> ReferenceTransfer {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut sent = 0usize;
    let mut received = Vec::with_capacity(frame.len());
    let mut write_calls = 0usize;
    let mut first_read = None;
    while received.len() < frame.len() {
        let mut progress = false;
        if sent < frame.len() {
            match tx.write(&frame[sent..]) {
                Ok(0) => panic!("reference socket closed while writing"),
                Ok(count) => {
                    sent += count;
                    write_calls += 1;
                    progress = true;
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(error) => panic!("reference socket write failed: {error}"),
            }
        }
        let mut chunk = [0u8; 16 * 1024];
        match rx.read(&mut chunk) {
            Ok(0) => panic!("reference socket closed while reading"),
            Ok(count) => {
                first_read.get_or_insert_with(Instant::now);
                received.extend_from_slice(&chunk[..count]);
                progress = true;
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("reference socket read failed: {error}"),
        }
        assert!(Instant::now() < deadline, "reference transfer timed out");
        if !progress {
            thread::yield_now();
        }
    }
    assert_eq!(sent, frame.len());
    let first_read_to_complete_us = first_read
        .map(|instant| instant.elapsed().as_micros())
        .unwrap_or(0);
    ReferenceTransfer {
        bytes: received,
        write_calls,
        first_read_to_complete_us,
    }
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy)]
struct Metrics {
    rss_kib: usize,
    child_rss_kib: usize,
    cpu_percent: f32,
    threads: usize,
    fds: usize,
}

#[cfg(target_os = "macos")]
fn process_metrics(runtime: &Runtime) -> Metrics {
    let pid = process::id();
    let output = Command::new("/bin/ps")
        .args(["-o", "rss=,%cpu=,thcount=", "-p", &pid.to_string()])
        .output()
        .expect("ps Runtime metrics");
    let line = String::from_utf8_lossy(&output.stdout);
    let mut fields = line.split_whitespace();
    let rss_kib = fields.next().and_then(|v| v.parse().ok()).unwrap_or(0);
    let cpu_percent = fields.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let threads = fields.next().and_then(|v| v.parse().ok()).unwrap_or(0);
    let child_rss_kib = runtime
        .list()
        .iter()
        .filter_map(|summary| runtime.execution(summary.id))
        .map(|execution| rss_for_pid(execution.child_id()))
        .sum();
    let fds = fs::read_dir("/dev/fd")
        .map(|entries| entries.count())
        .unwrap_or(0);
    Metrics {
        rss_kib,
        child_rss_kib,
        cpu_percent,
        threads,
        fds,
    }
}

#[cfg(target_os = "macos")]
fn rss_for_pid(pid: u32) -> usize {
    let output = Command::new("/bin/ps")
        .args(["-o", "rss=", "-p", &pid.to_string()])
        .output();
    output
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(0)
}

#[cfg(target_os = "macos")]
fn print_host_metadata() {
    let product = command_text("/usr/bin/sw_vers", &["-productVersion"]);
    let build = command_text("/usr/bin/sw_vers", &["-buildVersion"]);
    let hardware = command_text("/usr/sbin/sysctl", &["-n", "machdep.cpu.brand_string"]);
    let pty_max = command_text("/usr/sbin/sysctl", &["-n", "kern.tty.ptmx_max"]);
    let rustc = command_text("rustc", &["--version"]);
    println!(
        "host macos_version={product} macos_build={build} hardware={hardware:?} rust={rustc:?} pty_max={pty_max}"
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
