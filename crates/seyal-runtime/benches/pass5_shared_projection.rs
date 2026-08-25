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
const TRANSFER_DEADLINE: Duration = Duration::from_secs(2);

fn main() {
    #[cfg(not(target_os = "macos"))]
    {
        println!(
            "pass5_shared_projection PLATFORM_LIMITED target_os!=macos performance_claim=false"
        );
        let _ = Region::new(GLOBAL).change();
    }

    #[cfg(target_os = "macos")]
    run_macos();
}

#[cfg(target_os = "macos")]
fn run_macos() {
    let args = env::args().collect::<Vec<_>>();
    if args.get(1).is_some_and(|value| value == "--worker") {
        let fanout = args[2].parse::<usize>().expect("fanout");
        let repetitions = args[3].parse::<usize>().expect("repetitions");
        let workload = &args[4];
        run_shared_worker(fanout, repetitions, workload);
        return;
    }

    println!("pass5_shared_projection performance_claim=false candidate=C");
    println!(
        "scope=derived_complete_snapshot_transport prototype_only=true production_transport_unchanged=true"
    );
    println!(
        "semantics=one_projection_and_one_publish_per_execution_multiple_independent_readonly_viewers per_view_socket_wake=true"
    );
    println!(
        "abi_note=current_region_header_is_attachment_scoped; prototype_uses_projection_owner_attachment_identity and_does_not_change_production_protocol=true"
    );
    println!(
        "allocator=stats_alloc-0.1.10 allocator_instrumentation_affects_absolute_latency=true percentile_method=nearest_rank regular_repetitions={REGULAR_REPETITIONS} burst_repetitions={BURST_REPETITIONS}"
    );
    print_host_metadata();

    let executable = env::current_exe().expect("benchmark executable");
    for fanout in [1usize, 2, 10, 16] {
        run_timed_worker(
            &executable,
            fanout,
            REGULAR_REPETITIONS,
            "incremental_updates",
        );
    }
    for fanout in [1usize, 16] {
        run_timed_worker(
            &executable,
            fanout,
            BURST_REPETITIONS,
            "synthetic_derived_state_burst",
        );
    }
}

