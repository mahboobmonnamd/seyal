#[cfg(target_os = "macos")]
use stats_alloc::Region;
use stats_alloc::{INSTRUMENTED_SYSTEM, StatsAlloc};
use std::alloc::System;

#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

#[cfg(target_os = "macos")]
use std::{
    env,
    io::{Read, Write},
    os::unix::net::UnixStream,
    process::Command,
    time::{Duration, Instant},
};

#[cfg(target_os = "macos")]
const ROWS: usize = 24;
#[cfg(target_os = "macos")]
const COLS: usize = 80;
#[cfg(target_os = "macos")]
const CELL_LEN: usize = 16;
#[cfg(target_os = "macos")]
const HEADER_LEN: usize = 32;
#[cfg(target_os = "macos")]
const REPS: usize = 128;
#[cfg(target_os = "macos")]
const TRANSFER_DEADLINE: Duration = Duration::from_secs(2);

fn main() {
    #[cfg(not(target_os = "macos"))]
    println!("pass5_delta_transport PLATFORM_LIMITED target_os!=macos performance_claim=false");

    #[cfg(target_os = "macos")]
    run_macos();
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug)]
enum Scenario {
    Typing,
    TokenStream,
    LogScroll,
    TuiPartial,
    TuiFull,
    FullResync,
}

#[cfg(target_os = "macos")]
impl Scenario {
    fn name(self) -> &'static str {
        match self {
            Self::Typing => "typing_one_cell",
            Self::TokenStream => "token_stream_12_cells",
            Self::LogScroll => "log_scroll_one_row",
            Self::TuiPartial => "tui_partial_6_rows",
            Self::TuiFull => "tui_full_redraw",
            Self::FullResync => "full_snapshot_resync",
        }
    }

    fn parse(value: &str) -> Self {
        match value {
            "typing_one_cell" => Self::Typing,
            "token_stream_12_cells" => Self::TokenStream,
            "log_scroll_one_row" => Self::LogScroll,
            "tui_partial_6_rows" => Self::TuiPartial,
            "tui_full_redraw" => Self::TuiFull,
            "full_snapshot_resync" => Self::FullResync,
            _ => panic!("unknown scenario {value}"),
        }
    }
}

#[cfg(target_os = "macos")]
#[derive(Clone)]
struct Grid {
    generation: u64,
    cells: Vec<[u8; CELL_LEN]>,
}

#[cfg(target_os = "macos")]
impl Grid {
    fn new() -> Self {
        let mut cells = vec![[0; CELL_LEN]; ROWS * COLS];
        for (i, cell) in cells.iter_mut().enumerate() {
            write_cell(cell, b'a' + (i % 26) as u8, 1);
        }
        Self {
            generation: 1,
            cells,
        }
    }
}

#[cfg(target_os = "macos")]
struct Client {
    tx: UnixStream,
    rx: UnixStream,
    receive: Vec<u8>,
    grid: Grid,
}

#[cfg(target_os = "macos")]
#[derive(Default, Clone, Copy)]
struct Sample {
    total_ns: u128,
    server_ns: u128,
    transport_ns: u128,
    client_ns: u128,
    bytes: usize,
    writes: usize,
    server_allocs: usize,
    server_bytes_allocated: usize,
    client_allocs: usize,
    client_bytes_allocated: usize,
}

