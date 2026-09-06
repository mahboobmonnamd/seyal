use std::{collections::VecDeque, env, fs, hint::black_box, process::Command, time::Instant};

#[repr(C)]
#[derive(Clone, Copy)]
struct Atom {
    scalar: u32,
    style: u32,
    width: u8,
    flags: u8,
    combining: u16,
}

const ATOM: Atom = Atom { scalar: 97, style: 0, width: 1, flags: 0, combining: 0 };

struct Segmented {
    segments: VecDeque<Box<[Atom]>>,
    tail: Vec<Atom>,
    segment_target: usize,
    limit: usize,
    atoms: usize,
    sealed: usize,
}

impl Segmented {
    fn new(limit: usize, segment_target: usize) -> Self {
        Self {
            segments: VecDeque::new(),
            tail: Vec::with_capacity(segment_target),
            segment_target,
            limit,
            atoms: 0,
            sealed: 0,
        }
    }

    fn append_line(&mut self, len: usize) {
        if !self.tail.is_empty() && self.tail.len() + len > self.segment_target {
            self.seal();
        }
        self.tail.extend(std::iter::repeat_n(ATOM, len));
        self.atoms += len;
        if self.tail.len() >= self.segment_target {
            self.seal();
        }
    }

    fn seal(&mut self) {
        if self.tail.is_empty() { return; }
        let replacement = Vec::with_capacity(self.segment_target);
        let old = std::mem::replace(&mut self.tail, replacement);
        self.segments.push_back(old.into_boxed_slice());
        self.sealed += 1;
        self.evict();
    }

    fn evict(&mut self) {
        while self.atoms > self.limit && self.segments.len() > 1 {
            if let Some(segment) = self.segments.pop_front() {
                self.atoms = self.atoms.saturating_sub(segment.len());
            }
        }
    }

    fn finish(&mut self) {
        self.seal();
        self.evict();
    }
}

fn line_len(ordinal: usize) -> usize {
    let base = if ordinal % 97 == 0 { 640 } else { 64 + ordinal % 97 };
    base + usize::from(ordinal % 16 == 0) * 5
}

fn build(limit: usize, segment_target: usize) -> Segmented {
    let mut store = Segmented::new(limit, segment_target);
    let mut produced = 0usize;
    let mut ordinal = 0usize;
    while produced < limit + 512 {
        let len = line_len(ordinal);
        produced += len;
        store.append_line(len);
        ordinal += 1;
    }
    store.finish();
    store
}

fn rss_kib() -> u64 {
    let Ok(status) = fs::read_to_string("/proc/self/status") else { return 0; };
    status.lines().find_map(|line| {
        line.strip_prefix("VmRSS:")?.split_whitespace().next()?.parse::<u64>().ok()
    }).unwrap_or(0)
}

fn child(target: usize, limit: usize, executions: usize) {
    let baseline = rss_kib();
    let started = Instant::now();
    let mut stores = Vec::with_capacity(executions);
    for _ in 0..executions {
        stores.push(build(limit, target));
    }
    let build_ns = started.elapsed().as_nanos();
    let rss = rss_kib().saturating_sub(baseline);
    let retained: usize = stores.iter().map(|store| store.atoms).sum();
    let sealed: usize = stores.iter().map(|store| store.sealed).sum();
    black_box(&stores);
    println!("target={target}|limit={limit}|executions={executions}|retained={retained}|sealed={sealed}|rss_kib={rss}|build_ns={build_ns}");
}

fn run_child(target: usize, limit: usize, executions: usize) -> String {
    let output = Command::new(env::current_exe().expect("exe"))
        .args(["--child", &target.to_string(), &limit.to_string(), &executions.to_string()])
        .output().expect("child");
    assert!(output.status.success());
    String::from_utf8(output.stdout).expect("utf8").trim().to_owned()
}

fn field<'a>(record: &'a str, key: &str) -> &'a str {
    record.split('|').find_map(|part| part.strip_prefix(&format!("{key}="))).unwrap_or("")
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.get(1).is_some_and(|arg| arg == "--child") {
        child(args[2].parse().unwrap(), args[3].parse().unwrap(), args[4].parse().unwrap());
        return;
    }

    let targets = [512usize, 1024, 2048, 4096];
    let limits = [10_000usize, 100_000, 1_000_000];
    let mut records = Vec::new();
    for target in targets {
        for limit in limits {
            records.push(run_child(target, limit, 1));
        }
        records.push(run_child(target, 10_000, 100));
    }

    let mut report = String::from("# Issue #685 segment granularity calibration\n\nSegments seal only at logical-line boundaries. `target atoms` is a proxy for a future byte-targeted chunk size; it is deliberately not an architectural line-count constant.\n\n| target atoms | history budget | executions | retained atoms | retention % | sealed segments | RSS KiB | build ms |\n|---:|---:|---:|---:|---:|---:|---:|---:|\n");
    for record in records {
        let limit = field(&record, "limit").parse::<f64>().unwrap();
        let executions = field(&record, "executions").parse::<f64>().unwrap();
        let retained = field(&record, "retained").parse::<f64>().unwrap();
        let retention = retained / (limit * executions) * 100.0;
        let build_ms = field(&record, "build_ns").parse::<f64>().unwrap() / 1_000_000.0;
        report.push_str(&format!("| {} | {} | {} | {} | {:.2} | {} | {} | {:.2} |\n",
            field(&record, "target"), field(&record, "limit"), field(&record, "executions"),
            field(&record, "retained"), retention, field(&record, "sealed"), field(&record, "rss_kib"), build_ms));
    }
    report.push_str("\nInterpretation: choose a bounded byte-targeted segment size small enough that eviction overshoot is negligible at the minimum supported history budget, while retaining segment-level allocation/index/persistence advantages. A fixed number of logical lines is rejected because line lengths are unbounded and workload-dependent.\n");
    fs::write("segment-calibration.md", report).expect("write report");
}
