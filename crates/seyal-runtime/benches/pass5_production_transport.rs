#[cfg(target_os = "macos")]
use stats_alloc::Region;
use stats_alloc::{INSTRUMENTED_SYSTEM, StatsAlloc};
use std::alloc::System;

#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

#[cfg(target_os = "macos")]
use std::{
    env, fs,
    io::{Read, Write},
    os::unix::net::UnixStream,
    process::{self, Command},
    time::{Duration, Instant},
};

#[cfg(target_os = "macos")]
use seyal_exec::{CommandSpec, WindowSize};
#[cfg(target_os = "macos")]
use seyal_runtime::{
    ExecutionId, LocalIpcMode, Runtime, RuntimeConfig,
    display::{DecodedDisplayChunk, DisplayCache, decode_chunk, empty_cache},
    local_ipc::{
        fd_transfer::{benchmark_syscall_counters, reset_benchmark_syscall_counters},
        framing::{
            Attach, Attached, ClientHello, FrameHeader, HEADER_LEN, InputRef, MessageType, Role,
            ServerHello, encode_frame,
        },
    },
};

fn main() {
    #[cfg(not(target_os = "macos"))]
    println!(
        "pass5_production_transport PLATFORM_LIMITED target_os!=macos performance_claim=false"
    );

    #[cfg(target_os = "macos")]
    run_macos();
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Workload {
    Interactive,
    Token,
    Burst,
    Sustained,
    Tui,
    Alternate,
}

#[cfg(target_os = "macos")]
impl Workload {
    fn name(self) -> &'static str {
        match self {
            Self::Interactive => "interactive",
            Self::Token => "token_stream",
            Self::Burst => "burst_scroll",
            Self::Sustained => "sustained_high_output_2s",
            Self::Tui => "tui_full_redraw",
            Self::Alternate => "alternate_screen",
        }
    }

    fn parse(value: &str) -> Self {
        match value {
            "interactive" => Self::Interactive,
            "token_stream" => Self::Token,
            "burst_scroll" => Self::Burst,
            "sustained_high_output_2s" => Self::Sustained,
            "tui_full_redraw" => Self::Tui,
            "alternate_screen" => Self::Alternate,
            _ => panic!("unknown workload {value}"),
        }
    }
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy)]
struct Case {
    workload: Workload,
    population: usize,
    columns: u16,
    rows: u16,
    fanout: usize,
}

#[cfg(target_os = "macos")]
fn run_macos() {
    if env::args().nth(1).as_deref() == Some("--worker") {
        worker();
        return;
    }

    println!(
        "pass5_production_transport architecture=Candidate-D performance_claim=false path=real_child->PTY->Seyal_VT->canonical_damage->damage_sized_model_update->production_UDS->client_DisplayCache"
    );
    println!(
        "measurement_labels=MEASURED|PLATFORM_LIMITED percentile_method=nearest_rank repetitions_interactive=32 syscall_counts=feature_gated_exact_invocations queue_depth=bounded_by_production_contract"
    );
    print_host_metadata();

    let executable = env::current_exe().expect("benchmark executable");
    let mut cases = Vec::new();

    for fanout in [1usize, 2, 4, 8, 16] {
        cases.push(Case {
            workload: Workload::Interactive,
            population: 1,
            columns: 80,
            rows: 24,
            fanout,
        });
    }
    for fanout in [1usize, 2, 4, 8, 16] {
        cases.push(Case {
            workload: Workload::Sustained,
            population: 1,
            columns: 200,
            rows: 60,
            fanout,
        });
    }
    for population in [10usize, 50, 100] {
        cases.push(Case {
            workload: Workload::Interactive,
            population,
            columns: 80,
            rows: 24,
            fanout: 1,
        });
    }
    for (columns, rows) in [(120u16, 40u16), (200, 60), (512, 256)] {
        cases.push(Case {
            workload: Workload::Interactive,
            population: 1,
            columns,
            rows,
            fanout: 1,
        });
    }
    for workload in [
        Workload::Token,
        Workload::Burst,
        Workload::Tui,
        Workload::Alternate,
    ] {
        cases.push(Case {
            workload,
            population: 1,
            columns: 120,
            rows: 40,
            fanout: 16,
        });
    }

    for case in cases {
        run_worker(&executable, case);
    }
}

