use std::{env, fs, process::Command};

const ROW_WIDTH: usize = 80;
const SEGMENT_TARGET_UNITS: usize = 512;

#[repr(C)]
#[derive(Clone, Copy)]
struct Atom {
    scalar: u32,
    style: u32,
    width: u8,
    flags: u8,
    combining: u16,
}

struct Row {
    line_id: u64,
    start: u32,
    atoms: Vec<Atom>,
}

struct LogicalLine {
    line_id: u64,
    atoms: Vec<Atom>,
}

struct Segment {
    atoms: Box<[Atom]>,
    lines: Box<[(u64, u32, u32)]>,
}

enum Store {
    Rows(Vec<Row>),
    Ring(Vec<LogicalLine>),
    Segmented(Vec<Segment>),
}

fn atom(index: usize) -> Atom {
    Atom {
        scalar: b'a' as u32 + (index % 26) as u32,
        style: (index % 8) as u32,
        width: if index % 97 == 0 { 2 } else { 1 },
        flags: (index % 3) as u8,
        combining: (index % 5) as u16,
    }
}

fn logical_line_len(line: usize, remaining: usize) -> usize {
    let wanted = if line % 16 == 0 {
        240
    } else {
        12 + (line % 40)
    };
    wanted.min(remaining)
}

fn build_rows(units: usize) -> Store {
    let mut rows = Vec::new();
    let mut produced = 0usize;
    let mut line = 0usize;
    while produced < units {
        let len = logical_line_len(line, units - produced);
        let mut offset = 0usize;
        while offset < len {
            let count = ROW_WIDTH.min(len - offset);
            let mut atoms = Vec::with_capacity(count);
            for i in 0..count {
                atoms.push(atom(produced + offset + i));
            }
            rows.push(Row {
                line_id: line as u64 + 1,
                start: offset as u32,
                atoms,
            });
            offset += count;
        }
        produced += len;
        line += 1;
    }
    Store::Rows(rows)
}

fn build_ring(units: usize) -> Store {
    let mut lines = Vec::new();
    let mut produced = 0usize;
    let mut line = 0usize;
    while produced < units {
        let len = logical_line_len(line, units - produced);
        let mut atoms = Vec::with_capacity(len);
        for i in 0..len {
            atoms.push(atom(produced + i));
        }
        lines.push(LogicalLine {
            line_id: line as u64 + 1,
            atoms,
        });
        produced += len;
        line += 1;
    }
    Store::Ring(lines)
}

fn seal_segment(
    segments: &mut Vec<Segment>,
    atoms: &mut Vec<Atom>,
    lines: &mut Vec<(u64, u32, u32)>,
) {
    if atoms.is_empty() {
        return;
    }
    let sealed_atoms = std::mem::take(atoms).into_boxed_slice();
    let sealed_lines = std::mem::take(lines).into_boxed_slice();
    segments.push(Segment {
        atoms: sealed_atoms,
        lines: sealed_lines,
    });
    *atoms = Vec::with_capacity(SEGMENT_TARGET_UNITS);
    *lines = Vec::with_capacity(32);
}

fn build_segmented(units: usize) -> Store {
    let mut segments = Vec::new();
    let mut current_atoms = Vec::with_capacity(SEGMENT_TARGET_UNITS);
    let mut current_lines = Vec::with_capacity(32);
    let mut produced = 0usize;
    let mut line = 0usize;
    while produced < units {
        let len = logical_line_len(line, units - produced);
        if !current_atoms.is_empty() && current_atoms.len() + len > SEGMENT_TARGET_UNITS {
            seal_segment(&mut segments, &mut current_atoms, &mut current_lines);
        }
        let start = current_atoms.len() as u32;
        for i in 0..len {
            current_atoms.push(atom(produced + i));
        }
        current_lines.push((line as u64 + 1, start, len as u32));
        produced += len;
        line += 1;
    }
    seal_segment(&mut segments, &mut current_atoms, &mut current_lines);
    Store::Segmented(segments)
}

fn build(candidate: &str, units: usize) -> Store {
    match candidate {
        "rows" => build_rows(units),
        "ring" => build_ring(units),
        "segmented" => build_segmented(units),
        other => panic!("unknown candidate {other}"),
    }
}

