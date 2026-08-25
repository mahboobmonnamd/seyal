use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};
use std::alloc::System;

#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

#[cfg(target_os = "macos")]
use stats_alloc::Stats;

#[cfg(target_os = "macos")]
use std::{
    env,
    io::{Read, Write},
    os::fd::{AsRawFd, RawFd},
    os::unix::net::UnixStream,
    process::{self, Command},
    thread,
    time::{Duration, Instant},
};

#[cfg(target_os = "macos")]
use seyal_runtime::{
    AttachmentId, ProjectionId,
    local_ipc::{
        fd_transfer,
        framing::{FrameHeader, GenerationWake, HEADER_LEN, MessageType, encode_frame},
    },
    projection::{
        layout::{
            CELL_LEN, CellRecord, DAMAGE_LEN, DamageRecord, ModeFlags, REGION_HEADER_LEN,
            RegionHeader, SLOT_HEADER_LEN, WireAttributes, WireColor,
        },
        lifecycle::{ProjectionRegion, ReadOnlyMapping},
        producer::OwnedSnapshot,
        writer::{SnapshotRead, Writer, read_latest},
    },
};

#[cfg(target_os = "macos")]
const ROWS: u16 = 24;
#[cfg(target_os = "macos")]
const COLUMNS: u16 = 80;
#[cfg(target_os = "macos")]
const REGULAR_REPETITIONS: usize = 64;
#[cfg(target_os = "macos")]
const BURST_REPETITIONS: usize = 512;
#[cfg(target_os = "macos")]
const REFERENCE_HEADER_LEN: usize = 40;
#[cfg(target_os = "macos")]
const REFERENCE_MAGIC: [u8; 8] = *b"SYLSOCK1";

fn main() {
    #[cfg(not(target_os = "macos"))]
    {
        println!(
            "pass5_transport_stress PLATFORM_LIMITED target_os!=macos performance_claim=false"
        );
        let _ = Region::new(GLOBAL).change();
    }

    #[cfg(target_os = "macos")]
    run_macos();
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy)]
enum Transport {
    SocketOnly,
    Hybrid,
}

#[cfg(target_os = "macos")]
impl Transport {
    fn as_arg(self) -> &'static str {
        match self {
            Self::SocketOnly => "socket-only",
            Self::Hybrid => "hybrid",
        }
    }

    fn parse(value: &str) -> Self {
        match value {
            "socket-only" => Self::SocketOnly,
            "hybrid" => Self::Hybrid,
            _ => panic!("unknown transport {value}"),
        }
    }
}

#[cfg(target_os = "macos")]
fn run_macos() {
    let args = env::args().collect::<Vec<_>>();
    if args.get(1).is_some_and(|value| value == "--worker") {
        let transport = Transport::parse(&args[2]);
        let fanout = args[3].parse::<usize>().expect("fanout");
        let repetitions = args[4].parse::<usize>().expect("repetitions");
        let workload = &args[5];
        match transport {
            Transport::SocketOnly => run_socket_worker(fanout, repetitions, workload),
            Transport::Hybrid => run_hybrid_worker(fanout, repetitions, workload),
        }
        return;
    }

    println!("pass5_transport_stress performance_claim=false");
    println!(
        "scope=derived_complete_snapshot_transport allocator=stats_alloc-0.1.10 allocator_instrumentation_affects_absolute_latency=true"
    );
    println!(
        "decision_use=fanout_allocation_transport_burst_supplement not_replacement_for_uninstrumented_runtime_scalability"
    );
    println!(
        "cpu_scope=combined_worker_process_via_usr_bin_time phase_metrics=wall_time rss_scope=combined_worker_process"
    );
    println!(
        "percentile_method=nearest_rank regular_repetitions={REGULAR_REPETITIONS} burst_repetitions={BURST_REPETITIONS}"
    );
    print_host_metadata();

    let executable = env::current_exe().expect("benchmark executable");
    for fanout in [1usize, 10, 16] {
        for transport in [Transport::SocketOnly, Transport::Hybrid] {
            run_timed_worker(
                &executable,
                transport,
                fanout,
                REGULAR_REPETITIONS,
                "incremental_updates",
            );
        }
    }
    for transport in [Transport::SocketOnly, Transport::Hybrid] {
        run_timed_worker(
            &executable,
            transport,
            16,
            BURST_REPETITIONS,
            "synthetic_derived_state_burst",
        );
    }
}

