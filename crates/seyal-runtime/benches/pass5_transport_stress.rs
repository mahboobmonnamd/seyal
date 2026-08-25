use std::alloc::System;
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, Stats, StatsAlloc};

#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

#[cfg(target_os = "macos")]
use std::{
    env,
    io::{Read, Write},
    os::fd::AsRawFd,
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
            CELL_LEN, DAMAGE_LEN, REGION_HEADER_LEN, SLOT_HEADER_LEN, CellRecord, DamageRecord,
            ModeFlags, RegionHeader, WireAttributes, WireColor,
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
const HIGH_OUTPUT_REPETITIONS: usize = 512;
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
        let _ = Region::new(&GLOBAL).change();
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
    fn arg(self) -> &'static str {
        match self {
            Self::SocketOnly => "socket-only",
            Self::Hybrid => "hybrid",
        }
    }

    fn parse(value: &str) -> Self {
        match value {
            "socket-only" => Self::SocketOnly,
            "hybrid" => Self::Hybrid,
            _ => panic!("invalid transport {value}"),
        }
    }
}

#[cfg(target_os = "macos")]
fn run_macos() {
    let args = env::args().collect::<Vec<_>>();
    if args.get(1).is_some_and(|arg| arg == "--worker") {
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
        "scope=derived_complete_snapshot_to_all_clients allocator=stats_alloc-0.1.10 allocator_instrumentation_affects_absolute_latency=true"
    );
    println!(
        "decision_use=allocation_fanout_high_output_supplement not_replacement_for_uninstrumented_runtime_scalability"
    );
    println!(
        "cpu_rss_accounting=combined_worker_process runtime_client_split=not_meaningful_for_in_process_transport_microbench"
    );
    println!("percentile_method=nearest_rank repetitions_regular={REGULAR_REPETITIONS} repetitions_high_output={HIGH_OUTPUT_REPETITIONS}");
    print_host_metadata();

    let executable = env::current_exe().expect("benchmark executable");
    for fanout in [1usize, 10, 16] {
        for transport in [Transport::SocketOnly, Transport::Hybrid] {
            run_worker_process(
                &executable,
                transport,
                fanout,
                REGULAR_REPETITIONS,
                "incremental_updates",
            );
        }
    }
    for transport in [Transport::SocketOnly, Transport::Hybrid] {
        run_worker_process(
            &executable,
            transport,
            16,
            HIGH_OUTPUT_REPETITIONS,
            "high_output_updates",
        );
    }
}

#[cfg(target_os = "macos")]
fn run_worker_process(
    executable: &std::path::Path,
    transport: Transport,
    fanout: usize,
    repetitions: usize,
    workload: &str,
) {
    let status = Command::new(executable)
        .args([
            "--worker",
            transport.arg(),
            &fanout.to_string(),
            &repetitions.to_string(),
            workload,
        ])
        .status()
        .expect("launch transport stress worker");
    assert!(status.success(), "transport stress worker failed");
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, Default)]
struct Sample {
    elapsed_ns: u128,
    server_allocations: usize,
    server_bytes_allocated: usize,
    client_allocations: usize,
    client_bytes_allocated: usize,
    write_calls: usize,
    socket_bytes: usize,
    shm_bytes: usize,
}

#[cfg(target_os = "macos")]
fn sample_from_stats(
    elapsed: Duration,
    server: Stats,
    client: Stats,
    write_calls: usize,
    socket_bytes: usize,
    shm_bytes: usize,
) -> Sample {
    Sample {
        elapsed_ns: elapsed.as_nanos(),
        server_allocations: server.allocations + server.reallocations,
        server_bytes_allocated: server.bytes_allocated,
        client_allocations: client.allocations + client.reallocations,
        client_bytes_allocated: client.bytes_allocated,
        write_calls,
        socket_bytes,
        shm_bytes,
    }
}

#[cfg(target_os = "macos")]
struct SocketClient {
    tx: UnixStream,
    rx: UnixStream,
    recv: Vec<u8>,
}