#[cfg(target_os = "macos")]
fn run_timed_worker(
    executable: &std::path::Path,
    fanout: usize,
    repetitions: usize,
    workload: &str,
) {
    let output = Command::new("/usr/bin/time")
        .arg("-lp")
        .arg(executable)
        .args([
            "--worker",
            &fanout.to_string(),
            &repetitions.to_string(),
            workload,
        ])
        .output()
        .expect("launch timed shared-projection worker");
    if !output.status.success() {
        eprintln!(
            "shared projection worker failed fanout={fanout} repetitions={repetitions} workload={workload} status={}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        panic!("shared projection worker failed");
    }
    print!("{}", String::from_utf8_lossy(&output.stdout));
    let timing = String::from_utf8_lossy(&output.stderr);
    println!(
        "shared_projection_worker_cpu fanout={fanout} repetitions={repetitions} workload={workload} user_seconds={} system_seconds={} scope=combined_worker_process source=/usr/bin/time",
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
    transport_ns: u128,
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
struct SharedViewer {
    mapping: ReadOnlyMapping,
    attachment_id: AttachmentId,
    wake_tx: UnixStream,
    wake_rx: UnixStream,
    wake_receive: Vec<u8>,
}

#[cfg(target_os = "macos")]
fn run_shared_worker(fanout: usize, repetitions: usize, workload: &str) {
    assert!((1..=16).contains(&fanout));
    let mut snapshot = sample_snapshot();
    let baseline_rss = process_rss_kib();

    let projection_raw = 10_001u128;
    let projection_id = ProjectionId::from_bytes(projection_raw.to_le_bytes());
    let stride = slot_stride();
    let header = RegionHeader {
        region_bytes: REGION_HEADER_LEN as u64 + 2 * stride,
        execution_id: 1,
        attachment_id: 1,
        projection_id: projection_raw,
        slot_stride: stride,
        slot0_offset: REGION_HEADER_LEN as u64,
        capacity_rows: ROWS,
        capacity_cols: COLUMNS,
    };
    let mut region_owner = ProjectionRegion::create(&header).expect("create shared projection");
    let mut writer = Writer::new(region_owner.writer_memory(), header).expect("shared writer");
    writer
        .publish(&snapshot.as_snapshot_write())
        .expect("initial shared publish");
    let reader_fd = region_owner.take_reader_fd().expect("shared reader fd");

    let mut viewers = Vec::with_capacity(fanout);
    for index in 0..fanout {
        let mapping_fd = reader_fd.try_clone().expect("clone shared read-only fd");
        let mapping = ReadOnlyMapping::new(mapping_fd, header.region_bytes as usize)
            .expect("map shared read-only projection");
        assert!(read_matches(
            &read_latest(&mapping.memory(), &header).expect("initial shared read"),
            &snapshot
        ));
        let (wake_tx, wake_rx) = UnixStream::pair().expect("shared wake pair");
        wake_tx.set_nonblocking(true).expect("nonblocking wake tx");
        wake_rx.set_nonblocking(true).expect("nonblocking wake rx");
        viewers.push(SharedViewer {
            mapping,
            attachment_id: AttachmentId::from_bytes((index as u128 + 1).to_le_bytes()),
            wake_tx,
            wake_rx,
            wake_receive: vec![0; HEADER_LEN + GenerationWake::WIRE_LEN],
        });
    }
    drop(reader_fd);
    let populated_rss = process_rss_kib();
    let mut wake_frames = (0..fanout).map(|_| Vec::new()).collect::<Vec<_>>();

    let mut samples = Vec::with_capacity(repetitions);
    for iteration in 0..repetitions {
        mutate_snapshot(&mut snapshot, iteration as u64 + 2);
        let total_start = Instant::now();

        let server_start = Instant::now();
        let server_region = Region::new(GLOBAL);
        let generation = writer
            .publish(&snapshot.as_snapshot_write())
            .expect("publish shared generation");
        let mut socket_bytes = 0usize;
        for (index, viewer) in viewers.iter().enumerate() {
            wake_frames[index] = encode_frame(
                MessageType::GenerationWake,
                &GenerationWake {
                    attachment_id: viewer.attachment_id,
                    projection_id,
                    committed_generation: generation,
                }
                .encode(),
            );
            socket_bytes += wake_frames[index].len();
        }
        let server_stats = server_region.change();
        let server_ns = server_start.elapsed().as_nanos();

        let transport_start = Instant::now();
        let mut write_calls = 0usize;
        for (viewer, wake) in viewers.iter_mut().zip(&wake_frames) {
            write_calls += transfer_wake(viewer, wake);
        }
        let transport_ns = transport_start.elapsed().as_nanos();

        let client_start = Instant::now();
        let client_region = Region::new(GLOBAL);
        for viewer in &viewers {
            let frame_header =
                FrameHeader::decode(&viewer.wake_receive[..HEADER_LEN]).expect("wake header");
            assert_eq!(
                frame_header.message_type,
                MessageType::GenerationWake as u16
            );
            let wake = GenerationWake::decode(&viewer.wake_receive[HEADER_LEN..])
                .expect("shared wake body");
            assert_eq!(wake.attachment_id, viewer.attachment_id);
            assert_eq!(wake.projection_id, projection_id);
            let read = read_latest(&viewer.mapping.memory(), &header).expect("shared projection");
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
            transport_ns,
            client_ns,
            server_allocations,
            server_reallocations,
            server_bytes_allocated,
            client_allocations,
            client_reallocations,
            client_bytes_allocated,
            write_calls,
            socket_bytes,
            shm_bytes: writer_bytes_per_publish(&snapshot),
        });
    }

    print_summary(
        fanout,
        repetitions,
        workload,
        baseline_rss,
        populated_rss,
        &samples,
    );
}

#[cfg(target_os = "macos")]
fn transfer_wake(viewer: &mut SharedViewer, frame: &[u8]) -> usize {
    let SharedViewer {
        wake_tx,
        wake_rx,
        wake_receive,
        ..
    } = viewer;
    transfer_fd_stream_exact(wake_tx.as_raw_fd(), wake_rx, frame, wake_receive)
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
fn transfer_fd_stream_exact(
    tx: RawFd,
    rx: &mut UnixStream,
    frame: &[u8],
    receive: &mut [u8],
) -> usize {
    assert_eq!(frame.len(), receive.len());
    let deadline = Instant::now() + TRANSFER_DEADLINE;
    let mut written = 0usize;
    let mut received = 0usize;
    let mut write_calls = 0usize;
    while received < frame.len() {
        let mut progressed = false;
        if written < frame.len() {
            match fd_transfer::send_with_fd(tx, &frame[written..], None) {
                Ok(0) => {}
                Ok(count) => {
                    written += count;
                    write_calls += 1;
                    progressed = true;
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(error) => panic!("shared wake write failed: {error}"),
            }
        }
        match rx.read(&mut receive[received..]) {
            Ok(0) => panic!("socket closed while reading shared wake"),
            Ok(count) => {
                received += count;
                progressed = true;
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("shared wake read failed: {error}"),
        }
        if !progressed {
            thread::yield_now();
        }
        assert!(
            Instant::now() < deadline,
            "shared wake transfer timed out written={written} received={received} total={}",
            frame.len()
        );
    }
    assert_eq!(written, frame.len());
    write_calls
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
fn print_summary(
    fanout: usize,
    repetitions: usize,
    workload: &str,
    baseline_rss: usize,
    populated_rss: usize,
    samples: &[Sample],
) {
    let total = values(samples, |sample| sample.total_ns);
    let server = values(samples, |sample| sample.server_ns);
    let transport = values(samples, |sample| sample.transport_ns);
    let client = values(samples, |sample| sample.client_ns);
    let server_allocations = values(samples, |sample| sample.server_allocations as u128);
    let server_reallocations = values(samples, |sample| sample.server_reallocations as u128);
    let server_bytes = values(samples, |sample| sample.server_bytes_allocated as u128);
    let client_allocations = values(samples, |sample| sample.client_allocations as u128);
    let client_reallocations = values(samples, |sample| sample.client_reallocations as u128);
    let client_bytes = values(samples, |sample| sample.client_bytes_allocated as u128);
    let write_calls = values(samples, |sample| sample.write_calls as u128);
    let total_ns: u128 = total.iter().sum();
    let updates_per_second = (repetitions as u128)
        .saturating_mul(1_000_000_000)
        .checked_div(total_ns)
        .unwrap_or(0);
    let socket_bytes = samples.last().map_or(0, |sample| sample.socket_bytes);
    let shm_bytes = samples.last().map_or(0, |sample| sample.shm_bytes);
    println!(
        "transport_stress transport=hybrid-shared-per-execution candidate=C fanout={fanout} repetitions={repetitions} workload={workload} semantic_match=true total_p50_us={} total_p95_us={} total_p99_us={} server_phase_p50_us={} server_phase_p95_us={} transport_phase_p50_us={} transport_phase_p95_us={} client_phase_p50_us={} client_phase_p95_us={} server_allocations_p50={} server_allocations_p95={} server_reallocations_p50={} server_bytes_allocated_p50={} client_allocations_p50={} client_allocations_p95={} client_reallocations_p50={} client_bytes_allocated_p50={} write_calls_p50={} socket_bytes_per_update={socket_bytes} shm_writer_bytes_per_update={shm_bytes} updates_per_second={updates_per_second} rss_baseline_kib={baseline_rss} rss_populated_kib={populated_rss} rss_incremental_kib={} projection_regions=1 phase_time_scope=wall_clock rss_scope=combined_worker_process shm_bytes_source=exact_writer_contract socket_bytes_source=actual_frame_lengths classification=MEASURED prototype_only=true",
        percentile(&total, 50) / 1_000,
        percentile(&total, 95) / 1_000,
        percentile(&total, 99) / 1_000,
        percentile(&server, 50) / 1_000,
        percentile(&server, 95) / 1_000,
        percentile(&transport, 50) / 1_000,
        percentile(&transport, 95) / 1_000,
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
        "shared_projection_host macos_version={product} macos_build={build} machine_model={model:?} hardware={hardware:?} rust={rustc:?} build_mode=release commit={commit} geometry={COLUMNS}x{ROWS}"
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