#[cfg(target_os = "macos")]
fn run_macos() {
    let args = env::args().collect::<Vec<_>>();
    if args.get(1).is_some_and(|v| v == "--worker") {
        let fanout = args[2].parse::<usize>().expect("fanout");
        let scenario = Scenario::parse(&args[3]);
        run_worker(fanout, scenario);
        return;
    }

    println!(
        "pass5_delta_transport performance_claim=false candidate=D prototype_only=true production_transport_unchanged=true"
    );
    println!(
        "design=single_encode_per_execution compact_binary_uds incremental_generation_delta disposable_client_render_cache bounded_full_snapshot_resync"
    );
    println!(
        "geometry={}x{} cell_bytes={} repetitions={} percentile_method=nearest_rank",
        COLS, ROWS, CELL_LEN, REPS
    );
    print_host_metadata();

    let exe = env::current_exe().expect("current exe");
    for scenario in [
        Scenario::Typing,
        Scenario::TokenStream,
        Scenario::LogScroll,
        Scenario::TuiPartial,
        Scenario::TuiFull,
        Scenario::FullResync,
    ] {
        for fanout in [1usize, 2, 10, 16] {
            let output = Command::new("/usr/bin/time")
                .arg("-lp")
                .arg(&exe)
                .args(["--worker", &fanout.to_string(), scenario.name()])
                .output()
                .expect("run delta worker");
            if !output.status.success() {
                panic!(
                    "delta worker failed fanout={fanout} scenario={} stdout={} stderr={}",
                    scenario.name(),
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            print!("{}", String::from_utf8_lossy(&output.stdout));
        }
    }
}

#[cfg(target_os = "macos")]
fn run_worker(fanout: usize, scenario: Scenario) {
    assert!((1..=16).contains(&fanout));
    let mut canonical = Grid::new();
    let mut clients = (0..fanout)
        .map(|_| {
            let (tx, rx) = UnixStream::pair().expect("socketpair");
            tx.set_nonblocking(true).expect("tx nonblocking");
            rx.set_nonblocking(true).expect("rx nonblocking");
            Client {
                tx,
                rx,
                receive: Vec::new(),
                grid: canonical.clone(),
            }
        })
        .collect::<Vec<_>>();

    let mut samples = Vec::with_capacity(REPS);
    for iteration in 0..REPS {
        let base_generation = canonical.generation;
        let change = mutate(&mut canonical, scenario, iteration as u64 + 2);

        let total_start = Instant::now();
        let server_start = Instant::now();
        let server_region = Region::new(GLOBAL);
        let frame = encode_delta(&canonical, base_generation, &change);
        let server_stats = server_region.change();
        let server_ns = server_start.elapsed().as_nanos();

        let transport_start = Instant::now();
        let mut writes = 0usize;
        for client in &mut clients {
            client.receive.resize(frame.len(), 0);
            writes += transfer_exact(&mut client.tx, &mut client.rx, &frame, &mut client.receive);
        }
        let transport_ns = transport_start.elapsed().as_nanos();

        let client_start = Instant::now();
        let client_region = Region::new(GLOBAL);
        for client in &mut clients {
            apply_delta(&mut client.grid, &client.receive);
            assert_eq!(client.grid.generation, canonical.generation);
            assert_eq!(client.grid.cells, canonical.cells);
            std::hint::black_box(&client.grid);
        }
        let client_stats = client_region.change();
        let client_ns = client_start.elapsed().as_nanos();

        samples.push(Sample {
            total_ns: total_start.elapsed().as_nanos(),
            server_ns,
            transport_ns,
            client_ns,
            bytes: frame.len() * fanout,
            writes,
            server_allocs: server_stats.allocations,
            server_bytes_allocated: server_stats.bytes_allocated,
            client_allocs: client_stats.allocations,
            client_bytes_allocated: client_stats.bytes_allocated,
        });
    }

    println!(
        "delta_transport candidate=D fanout={} scenario={} semantic_match=true total_p50_us={} total_p95_us={} total_p99_us={} server_p50_us={} transport_p50_us={} client_p50_us={} socket_bytes_per_update={} write_calls_p50={} server_allocations_p50={} server_bytes_allocated_p50={} client_allocations_p50={} client_bytes_allocated_p50={} client_cache_bytes={} classification=MEASURED prototype_only=true",
        fanout,
        scenario.name(),
        percentile_us(&samples, |s| s.total_ns, 50),
        percentile_us(&samples, |s| s.total_ns, 95),
        percentile_us(&samples, |s| s.total_ns, 99),
        percentile_us(&samples, |s| s.server_ns, 50),
        percentile_us(&samples, |s| s.transport_ns, 50),
        percentile_us(&samples, |s| s.client_ns, 50),
        percentile_usize(&samples, |s| s.bytes, 50),
        percentile_usize(&samples, |s| s.writes, 50),
        percentile_usize(&samples, |s| s.server_allocs, 50),
        percentile_usize(&samples, |s| s.server_bytes_allocated, 50),
        percentile_usize(&samples, |s| s.client_allocs, 50),
        percentile_usize(&samples, |s| s.client_bytes_allocated, 50),
        fanout * ROWS * COLS * CELL_LEN,
    );
}

#[cfg(target_os = "macos")]
#[derive(Clone)]
enum Change {
    Runs(Vec<(usize, usize, usize)>),
    ScrollOneRow,
    Full,
}

#[cfg(target_os = "macos")]
fn mutate(grid: &mut Grid, scenario: Scenario, seed: u64) -> Change {
    grid.generation += 1;
    let marker = b'A' + (seed % 26) as u8;
    match scenario {
        Scenario::Typing => {
            let row = ROWS - 1;
            let col = (seed as usize) % COLS;
            write_cell(&mut grid.cells[row * COLS + col], marker, seed);
            Change::Runs(vec![(row, col, 1)])
        }
        Scenario::TokenStream => {
            let row = ROWS - 1;
            let start = ((seed as usize) * 7) % (COLS - 12);
            for col in start..start + 12 {
                write_cell(&mut grid.cells[row * COLS + col], marker, seed);
            }
            Change::Runs(vec![(row, start, 12)])
        }
        Scenario::LogScroll => {
            grid.cells.copy_within(COLS.., 0);
            let start = (ROWS - 1) * COLS;
            for col in 0..COLS {
                write_cell(
                    &mut grid.cells[start + col],
                    marker.wrapping_add((col % 8) as u8),
                    seed,
                );
            }
            Change::ScrollOneRow
        }
        Scenario::TuiPartial => {
            let first = ((seed as usize) % (ROWS - 6)).min(ROWS - 6);
            for row in first..first + 6 {
                for col in 0..COLS {
                    write_cell(&mut grid.cells[row * COLS + col], marker, seed + row as u64);
                }
            }
            Change::Runs((first..first + 6).map(|row| (row, 0, COLS)).collect())
        }
        Scenario::TuiFull | Scenario::FullResync => {
            for (index, cell) in grid.cells.iter_mut().enumerate() {
                write_cell(cell, marker.wrapping_add((index % 8) as u8), seed);
            }
            Change::Full
        }
    }
}

#[cfg(target_os = "macos")]
fn write_cell(cell: &mut [u8; CELL_LEN], ch: u8, seed: u64) {
    cell.fill(0);
    cell[0] = ch;
    cell[4..12].copy_from_slice(&seed.to_le_bytes());
    cell[12] = (seed & 0xff) as u8;
    cell[13] = ((seed >> 8) & 0xff) as u8;
}

#[cfg(target_os = "macos")]
fn encode_delta(grid: &Grid, base_generation: u64, change: &Change) -> Vec<u8> {
    let payload = match change {
        Change::Runs(runs) => runs.iter().map(|(_, _, len)| 6 + len * CELL_LEN).sum(),
        Change::ScrollOneRow => 2 + COLS * CELL_LEN,
        Change::Full => ROWS * COLS * CELL_LEN,
    };
    let mut out = Vec::with_capacity(HEADER_LEN + payload);
    out.extend_from_slice(b"SYDL");
    out.extend_from_slice(&grid.generation.to_le_bytes());
    out.extend_from_slice(&base_generation.to_le_bytes());
    out.extend_from_slice(&(ROWS as u16).to_le_bytes());
    out.extend_from_slice(&(COLS as u16).to_le_bytes());
    let kind = match change {
        Change::Runs(_) => 1u8,
        Change::ScrollOneRow => 2,
        Change::Full => 3,
    };
    out.push(kind);
    out.push(0);
    let count = match change {
        Change::Runs(runs) => runs.len() as u16,
        _ => 0,
    };
    out.extend_from_slice(&count.to_le_bytes());
    out.resize(HEADER_LEN, 0);

    match change {
        Change::Runs(runs) => {
            for (row, start, len) in runs {
                out.extend_from_slice(&(*row as u16).to_le_bytes());
                out.extend_from_slice(&(*start as u16).to_le_bytes());
                out.extend_from_slice(&(*len as u16).to_le_bytes());
                let first = row * COLS + start;
                for cell in &grid.cells[first..first + len] {
                    out.extend_from_slice(cell);
                }
            }
        }
        Change::ScrollOneRow => {
            out.extend_from_slice(&1u16.to_le_bytes());
            for cell in &grid.cells[(ROWS - 1) * COLS..] {
                out.extend_from_slice(cell);
            }
        }
        Change::Full => {
            for cell in &grid.cells {
                out.extend_from_slice(cell);
            }
        }
    }
    out
}

#[cfg(target_os = "macos")]
fn apply_delta(grid: &mut Grid, frame: &[u8]) {
    assert_eq!(&frame[..4], b"SYDL");
    let generation = u64::from_le_bytes(frame[4..12].try_into().unwrap());
    let base = u64::from_le_bytes(frame[12..20].try_into().unwrap());
    let rows = u16::from_le_bytes(frame[20..22].try_into().unwrap()) as usize;
    let cols = u16::from_le_bytes(frame[22..24].try_into().unwrap()) as usize;
    assert_eq!((rows, cols), (ROWS, COLS));
    let kind = frame[24];
    if kind != 3 {
        assert_eq!(base, grid.generation);
    }
    let mut offset = HEADER_LEN;
    match kind {
        1 => {
            let count = u16::from_le_bytes(frame[26..28].try_into().unwrap()) as usize;
            for _ in 0..count {
                let row =
                    u16::from_le_bytes(frame[offset..offset + 2].try_into().unwrap()) as usize;
                let start =
                    u16::from_le_bytes(frame[offset + 2..offset + 4].try_into().unwrap()) as usize;
                let len =
                    u16::from_le_bytes(frame[offset + 4..offset + 6].try_into().unwrap()) as usize;
                offset += 6;
                let first = row * COLS + start;
                for index in 0..len {
                    grid.cells[first + index].copy_from_slice(&frame[offset..offset + CELL_LEN]);
                    offset += CELL_LEN;
                }
            }
        }
        2 => {
            let amount = u16::from_le_bytes(frame[offset..offset + 2].try_into().unwrap()) as usize;
            assert_eq!(amount, 1);
            offset += 2;
            grid.cells.copy_within(COLS.., 0);
            let first = (ROWS - 1) * COLS;
            for index in 0..COLS {
                grid.cells[first + index].copy_from_slice(&frame[offset..offset + CELL_LEN]);
                offset += CELL_LEN;
            }
        }
        3 => {
            for index in 0..ROWS * COLS {
                grid.cells[index].copy_from_slice(&frame[offset..offset + CELL_LEN]);
                offset += CELL_LEN;
            }
        }
        _ => panic!("unknown delta kind"),
    }
    assert_eq!(offset, frame.len());
    grid.generation = generation;
}

#[cfg(target_os = "macos")]
fn transfer_exact(
    tx: &mut UnixStream,
    rx: &mut UnixStream,
    frame: &[u8],
    receive: &mut [u8],
) -> usize {
    let deadline = Instant::now() + TRANSFER_DEADLINE;
    let mut sent = 0usize;
    let mut read = 0usize;
    let mut writes = 0usize;
    while read < frame.len() {
        if sent < frame.len() {
            match tx.write(&frame[sent..]) {
                Ok(0) => panic!("socket write zero"),
                Ok(n) => {
                    sent += n;
                    writes += 1;
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(err) => panic!("socket write failed: {err}"),
            }
        }
        match rx.read(&mut receive[read..]) {
            Ok(0) => panic!("socket read zero"),
            Ok(n) => read += n,
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(err) => panic!("socket read failed: {err}"),
        }
        assert!(Instant::now() < deadline, "socket transfer timed out");
    }
    writes
}

#[cfg(target_os = "macos")]
fn percentile_us(samples: &[Sample], f: impl Fn(&Sample) -> u128, pct: usize) -> u128 {
    let mut values = samples.iter().map(f).collect::<Vec<_>>();
    values.sort_unstable();
    let index = ((values.len() * pct + 99) / 100)
        .saturating_sub(1)
        .min(values.len() - 1);
    values[index] / 1_000
}

#[cfg(target_os = "macos")]
fn percentile_usize(samples: &[Sample], f: impl Fn(&Sample) -> usize, pct: usize) -> usize {
    let mut values = samples.iter().map(f).collect::<Vec<_>>();
    values.sort_unstable();
    let index = ((values.len() * pct + 99) / 100)
        .saturating_sub(1)
        .min(values.len() - 1);
    values[index]
}

#[cfg(target_os = "macos")]
fn print_host_metadata() {
    let macos = command_output("sw_vers", &["-productVersion"]);
    let build = command_output("sw_vers", &["-buildVersion"]);
    let model = command_output("sysctl", &["-n", "hw.model"]);
    let chip = command_output("sysctl", &["-n", "machdep.cpu.brand_string"]);
    let rust = command_output("rustc", &["--version"]);
    let commit = command_output("git", &["rev-parse", "HEAD"]);
    println!(
        "delta_transport_host macos_version={} macos_build={} machine_model={:?} hardware={:?} rust={:?} build_mode=release commit={}",
        macos, build, model, chip, rust, commit
    );
}

#[cfg(target_os = "macos")]
fn command_output(program: &str, args: &[&str]) -> String {
    Command::new(program)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap_or_else(|| "unavailable".to_owned())
}