#[cfg(target_os = "macos")]
fn run_timed_worker(
    executable: &std::path::Path,
    transport: Transport,
    fanout: usize,
    repetitions: usize,
    workload: &str,
) {
    let output = Command::new("/usr/bin/time")
        .arg("-lp")
        .arg(executable)
        .args([
            "--worker",
            transport.as_arg(),
            &fanout.to_string(),
            &repetitions.to_string(),
            workload,
        ])
        .output()
        .expect("launch timed transport worker");
    assert!(output.status.success(), "transport stress worker failed");
    print!("{}", String::from_utf8_lossy(&output.stdout));
    let timing = String::from_utf8_lossy(&output.stderr);
    println!(
        "transport_worker_cpu transport={} fanout={fanout} repetitions={repetitions} workload={workload} user_seconds={} system_seconds={} scope=combined_worker_process source=/usr/bin/time",
        transport.as_arg(),
        time_metric(&timing, "user").unwrap_or_else(|| "unavailable".to_owned()),
        time_metric(&timing, "sys").unwrap_or_else(|| "unavailable".to_owned()),
    );
}

#[cfg(target_os = "macos")]
fn time_metric(output: &str, key: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let mut fields = line.split_whitespace();
        let first = fields.next()?;
        let second = fields.next()?;
        if first == key {
            Some(second.to_owned())
        } else if second == key {
            Some(first.to_owned())
        } else {
            None
        }
    })
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, Default)]
struct Sample {
    total_ns: u128,
    server_ns: u128,
    client_ns: u128,
    server_allocations: usize,
    server_reallocations: usize,
    server_bytes_allocated: usize,
    client_allocations: usize,
    client_reallocations: usize,
    client_bytes_allocated: usize,
    write_calls: usize,
    socket_bytes: usize,
    shm_bytes: usize,
}

#[cfg(target_os = "macos")]
fn allocation_fields(stats: Stats) -> (usize, usize, usize) {
    (
        stats.allocations,
        stats.reallocations,
        stats.bytes_allocated,
    )
}

#[cfg(target_os = "macos")]
struct SocketClient {
    tx: UnixStream,
    rx: UnixStream,
    receive: Vec<u8>,
}

#[cfg(target_os = "macos")]
fn run_socket_worker(fanout: usize, repetitions: usize, workload: &str) {
    assert!((1..=16).contains(&fanout));
    let mut snapshot = sample_snapshot();
    let initial = encode_reference_snapshot(&snapshot);
    let baseline_rss = process_rss_kib();
    let mut clients = (0..fanout)
        .map(|_| {
            let (tx, rx) = UnixStream::pair().expect("socket reference pair");
            tx.set_nonblocking(true).expect("nonblocking tx");
            rx.set_nonblocking(true).expect("nonblocking rx");
            SocketClient {
                tx,
                rx,
                receive: vec![0; initial.len()],
            }
        })
        .collect::<Vec<_>>();

    for client in &mut clients {
        send_stream_all(&mut client.tx, &initial);
        receive_exact(&mut client.rx, &mut client.receive);
        assert!(owned_matches(
            &decode_reference_snapshot(&client.receive),
            &snapshot
        ));
    }
    let populated_rss = process_rss_kib();

    let mut samples = Vec::with_capacity(repetitions);
    for iteration in 0..repetitions {
        mutate_snapshot(&mut snapshot, iteration as u64 + 2);
        let total_start = Instant::now();

        let server_start = Instant::now();
        let server_region = Region::new(GLOBAL);
        let frame = encode_reference_snapshot(&snapshot);
        let mut write_calls = 0usize;
        for client in &mut clients {
            write_calls += send_stream_all(&mut client.tx, &frame);
        }
        let server_stats = server_region.change();
        let server_ns = server_start.elapsed().as_nanos();

        let client_start = Instant::now();
        let client_region = Region::new(GLOBAL);
        for client in &mut clients {
            receive_exact(&mut client.rx, &mut client.receive);
            let decoded = decode_reference_snapshot(&client.receive);
            assert!(owned_matches(&decoded, &snapshot));
            std::hint::black_box(decoded);
        }
        let client_stats = client_region.change();
        let client_ns = client_start.elapsed().as_nanos();
        let (server_allocations, server_reallocations, server_bytes_allocated) =
            allocation_fields(server_stats);
        let (client_allocations, client_reallocations, client_bytes_allocated) =
            allocation_fields(client_stats);
        samples.push(Sample {
            total_ns: total_start.elapsed().as_nanos(),
            server_ns,
            client_ns,
            server_allocations,
            server_reallocations,
            server_bytes_allocated,
            client_allocations,
            client_reallocations,
            client_bytes_allocated,
            write_calls,
            socket_bytes: frame.len() * fanout,
            shm_bytes: 0,
        });
    }

    print_summary(
        Transport::SocketOnly,
        fanout,
        repetitions,
        workload,
        baseline_rss,
        populated_rss,
        &samples,
    );
}