#[cfg(target_os = "macos")]
fn run_worker(executable: &std::path::Path, case: Case) {
    let args = [
        "--worker".to_owned(),
        case.workload.name().to_owned(),
        case.population.to_string(),
        case.columns.to_string(),
        case.rows.to_string(),
        case.fanout.to_string(),
    ];
    let status = Command::new("/usr/bin/time")
        .arg("-l")
        .arg(executable)
        .args(args)
        .status()
        .expect("launch measured Pass-5 worker");
    assert!(
        status.success(),
        "Pass-5 production benchmark worker failed"
    );
}

#[cfg(target_os = "macos")]
fn worker() {
    let args = env::args().collect::<Vec<_>>();
    let workload = Workload::parse(&args[2]);
    let requested_population = args[3].parse::<usize>().expect("population");
    let columns = args[4].parse::<u16>().expect("columns");
    let rows = args[5].parse::<u16>().expect("rows");
    let requested_fanout = args[6].parse::<usize>().expect("fanout");

    let suffix = format!(
        "{}-{}-{}-{}x{}-{}",
        process::id(),
        workload.name(),
        requested_population,
        columns,
        rows,
        requested_fanout
    );
    let mut config = RuntimeConfig::m001().expect("M001 config");
    config.singleton_path = env::temp_dir().join(format!("s5p-{suffix}.lock"));
    let runtime_dir = env::temp_dir().join(format!("s5pd-{suffix}"));
    config.local_ipc = LocalIpcMode::Enabled {
        runtime_dir_override: Some(runtime_dir.clone()),
    };
    config.max_executions = requested_population.max(1);
    config.graceful_termination = Duration::from_millis(100);
    config.forced_reap = Duration::from_millis(500);
    config.final_drain = Duration::from_millis(150);

    let baseline = process_metrics();
    let mut runtime = Runtime::new(config).expect("Runtime");
    let socket_path = runtime
        .local_ipc_socket_path()
        .expect("production local IPC")
        .to_path_buf();

    let create_start = Instant::now();
    let mut ids = Vec::with_capacity(requested_population);
    let mut platform_error = None;
    for index in 0..requested_population {
        let command = if index == 0 {
            workload_command(workload)
        } else {
            CommandSpec::new("/bin/cat")
        };
        match runtime.create_execution(
            command,
            WindowSize::new(columns, rows, 0, 0).expect("valid geometry"),
        ) {
            Ok(id) => ids.push(id),
            Err(error) => {
                platform_error = Some(error.to_string());
                break;
            }
        }
    }
    let create_us = create_start.elapsed().as_micros();
    let Some(&execution_id) = ids.first() else {
        println!(
            "pass5_production_result workload={} population_requested={} population_created=0 fanout_requested={} geometry={}x{} classification=PLATFORM_LIMITED platform_error={:?}",
            workload.name(),
            requested_population,
            requested_fanout,
            columns,
            rows,
            platform_error
        );
        return;
    };

    let fanout = requested_fanout.min(16);
    let setup_start = Instant::now();
    let mut clients = Vec::with_capacity(fanout);
    for index in 0..fanout {
        let role = if index == 0 {
            Role::Controller
        } else {
            Role::Observer
        };
        clients.push(BenchClient::connect_and_attach(
            &mut runtime,
            &socket_path,
            execution_id,
            role,
        ));
    }
    let setup_us = setup_start.elapsed().as_micros();
    let controller_attachment = clients[0].attachment_id;
    for client in &mut clients {
        client.reset_measurement_counters();
    }

    reset_benchmark_syscall_counters();
    let allocation_region = Region::new(GLOBAL);
    let measurement = if workload == Workload::Interactive {
        measure_interactive(
            &mut runtime,
            &mut clients,
            controller_attachment,
            execution_id,
        )
    } else {
        measure_streaming(&mut runtime, &mut clients, controller_attachment, workload)
    };
    let allocation_stats = allocation_region.change();
    let syscall_counters = benchmark_syscall_counters();
    assert!(clients.iter().all(|client| client.cache.generation > 0));

    let reconnect_start = Instant::now();
    let mut reconnect =
        BenchClient::connect_and_attach(&mut runtime, &socket_path, execution_id, Role::Observer);
    let reconnect_us = reconnect_start.elapsed().as_micros();
    let reconnect_generation = reconnect.cache.generation;
    let current_generation = clients
        .iter()
        .map(|client| client.cache.generation)
        .max()
        .unwrap_or(0);
    assert!(reconnect_generation >= current_generation);
    reconnect.drain_available().expect("reconnect drain");
    drop(reconnect);

    let populated = process_metrics();
    let bytes_received = clients
        .iter()
        .map(|client| client.bytes_received)
        .sum::<usize>();
    let display_batches = clients
        .iter()
        .map(|client| client.display_batches)
        .sum::<usize>();
    let snapshots = clients.iter().map(|client| client.snapshots).sum::<usize>();
    let deltas = clients.iter().map(|client| client.deltas).sum::<usize>();
    let client_read_syscalls = clients
        .iter()
        .map(|client| client.read_syscalls)
        .sum::<usize>();

    drop(clients);
    for _ in 0..8 {
        let _ = runtime.poll_once(Some(Duration::from_millis(2)));
    }
    let teardown_start = Instant::now();
    runtime.begin_shutdown().expect("begin shutdown");
    let shutdown = runtime.run_until_empty(Instant::now() + Duration::from_secs(10));
    let teardown_us = teardown_start.elapsed().as_micros();
    let final_metrics = process_metrics();

    let classification = if ids.len() == requested_population && requested_fanout <= 16 {
        "MEASURED"
    } else {
        "PLATFORM_LIMITED"
    };
    println!(
        "pass5_production_result workload={} population_requested={} population_created={} fanout_requested={} fanout_attached={} geometry={}x{} classification={} create_us={} attach_setup_us={} latency_p50_us={} latency_p95_us={} latency_p99_us={} runtime_poll_phase_us={} client_decode_apply_phase_us={} elapsed_us={} throughput_payload_bytes_per_sec={} socket_bytes_received={} client_read_syscalls={} server_send_syscalls={} server_sendmsg_syscalls={} runtime_recvmsg_syscalls={} display_batches={} snapshots={} deltas={} resync_or_recovery_snapshots={} allocations={} reallocations={} bytes_allocated={} reconnect_full_snapshot_us={} rss_baseline_kib={} rss_populated_kib={} rss_final_kib={} incremental_rss_kib={} cpu_percent_sample={} threads_baseline={} threads_populated={} threads_final={} fd_baseline={} fd_populated={} fd_final={} teardown_us={} shutdown_ok={} aggregate_pending_input_final={} semantic_generation={} platform_error={:?}",
        workload.name(),
        requested_population,
        ids.len(),
        requested_fanout,
        fanout,
        columns,
        rows,
        classification,
        create_us,
        setup_us,
        measurement.p50_us,
        measurement.p95_us,
        measurement.p99_us,
        measurement.runtime_poll_us,
        measurement.client_apply_us,
        measurement.elapsed_us,
        measurement.throughput_payload_bytes_per_sec,
        bytes_received,
        client_read_syscalls,
        syscall_counters.send,
        syscall_counters.sendmsg,
        syscall_counters.recvmsg,
        display_batches,
        snapshots,
        deltas,
        snapshots.saturating_sub(fanout),
        allocation_stats.allocations,
        allocation_stats.reallocations,
        allocation_stats.bytes_allocated,
        reconnect_us,
        baseline.rss_kib,
        populated.rss_kib,
        final_metrics.rss_kib,
        populated.rss_kib.saturating_sub(baseline.rss_kib),
        populated.cpu_percent,
        baseline.threads,
        populated.threads,
        final_metrics.threads,
        baseline.fds,
        populated.fds,
        final_metrics.fds,
        teardown_us,
        shutdown.is_ok(),
        runtime.aggregate_accepted_but_unwritten_bytes(),
        current_generation,
        platform_error,
    );
    shutdown.expect("controlled Runtime teardown");
    drop(runtime);
    let _ = fs::remove_dir_all(runtime_dir);
}