#[cfg(target_os = "macos")]
fn run_socket_worker(fanout: usize, repetitions: usize, workload: &str) {
    assert!((1..=16).contains(&fanout));
    let mut snapshot = sample_snapshot();
    let initial_frame = encode_reference_snapshot(&snapshot);
    let baseline_rss = process_rss_kib();
    let mut clients = (0..fanout)
        .map(|_| {
            let (tx, rx) = UnixStream::pair().expect("socket reference pair");
            tx.set_nonblocking(true).expect("nonblocking socket tx");
            rx.set_nonblocking(true).expect("nonblocking socket rx");
            SocketClient {
                tx,
                rx,
                recv: vec![0; initial_frame.len()],
            }
        })
        .collect::<Vec<_>>();
    for client in &mut clients {
        send_stream_all(&mut client.tx, &initial_frame);
        receive_stream_exact(&mut client.rx, &mut client.recv);
        let decoded = decode_reference_snapshot(&client.recv);
        assert!(visible_state_matches_owned(&decoded, &snapshot));
    }
    let populated_rss = process_rss_kib();

    let mut samples = Vec::with_capacity(repetitions);
    for iteration in 0..repetitions {
        mutate_snapshot(&mut snapshot, iteration as u64 + 2);
        let started = Instant::now();

        let server_region = Region::new(&GLOBAL);
        let frame = encode_reference_snapshot(&snapshot);
        let mut write_calls = 0usize;
        for client in &mut clients {
            write_calls += send_stream_all(&mut client.tx, &frame);
        }
        let server_stats = server_region.change();

        let client_region = Region::new(&GLOBAL);
        for client in &mut clients {
            if client.recv.len() != frame.len() {
                client.recv.resize(frame.len(), 0);
            }
            receive_stream_exact(&mut client.rx, &mut client.recv);
            let decoded = decode_reference_snapshot(&client.recv);
            assert!(visible_state_matches_owned(&decoded, &snapshot));
            std::hint::black_box(decoded);
        }
        let client_stats = client_region.change();
        let elapsed = started.elapsed();
        samples.push(sample_from_stats(
            elapsed,
            server_stats,
            client_stats,
            write_calls,
            frame.len() * fanout,
            0,
        ));
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
    region: RegionHeader,
    attachment_id: AttachmentId,
    projection_id: ProjectionId,
    wake_tx: UnixStream,
    wake_rx: UnixStream,
    wake_recv: Vec<u8>,
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
        let started = Instant::now();

        let server_region = Region::new(&GLOBAL);
        let mut write_calls = 0usize;
        let mut socket_bytes = 0usize;
        for client in &mut clients {
            let generation = client
                .writer
                .publish(&snapshot.as_snapshot_write())
                .expect("publish stress generation");
            let frame = encode_frame(
                MessageType::GenerationWake,
                &GenerationWake {
                    attachment_id: client.attachment_id,
                    projection_id: client.projection_id,
                    committed_generation: generation,
                }
                .encode(),
            );
            socket_bytes += frame.len();
            write_calls += send_fd_all(client.wake_tx.as_raw_fd(), &frame);
        }
        let server_stats = server_region.change();

        let client_region = Region::new(&GLOBAL);
        for client in &mut clients {
            receive_stream_exact(&mut client.wake_rx, &mut client.wake_recv);
            let header = FrameHeader::decode(&client.wake_recv[..HEADER_LEN]).expect("wake header");
            assert_eq!(header.message_type, MessageType::GenerationWake as u16);
            let wake = GenerationWake::decode(&client.wake_recv[HEADER_LEN..]).expect("wake payload");
            assert_eq!(wake.attachment_id, client.attachment_id);
            assert_eq!(wake.projection_id, client.projection_id);
            let read = read_latest(&client.mapping.memory(), &client.region)
                .expect("read stress projection");
            assert!(read.generation >= wake.committed_generation);
            assert!(visible_state_matches_read(&read, &snapshot));
            std::hint::black_box(read);
        }
        let client_stats = client_region.change();
        let elapsed = started.elapsed();
        samples.push(sample_from_stats(
            elapsed,
            server_stats,
            client_stats,
            write_calls,
            socket_bytes,
            writer_bytes_per_publish(&snapshot) * fanout,
        ));
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
    let slot_stride = slot_stride();
    let region_bytes = REGION_HEADER_LEN as u64 + 2 * slot_stride;
    let region = RegionHeader {
        region_bytes,
        execution_id: 1,
        attachment_id: attachment_raw,
        projection_id: projection_raw,
        slot_stride,
        slot0_offset: REGION_HEADER_LEN as u64,
        capacity_rows: ROWS,
        capacity_cols: COLUMNS,
    };
    let mut region_owner = ProjectionRegion::create(&region).expect("create stress projection");
    let mut writer = Writer::new(region_owner.writer_memory(), region).expect("stress writer");
    writer
        .publish(&snapshot.as_snapshot_write())
        .expect("initial stress generation");
    let reader_fd = region_owner.take_reader_fd().expect("stress reader fd");
    let mapping = ReadOnlyMapping::new(reader_fd, region_bytes as usize).expect("stress reader map");
    let initial = read_latest(&mapping.memory(), &region).expect("initial stress read");
    assert!(visible_state_matches_read(&initial, snapshot));
    let (wake_tx, wake_rx) = UnixStream::pair().expect("hybrid wake socket pair");
    wake_tx.set_nonblocking(true).expect("nonblocking wake tx");
    wake_rx.set_nonblocking(true).expect("nonblocking wake rx");
    let wake_len = HEADER_LEN + GenerationWake::ENCODED_LEN;
    HybridClient {
        writer,
        _region_owner: region_owner,
        mapping,
        region,
        attachment_id,
        projection_id,
        wake_tx,
        wake_rx,
        wake_recv: vec![0; wake_len],
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
    let row = ((generation - 1) as usize % ROWS as usize) as u16;
    let column = ((generation * 7) as usize % COLUMNS as usize) as u16;
    let index = row as usize * COLUMNS as usize + column as usize;
    snapshot.cells[index].scalar = char::from_u32(b'!' as u32 + (generation % 80) as u32)
        .expect("ASCII benchmark scalar");
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
    let payload = SLOT_HEADER_LEN + ROWS as usize * COLUMNS as usize * CELL_LEN
        + ROWS as usize * DAMAGE_LEN;
    ((payload + 7) & !7) as u64
}

#[cfg(target_os = "macos")]
fn writer_bytes_per_publish(snapshot: &OwnedSnapshot) -> usize {
    // Writer::publish writes the static 56-byte portion of SlotHeader, every
    // encoded cell/damage byte, plus three 8-byte atomic publication words:
    // odd sequence, finalized sequence and region publication.
    (SLOT_HEADER_LEN - 8) + snapshot.cells.len() * CELL_LEN + snapshot.damages.len() * DAMAGE_LEN + 24
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
fn send_fd_all(socket: std::os::fd::RawFd, bytes: &[u8]) -> usize {
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
            Err(error) => panic!("stress hybrid wake write failed: {error}"),
        }
        assert!(Instant::now() < deadline, "stress hybrid wake timed out");
    }
    calls
}

#[cfg(target_os = "macos")]
fn receive_stream_exact(stream: &mut UnixStream, buffer: &mut [u8]) {
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
fn visible_state_matches_read(read: &SnapshotRead, expected: &OwnedSnapshot) -> bool {
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
fn visible_state_matches_owned(actual: &OwnedSnapshot, expected: &OwnedSnapshot) -> bool {
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
fn print_summary(
    transport: Transport,
    fanout: usize,
    repetitions: usize,
    workload: &str,
    baseline_rss: usize,
    populated_rss: usize,
    samples: &[Sample],
) {
    let elapsed = samples.iter().map(|sample| sample.elapsed_ns).collect::<Vec<_>>();
    let server_allocations = samples
        .iter()
        .map(|sample| sample.server_allocations as u128)
        .collect::<Vec<_>>();
    let client_allocations = samples
        .iter()
        .map(|sample| sample.client_allocations as u128)
        .collect::<Vec<_>>();
    let server_bytes = samples
        .iter()
        .map(|sample| sample.server_bytes_allocated as u128)
        .collect::<Vec<_>>();
    let client_bytes = samples
        .iter()
        .map(|sample| sample.client_bytes_allocated as u128)
        .collect::<Vec<_>>();
    let write_calls = samples
        .iter()
        .map(|sample| sample.write_calls as u128)
        .collect::<Vec<_>>();
    let total_ns: u128 = elapsed.iter().sum();
    let updates_per_second = if total_ns == 0 {
        0
    } else {
        ((repetitions as u128) * 1_000_000_000 / total_ns) as u64
    };
    let socket_bytes = samples.last().map_or(0, |sample| sample.socket_bytes);
    let shm_bytes = samples.last().map_or(0, |sample| sample.shm_bytes);
    println!(
        "transport_stress transport={} fanout={fanout} repetitions={repetitions} workload={workload} semantic_match=true latency_p50_us={} latency_p95_us={} latency_p99_us={} server_allocations_p50={} server_allocations_p95={} server_bytes_allocated_p50={} client_allocations_p50={} client_allocations_p95={} client_bytes_allocated_p50={} write_calls_p50={} socket_bytes_per_update={socket_bytes} shm_writer_bytes_per_update={shm_bytes} updates_per_second={updates_per_second} rss_baseline_kib={baseline_rss} rss_populated_kib={populated_rss} rss_incremental_kib={} cpu_percent_after={} allocation_scope=server_then_client_in_process rss_cpu_scope=combined_worker_process shm_bytes_source=exact_writer_contract socket_bytes_source=actual_frame_lengths",
        transport.arg(),
        percentile(&elapsed, 50) / 1_000,
        percentile(&elapsed, 95) / 1_000,
        percentile(&elapsed, 99) / 1_000,
        percentile(&server_allocations, 50),
        percentile(&server_allocations, 95),
        percentile(&server_bytes, 50),
        percentile(&client_allocations, 50),
        percentile(&client_allocations, 95),
        percentile(&client_bytes, 50),
        percentile(&write_calls, 50),
        populated_rss.saturating_sub(baseline_rss),
        process_cpu_percent(),
    );
}

#[cfg(target_os = "macos")]
fn percentile(values: &[u128], percentile: usize) -> u128 {
    assert!(!values.is_empty());
    assert!((1..=100).contains(&percentile));
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let rank = (percentile * sorted.len()).div_ceil(100);
    sorted[rank.saturating_sub(1)]
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
fn process_cpu_percent() -> f32 {
    let output = Command::new("/bin/ps")
        .args(["-o", "%cpu=", "-p", &process::id().to_string()])
        .output()
        .expect("ps cpu");
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .unwrap_or(0.0)
}

#[cfg(target_os = "macos")]
fn print_host_metadata() {
    let product = command_text("/usr/bin/sw_vers", &["-productVersion"]);
    let build = command_text("/usr/bin/sw_vers", &["-buildVersion"]);
    let machine_model = command_text("/usr/sbin/sysctl", &["-n", "hw.model"]);
    let hardware = command_text("/usr/sbin/sysctl", &["-n", "machdep.cpu.brand_string"]);
    let rustc = command_text("rustc", &["--version"]);
    let commit = command_text("git", &["rev-parse", "HEAD"]);
    println!(
        "transport_stress_host macos_version={product} macos_build={build} machine_model={machine_model:?} hardware={hardware:?} rust={rustc:?} build_mode=release commit={commit} geometry={COLUMNS}x{ROWS}"
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