#[cfg(target_os = "macos")]
struct HybridClient {
    writer: Writer,
    _region_owner: ProjectionRegion,
    mapping: ReadOnlyMapping,
    header: RegionHeader,
    attachment_id: AttachmentId,
    projection_id: ProjectionId,
    wake_tx: UnixStream,
    wake_rx: UnixStream,
    wake_receive: Vec<u8>,
}

#[cfg(target_os = "macos")]
fn run_hybrid_worker(fanout: usize, repetitions: usize, workload: &str) {
    assert!((1..=16).contains(&fanout));
    let mut snapshot = sample_snapshot();
    let baseline_rss = process_rss_kib();
    let mut clients = (0..fanout)
        .map(|index| setup_hybrid_client(index, &snapshot))
        .collect::<Vec<_>>();
    let populated_rss = process_rss_kib();

    let mut samples = Vec::with_capacity(repetitions);
    for iteration in 0..repetitions {
        mutate_snapshot(&mut snapshot, iteration as u64 + 2);
        let total_start = Instant::now();

        let server_start = Instant::now();
        let server_region = Region::new(GLOBAL);
        let mut write_calls = 0usize;
        let mut socket_bytes = 0usize;
        for client in &mut clients {
            let generation = client
                .writer
                .publish(&snapshot.as_snapshot_write())
                .expect("publish stress generation");
            let wake = encode_frame(
                MessageType::GenerationWake,
                &GenerationWake {
                    attachment_id: client.attachment_id,
                    projection_id: client.projection_id,
                    committed_generation: generation,
                }
                .encode(),
            );
            socket_bytes += wake.len();
            write_calls += send_no_fd_all(client.wake_tx.as_raw_fd(), &wake);
        }
        let server_stats = server_region.change();
        let server_ns = server_start.elapsed().as_nanos();

        let client_start = Instant::now();
        let client_region = Region::new(GLOBAL);
        for client in &mut clients {
            receive_exact(&mut client.wake_rx, &mut client.wake_receive);
            let frame_header =
                FrameHeader::decode(&client.wake_receive[..HEADER_LEN]).expect("wake header");
            assert_eq!(
                frame_header.message_type,
                MessageType::GenerationWake as u16
            );
            let wake =
                GenerationWake::decode(&client.wake_receive[HEADER_LEN..]).expect("wake body");
            assert_eq!(wake.attachment_id, client.attachment_id);
            assert_eq!(wake.projection_id, client.projection_id);
            let read =
                read_latest(&client.mapping.memory(), &client.header).expect("stress projection");
            assert!(read.generation >= wake.committed_generation);
            assert!(read_matches(&read, &snapshot));
            std::hint::black_box(read);
        }
        let client_stats = client_region.change();
        let client_ns = client_start.elapsed().as_nanos();
        let (server_allocations, server_reallocations, server_bytes_allocated) =
            allocation_fields(server_stats);
        let (client_allocations, client_reallocations, client_bytes_allocated) =
            allocation_fields(client_stats);
        samples.push(Sample {
            total_ns: total_start.elapsed().as_nanos(),
            server_ns,
            client_ns,
            server_allocations,
            server_reallocations,
            server_bytes_allocated,
            client_allocations,
            client_reallocations,
            client_bytes_allocated,
            write_calls,
            socket_bytes,
            shm_bytes: writer_bytes_per_publish(&snapshot) * fanout,
        });
    }

    print_summary(
        Transport::Hybrid,
        fanout,
        repetitions,
        workload,
        baseline_rss,
        populated_rss,
        &samples,
    );
}