#[cfg(target_os = "macos")]
fn workload_command(workload: Workload) -> CommandSpec {
    match workload {
        Workload::Interactive => CommandSpec::new("/bin/cat"),
        Workload::Token => CommandSpec::new("/bin/sh").args([
            "-c",
            "read _; i=0; while [ $i -lt 200 ]; do printf 'tok%04d ' \"$i\"; sleep 0.005; i=$((i+1)); done; printf 'DONE\\r\\n'; sleep 1",
        ]),
        Workload::Burst => CommandSpec::new("/bin/sh").args([
            "-c",
            "read _; yes BURST | head -n 10000; printf 'DONE\\r\\n'; sleep 1",
        ]),
        Workload::Sustained => CommandSpec::new("/bin/sh").args([
            "-c",
            "read _; i=0; while [ $i -lt 220 ]; do printf '%04096d\\r\\n' 0; sleep 0.01; i=$((i+1)); done; printf 'DONE\\r\\n'; sleep 1",
        ]),
        Workload::Tui => CommandSpec::new("/bin/sh").args([
            "-c",
            "read _; i=0; while [ $i -lt 100 ]; do printf '\\033[HFRAME%04d\\r\\n' \"$i\"; j=0; while [ $j -lt 30 ]; do printf 'row%02d value%04d\\r\\n' \"$j\" \"$i\"; j=$((j+1)); done; sleep 0.01; i=$((i+1)); done; printf 'DONE\\r\\n'; sleep 1",
        ]),
        Workload::Alternate => CommandSpec::new("/bin/sh").args([
            "-c",
            "read _; printf '\\033[?1049h'; i=0; while [ $i -lt 100 ]; do printf '\\033[HAFRAME%04d\\r\\n' \"$i\"; j=0; while [ $j -lt 30 ]; do printf 'alt%02d value%04d\\r\\n' \"$j\" \"$i\"; j=$((j+1)); done; sleep 0.01; i=$((i+1)); done; printf 'DONE\\r\\n'; sleep 1",
        ]),
    }
}