fn store_checksum(store: &Store) -> (usize, u64) {
    match store {
        Store::Rows(rows) => (
            rows.len(),
            rows.iter()
                .map(|row| row.line_id ^ u64::from(row.start) ^ row.atoms.len() as u64)
                .sum(),
        ),
        Store::Ring(lines) => (
            lines.len(),
            lines.iter()
                .map(|line| line.line_id ^ line.atoms.len() as u64)
                .sum(),
        ),
        Store::Segmented(segments) => (
            segments.len(),
            segments.iter()
                .map(|segment| segment.atoms.len() as u64 ^ segment.lines.len() as u64)
                .sum(),
        ),
    }
}

fn rss_kib() -> usize {
    let status = fs::read_to_string("/proc/self/status").expect("Linux /proc status available");
    status
        .lines()
        .find_map(|line| {
            let value = line.strip_prefix("VmRSS:")?;
            value.split_whitespace().next()?.parse::<usize>().ok()
        })
        .expect("VmRSS present")
}

fn worker(candidate: &str, units_per_execution: usize, executions: usize) {
    let mut stores = Vec::with_capacity(executions);
    if candidate != "baseline" {
        for _ in 0..executions {
            stores.push(build(candidate, units_per_execution));
        }
    }
    let (objects, checksum) = stores.iter().fold((0usize, 0u64), |acc, store| {
        let value = store_checksum(store);
        (acc.0 + value.0, acc.1.wrapping_add(value.1))
    });
    println!("{} {} {}", rss_kib(), objects, checksum);
}

fn run_worker(candidate: &str, units: usize, executions: usize) -> (usize, usize) {
    let exe = env::current_exe().expect("current executable path");
    let output = Command::new(exe)
        .args([
            "worker",
            candidate,
            &units.to_string(),
            &executions.to_string(),
        ])
        .output()
        .expect("worker process starts");
    assert!(
        output.status.success(),
        "worker failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("worker output utf8");
    let mut fields = stdout.split_whitespace();
    let rss = fields.next().expect("rss").parse().expect("numeric rss");
    let objects = fields
        .next()
        .expect("objects")
        .parse()
        .expect("numeric objects");
    (rss, objects)
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.get(1).map(String::as_str) == Some("worker") {
        let candidate = args.get(2).expect("candidate");
        let units = args
            .get(3)
            .expect("units")
            .parse()
            .expect("numeric units");
        let executions = args
            .get(4)
            .expect("executions")
            .parse()
            .expect("numeric executions");
        worker(candidate, units, executions);
        return;
    }

    let baseline = run_worker("baseline", 0, 0).0;
    let candidates = ["rows", "ring", "segmented"];
    let mut report = String::from("# Issue #685 actual RSS calibration\n\n");
    report.push_str(&format!("Linux worker baseline VmRSS: {baseline} KiB. Delta values subtract this process/runtime baseline. Measurements are allocator/OS observations for the isolated spike model, not production RSS promises. The segmented candidate seals builder capacity into exact boxed immutable slices before measurement.\n\n"));
    report.push_str("## Single-execution retained-unit slope\n\n");
    report.push_str("| candidate | retained units | VmRSS KiB | delta KiB | storage objects |\n|---|---:|---:|---:|---:|\n");
    for units in [10_000usize, 100_000, 1_000_000] {
        for candidate in candidates {
            let (rss, objects) = run_worker(candidate, units, 1);
            report.push_str(&format!(
                "| {candidate} | {units} | {rss} | {} | {objects} |\n",
                rss.saturating_sub(baseline)
            ));
        }
    }

    report.push_str("\n## Execution-population scaling\n\nEach execution retains 100,000 canonical units in this calibration.\n\n");
    report.push_str("| candidate | executions | VmRSS KiB | delta KiB | delta/execution KiB | storage objects |\n|---|---:|---:|---:|---:|---:|\n");
    for executions in [1usize, 10, 50, 100] {
        for candidate in candidates {
            let (rss, objects) = run_worker(candidate, 100_000, executions);
            let delta = rss.saturating_sub(baseline);
            report.push_str(&format!(
                "| {candidate} | {executions} | {rss} | {delta} | {} | {objects} |\n",
                delta / executions
            ));
        }
    }

    report.push_str("\nThe RSS run is intentionally separate from the latency harness so allocator reuse from one candidate cannot contaminate another candidate's measurement.\n");
    fs::write("rss-measurement.md", report).expect("write RSS report");
    println!("wrote rss-measurement.md");
}