#[cfg(target_os = "macos")]
fn setup_hybrid_client(index: usize, snapshot: &OwnedSnapshot) -> HybridClient {
    let attachment_raw = index as u128 + 1;
    let projection_raw = index as u128 + 10_001;
    let attachment_id = AttachmentId::from_bytes(attachment_raw.to_le_bytes());
    let projection_id = ProjectionId::from_bytes(projection_raw.to_le_bytes());
    let stride = slot_stride();
    let header = RegionHeader {
        region_bytes: REGION_HEADER_LEN as u64 + 2 * stride,
        execution_id: 1,
        attachment_id: attachment_raw,
        projection_id: projection_raw,
        slot_stride: stride,
        slot0_offset: REGION_HEADER_LEN as u64,
        capacity_rows: ROWS,
        capacity_cols: COLUMNS,
    };
    let mut region_owner = ProjectionRegion::create(&header).expect("create stress projection");
    let mut writer = Writer::new(region_owner.writer_memory(), header).expect("stress writer");
    writer
        .publish(&snapshot.as_snapshot_write())
        .expect("initial stress publish");
    let reader_fd = region_owner.take_reader_fd().expect("stress reader fd");
    let mapping = ReadOnlyMapping::new(reader_fd, header.region_bytes as usize)
        .expect("stress read-only mapping");
    assert!(read_matches(
        &read_latest(&mapping.memory(), &header).expect("initial stress read"),
        snapshot
    ));
    let (wake_tx, wake_rx) = UnixStream::pair().expect("wake socket pair");
    wake_tx.set_nonblocking(true).expect("nonblocking wake tx");
    wake_rx.set_nonblocking(true).expect("nonblocking wake rx");
    HybridClient {
        writer,
        _region_owner: region_owner,
        mapping,
        header,
        attachment_id,
        projection_id,
        wake_tx,
        wake_rx,
        wake_receive: vec![0; HEADER_LEN + GenerationWake::WIRE_LEN],
    }
}

#[cfg(target_os = "macos")]
fn sample_snapshot() -> OwnedSnapshot {
    let blank = CellRecord {
        scalar: ' ',
        foreground: WireColor::Default,
        background: WireColor::Default,
        attributes: WireAttributes::default(),
    };
    OwnedSnapshot {
        rows: ROWS,
        columns: COLUMNS,
        cursor_row: 0,
        cursor_col: 0,
        cursor_visible: true,
        mode_flags: ModeFlags {
            alternate_screen: false,
            cursor_visible: true,
        },
        cells: vec![blank; ROWS as usize * COLUMNS as usize],
        damages: vec![DamageRecord {
            first_row: 0,
            last_row: 0,
            full: false,
        }],
        full_snapshot: true,
        source_damage_generation: 1,
    }
}

#[cfg(target_os = "macos")]
fn mutate_snapshot(snapshot: &mut OwnedSnapshot, generation: u64) {
    let row = ((generation - 1) % ROWS as u64) as u16;
    let column = ((generation * 7) % COLUMNS as u64) as u16;
    let index = row as usize * COLUMNS as usize + column as usize;
    snapshot.cells[index].scalar =
        char::from_u32(b'!' as u32 + (generation % 80) as u32).expect("ASCII benchmark scalar");
    snapshot.cursor_row = row;
    snapshot.cursor_col = column;
    snapshot.damages[0] = DamageRecord {
        first_row: row,
        last_row: row,
        full: false,
    };
    snapshot.source_damage_generation = generation;
}