#[cfg(target_os = "macos")]
struct Measurement {
    p50_us: u128,
    p95_us: u128,
    p99_us: u128,
    runtime_poll_us: u128,
    client_apply_us: u128,
    elapsed_us: u128,
    throughput_payload_bytes_per_sec: u128,
}

#[cfg(target_os = "macos")]
fn measure_interactive(
    runtime: &mut Runtime,
    clients: &mut [BenchClient],
    controller_attachment: seyal_runtime::AttachmentId,
    _execution_id: ExecutionId,
) -> Measurement {
    let mut samples = Vec::with_capacity(32);
    let mut runtime_poll_us = 0u128;
    let mut client_apply_us = 0u128;
    let overall = Instant::now();

    for _ in 0..32 {
        let before = clients
            .iter()
            .map(|client| client.cache.generation)
            .collect::<Vec<_>>();
        let started = Instant::now();
        send_client_frame(
            runtime,
            &mut clients[0],
            MessageType::Input,
            &InputRef {
                attachment_id: controller_attachment,
                bytes: b"x",
            }
            .encode(),
        );
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let poll_start = Instant::now();
            runtime
                .poll_once(Some(Duration::from_millis(1)))
                .expect("Runtime poll");
            runtime_poll_us += poll_start.elapsed().as_micros();

            let client_start = Instant::now();
            for client in clients.iter_mut() {
                client.drain_available().expect("client drain");
            }
            client_apply_us += client_start.elapsed().as_micros();
            if clients
                .iter()
                .zip(&before)
                .all(|(client, generation)| client.cache.generation > *generation)
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "interactive display latency timed out"
            );
        }
        samples.push(started.elapsed().as_micros());
    }

    samples.sort_unstable();
    Measurement {
        p50_us: percentile(&samples, 50),
        p95_us: percentile(&samples, 95),
        p99_us: percentile(&samples, 99),
        runtime_poll_us,
        client_apply_us,
        elapsed_us: overall.elapsed().as_micros(),
        throughput_payload_bytes_per_sec: 0,
    }
}