#[cfg(target_os = "macos")]
fn slot_stride() -> u64 {
    let bytes =
        SLOT_HEADER_LEN + ROWS as usize * COLUMNS as usize * CELL_LEN + ROWS as usize * DAMAGE_LEN;
    bytes.next_multiple_of(8) as u64
}

#[cfg(target_os = "macos")]
fn writer_bytes_per_publish(snapshot: &OwnedSnapshot) -> usize {
    (SLOT_HEADER_LEN - 8)
        + snapshot.cells.len() * CELL_LEN
        + snapshot.damages.len() * DAMAGE_LEN
        + 24
}

#[cfg(target_os = "macos")]
fn send_stream_all(stream: &mut UnixStream, bytes: &[u8]) -> usize {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut sent = 0usize;
    let mut calls = 0usize;
    while sent < bytes.len() {
        match stream.write(&bytes[sent..]) {
            Ok(0) => panic!("socket closed while writing stress frame"),
            Ok(count) => {
                sent += count;
                calls += 1;
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => thread::yield_now(),
            Err(error) => panic!("stress socket write failed: {error}"),
        }
        assert!(Instant::now() < deadline, "stress socket write timed out");
    }
    calls
}

#[cfg(target_os = "macos")]
fn send_no_fd_all(socket: RawFd, bytes: &[u8]) -> usize {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut sent = 0usize;
    let mut calls = 0usize;
    while sent < bytes.len() {
        match fd_transfer::send_with_fd(socket, &bytes[sent..], None) {
            Ok(0) => thread::yield_now(),
            Ok(count) => {
                sent += count;
                calls += 1;
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => thread::yield_now(),
            Err(error) => panic!("stress wake write failed: {error}"),
        }
        assert!(Instant::now() < deadline, "stress wake write timed out");
    }
    calls
}

#[cfg(target_os = "macos")]
fn receive_exact(stream: &mut UnixStream, buffer: &mut [u8]) {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut received = 0usize;
    while received < buffer.len() {
        match stream.read(&mut buffer[received..]) {
            Ok(0) => panic!("socket closed while reading stress frame"),
            Ok(count) => received += count,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => thread::yield_now(),
            Err(error) => panic!("stress socket read failed: {error}"),
        }
        assert!(Instant::now() < deadline, "stress socket read timed out");
    }
}

#[cfg(target_os = "macos")]
fn read_matches(read: &SnapshotRead, expected: &OwnedSnapshot) -> bool {
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
fn owned_matches(actual: &OwnedSnapshot, expected: &OwnedSnapshot) -> bool {
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
    let total = REFERENCE_HEADER_LEN
        + snapshot.cells.len() * CELL_LEN
        + snapshot.damages.len() * DAMAGE_LEN;
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
    bytes[23] = 0;
    bytes[24..28].copy_from_slice(&(snapshot.cells.len() as u32).to_le_bytes());
    bytes[28..30].copy_from_slice(&(snapshot.damages.len() as u16).to_le_bytes());
    bytes[30..32].fill(0);
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
    assert_eq!(expected_len, bytes.len());
    let cells_end = REFERENCE_HEADER_LEN + cell_count * CELL_LEN;
    let cells = bytes[REFERENCE_HEADER_LEN..cells_end]
        .chunks_exact(CELL_LEN)
        .map(|chunk| CellRecord::decode(chunk).expect("decode reference cell"))
        .collect();
    let damages = bytes[cells_end..]
        .chunks_exact(DAMAGE_LEN)
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
fn print_summary(
    transport: Transport,
    fanout: usize,
    repetitions: usize,
    workload: &str,
    baseline_rss: usize,
    populated_rss: usize,
    samples: &[Sample],
) {
    let total = values(samples, |sample| sample.total_ns);
    let server = values(samples, |sample| sample.server_ns);
    let client = values(samples, |sample| sample.client_ns);
    let server_allocations = values(samples, |sample| sample.server_allocations as u128);
    let server_reallocations = values(samples, |sample| sample.server_reallocations as u128);
    let server_bytes = values(samples, |sample| sample.server_bytes_allocated as u128);
    let client_allocations = values(samples, |sample| sample.client_allocations as u128);
    let client_reallocations = values(samples, |sample| sample.client_reallocations as u128);
    let client_bytes = values(samples, |sample| sample.client_bytes_allocated as u128);
    let write_calls = values(samples, |sample| sample.write_calls as u128);
    let total_ns: u128 = total.iter().sum();
    let updates_per_second = if total_ns == 0 {
        0
    } else {
        repetitions as u128 * 1_000_000_000 / total_ns
    };
    let socket_bytes = samples.last().map_or(0, |sample| sample.socket_bytes);
    let shm_bytes = samples.last().map_or(0, |sample| sample.shm_bytes);
    println!(
        "transport_stress transport={} fanout={fanout} repetitions={repetitions} workload={workload} semantic_match=true total_p50_us={} total_p95_us={} total_p99_us={} server_phase_p50_us={} server_phase_p95_us={} client_phase_p50_us={} client_phase_p95_us={} server_allocations_p50={} server_allocations_p95={} server_reallocations_p50={} server_bytes_allocated_p50={} client_allocations_p50={} client_allocations_p95={} client_reallocations_p50={} client_bytes_allocated_p50={} write_calls_p50={} socket_bytes_per_update={socket_bytes} shm_writer_bytes_per_update={shm_bytes} updates_per_second={updates_per_second} rss_baseline_kib={baseline_rss} rss_populated_kib={populated_rss} rss_incremental_kib={} phase_time_scope=wall_clock rss_scope=combined_worker_process shm_bytes_source=exact_writer_contract socket_bytes_source=actual_frame_lengths",
        transport.as_arg(),
        percentile(&total, 50) / 1_000,
        percentile(&total, 95) / 1_000,
        percentile(&total, 99) / 1_000,
        percentile(&server, 50) / 1_000,
        percentile(&server, 95) / 1_000,
        percentile(&client, 50) / 1_000,
        percentile(&client, 95) / 1_000,
        percentile(&server_allocations, 50),
        percentile(&server_allocations, 95),
        percentile(&server_reallocations, 50),
        percentile(&server_bytes, 50),
        percentile(&client_allocations, 50),
        percentile(&client_allocations, 95),
        percentile(&client_reallocations, 50),
        percentile(&client_bytes, 50),
        percentile(&write_calls, 50),
        populated_rss.saturating_sub(baseline_rss),
    );
}

#[cfg(target_os = "macos")]
fn values(samples: &[Sample], f: impl Fn(&Sample) -> u128) -> Vec<u128> {
    samples.iter().map(f).collect()
}

#[cfg(target_os = "macos")]
fn percentile(values: &[u128], percent: usize) -> u128 {
    assert!(!values.is_empty());
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let rank = (percent * sorted.len()).div_ceil(100).max(1);
    sorted[rank - 1]
}

#[cfg(target_os = "macos")]
fn process_rss_kib() -> usize {
    let output = Command::new("/bin/ps")
        .args(["-o", "rss=", "-p", &process::id().to_string()])
        .output()
        .expect("ps rss");
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .unwrap_or(0)
}

#[cfg(target_os = "macos")]
fn print_host_metadata() {
    let product = command_text("/usr/bin/sw_vers", &["-productVersion"]);
    let build = command_text("/usr/bin/sw_vers", &["-buildVersion"]);
    let model = command_text("/usr/sbin/sysctl", &["-n", "hw.model"]);
    let hardware = command_text("/usr/sbin/sysctl", &["-n", "machdep.cpu.brand_string"]);
    let rustc = command_text("rustc", &["--version"]);
    let commit = command_text("git", &["rev-parse", "HEAD"]);
    println!(
        "transport_stress_host macos_version={product} macos_build={build} machine_model={model:?} hardware={hardware:?} rust={rustc:?} build_mode=release commit={commit} geometry={COLUMNS}x{ROWS}"
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