#[cfg(target_os = "macos")]
fn measure_streaming(
    runtime: &mut Runtime,
    clients: &mut [BenchClient],
    controller_attachment: seyal_runtime::AttachmentId,
    workload: Workload,
) -> Measurement {
    let started = Instant::now();
    send_client_frame(
        runtime,
        &mut clients[0],
        MessageType::Input,
        &InputRef {
            attachment_id: controller_attachment,
            bytes: b"GO\n",
        }
        .encode(),
    );
    let deadline = Instant::now()
        + if workload == Workload::Sustained {
            Duration::from_secs(8)
        } else {
            Duration::from_secs(6)
        };
    let mut completion_samples = vec![0u128; clients.len()];
    let mut runtime_poll_us = 0u128;
    let mut client_apply_us = 0u128;

    loop {
        let poll_start = Instant::now();
        runtime
            .poll_once(Some(Duration::from_millis(1)))
            .expect("Runtime poll");
        runtime_poll_us += poll_start.elapsed().as_micros();

        let client_start = Instant::now();
        for (index, client) in clients.iter_mut().enumerate() {
            client.drain_available().expect("client drain");
            if completion_samples[index] == 0 && cache_contains(&client.cache, "DONE") {
                completion_samples[index] = started.elapsed().as_micros();
            }
        }
        client_apply_us += client_start.elapsed().as_micros();
        if completion_samples.iter().all(|value| *value > 0) {
            break;
        }
        assert!(Instant::now() < deadline, "streaming workload timed out");
    }

    let elapsed = started.elapsed();
    let total_bytes = clients
        .iter()
        .map(|client| client.bytes_received)
        .sum::<usize>();
    let bytes_per_sec = if elapsed.as_nanos() == 0 {
        0
    } else {
        (total_bytes as u128 * 1_000_000_000u128) / elapsed.as_nanos()
    };
    completion_samples.sort_unstable();
    Measurement {
        p50_us: percentile(&completion_samples, 50),
        p95_us: percentile(&completion_samples, 95),
        p99_us: percentile(&completion_samples, 99),
        runtime_poll_us,
        client_apply_us,
        elapsed_us: elapsed.as_micros(),
        throughput_payload_bytes_per_sec: bytes_per_sec,
    }
}

#[cfg(target_os = "macos")]
fn percentile(sorted: &[u128], percentile: usize) -> u128 {
    if sorted.is_empty() {
        return 0;
    }
    let rank = (percentile * sorted.len()).div_ceil(100).max(1);
    sorted[rank.saturating_sub(1).min(sorted.len() - 1)]
}

#[cfg(target_os = "macos")]
struct PendingBatch {
    kind: MessageType,
    expected: usize,
    chunks: Vec<DecodedDisplayChunk>,
}

#[cfg(target_os = "macos")]
struct BenchClient {
    stream: UnixStream,
    buffered: Vec<u8>,
    cache: DisplayCache,
    attachment_id: seyal_runtime::AttachmentId,
    pending: Option<PendingBatch>,
    bytes_received: usize,
    read_syscalls: usize,
    display_batches: usize,
    snapshots: usize,
    deltas: usize,
}

#[cfg(target_os = "macos")]
impl BenchClient {
    fn connect_and_attach(
        runtime: &mut Runtime,
        socket_path: &std::path::Path,
        execution_id: ExecutionId,
        role: Role,
    ) -> Self {
        let stream = UnixStream::connect(socket_path).expect("connect production UDS");
        stream.set_nonblocking(true).expect("nonblocking client");
        let mut client = Self {
            stream,
            buffered: Vec::new(),
            cache: empty_cache(),
            attachment_id: seyal_runtime::AttachmentId::from_bytes([0; 16]),
            pending: None,
            bytes_received: 0,
            read_syscalls: 0,
            display_batches: 0,
            snapshots: 0,
            deltas: 0,
        };
        send_client_frame(
            runtime,
            &mut client,
            MessageType::ClientHello,
            &ClientHello {
                client_capabilities: 0,
            }
            .encode(),
        );
        let (_, hello_payload) = await_frame(runtime, &mut client, MessageType::ServerHello);
        let hello = ServerHello::decode(&hello_payload).expect("ServerHello");
        assert_ne!(
            hello.server_capabilities & seyal_runtime::local_ipc::framing::CAP_BINARY_DISPLAY,
            0
        );

        send_client_frame(
            runtime,
            &mut client,
            MessageType::Attach,
            &Attach {
                execution_id,
                requested_role: role,
            }
            .encode(),
        );
        let (_, attached_payload) = await_frame(runtime, &mut client, MessageType::Attached);
        let attached = Attached::decode(&attached_payload).expect("Attached");
        client.attachment_id = attached.attachment_id;
        let chunks = await_display_batch(runtime, &mut client, MessageType::DisplaySnapshot);
        client.cache.apply_chunks(&chunks).expect("attach snapshot");
        assert_eq!(client.cache.generation, attached.current_generation);
        client
    }

    fn reset_measurement_counters(&mut self) {
        self.bytes_received = 0;
        self.read_syscalls = 0;
        self.display_batches = 0;
        self.snapshots = 0;
        self.deltas = 0;
    }

    fn drain_available(&mut self) -> std::io::Result<()> {
        let mut buffer = [0u8; 32 * 1024];
        loop {
            match self.stream.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => {
                    self.bytes_received += count;
                    self.read_syscalls += 1;
                    self.buffered.extend_from_slice(&buffer[..count]);
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(error) => return Err(error),
            }
        }

        while let Some((kind, payload)) = take_frame(&mut self.buffered) {
            let Some(message_type) = MessageType::from_u16(kind) else {
                continue;
            };
            if !matches!(
                message_type,
                MessageType::DisplaySnapshot | MessageType::DisplayDelta
            ) {
                continue;
            }
            let chunk =
                decode_chunk(&encode_frame(message_type, &payload)).expect("display decode");
            let expected = chunk.chunk_count as usize;
            if self.pending.is_none() {
                self.pending = Some(PendingBatch {
                    kind: message_type,
                    expected,
                    chunks: Vec::with_capacity(expected),
                });
            }
            let pending = self.pending.as_mut().expect("pending batch");
            assert_eq!(
                pending.kind, message_type,
                "display chunks must remain contiguous"
            );
            assert_eq!(pending.expected, expected, "chunk count changed mid-batch");
            pending.chunks.push(chunk);
            if pending.chunks.len() == pending.expected {
                let complete = self.pending.take().expect("complete batch");
                self.cache
                    .apply_chunks(&complete.chunks)
                    .expect("client cache apply");
                self.display_batches += 1;
                match complete.kind {
                    MessageType::DisplaySnapshot => self.snapshots += 1,
                    MessageType::DisplayDelta => self.deltas += 1,
                    _ => unreachable!(),
                }
            }
        }
        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn send_client_frame(
    runtime: &mut Runtime,
    client: &mut BenchClient,
    message_type: MessageType,
    payload: &[u8],
) {
    let frame = encode_frame(message_type, payload);
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut sent = 0usize;
    while sent < frame.len() {
        match client.stream.write(&frame[sent..]) {
            Ok(0) => panic!("client socket closed while writing"),
            Ok(count) => sent += count,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                runtime
                    .poll_once(Some(Duration::from_millis(1)))
                    .expect("Runtime poll");
            }
            Err(error) => panic!("client write failed: {error}"),
        }
        assert!(Instant::now() < deadline, "client send timed out");
    }
    runtime
        .poll_once(Some(Duration::from_millis(1)))
        .expect("Runtime poll");
}

#[cfg(target_os = "macos")]
fn await_frame(
    runtime: &mut Runtime,
    client: &mut BenchClient,
    expected: MessageType,
) -> (u16, Vec<u8>) {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(frame) = take_frame(&mut client.buffered) {
            assert_eq!(frame.0, expected as u16);
            return frame;
        }
        let mut buffer = [0u8; 16 * 1024];
        match client.stream.read(&mut buffer) {
            Ok(0) => panic!("client socket closed"),
            Ok(count) => client.buffered.extend_from_slice(&buffer[..count]),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                runtime
                    .poll_once(Some(Duration::from_millis(1)))
                    .expect("Runtime poll");
            }
            Err(error) => panic!("client read failed: {error}"),
        }
        assert!(Instant::now() < deadline, "frame wait timed out");
    }
}

#[cfg(target_os = "macos")]
fn await_display_batch(
    runtime: &mut Runtime,
    client: &mut BenchClient,
    expected: MessageType,
) -> Vec<DecodedDisplayChunk> {
    let (_, payload) = await_frame(runtime, client, expected);
    let first = decode_chunk(&encode_frame(expected, &payload)).expect("display chunk");
    let count = first.chunk_count as usize;
    let mut chunks = vec![first];
    for _ in 1..count {
        let (_, payload) = await_frame(runtime, client, expected);
        chunks.push(decode_chunk(&encode_frame(expected, &payload)).expect("display chunk"));
    }
    chunks
}

#[cfg(target_os = "macos")]
fn take_frame(buffer: &mut Vec<u8>) -> Option<(u16, Vec<u8>)> {
    if buffer.len() < HEADER_LEN {
        return None;
    }
    let header = FrameHeader::decode(&buffer[..HEADER_LEN]).ok()?;
    let total = HEADER_LEN.checked_add(header.payload_len as usize)?;
    if buffer.len() < total {
        return None;
    }
    let frame = buffer.drain(..total).collect::<Vec<_>>();
    Some((header.message_type, frame[HEADER_LEN..].to_vec()))
}

#[cfg(target_os = "macos")]
fn cache_contains(cache: &DisplayCache, needle: &str) -> bool {
    if cache.columns == 0 {
        return false;
    }
    cache.cells.chunks(cache.columns as usize).any(|row| {
        row.iter()
            .map(|cell| cell.scalar)
            .collect::<String>()
            .contains(needle)
    })
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
    let threads = fields
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
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
fn print_host_metadata() {
    let product = command_text("/usr/bin/sw_vers", &["-productVersion"]);
    let build = command_text("/usr/bin/sw_vers", &["-buildVersion"]);
    let model = command_text("/usr/sbin/sysctl", &["-n", "hw.model"]);
    let hardware = command_text("/usr/sbin/sysctl", &["-n", "machdep.cpu.brand_string"]);
    let pty_max = command_text("/usr/sbin/sysctl", &["-n", "kern.tty.ptmx_max"]);
    let rust = command_text("rustc", &["--version"]);
    let commit = command_text("git", &["rev-parse", "HEAD"]);
    println!(
        "pass5_production_host macos_version={} macos_build={} model={:?} hardware={:?} rust={:?} pty_max={} build_mode=release commit={} cpu_measurement=/usr/bin/time_-l repetitions=case_defined",
        product, build, model, hardware, rust, pty_max, commit
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
