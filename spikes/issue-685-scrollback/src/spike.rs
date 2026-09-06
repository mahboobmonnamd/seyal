use std::{collections::VecDeque, env, fs, hint::black_box, process::Command, time::Instant};

const BASE_WIDTH: usize = 80;
const SEGMENT_LINES: usize = 64;
const NEEDLE: &[u32] = &[69, 82, 82, 79, 82]; // ERROR

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Atom {
    scalar: u32,
    style: u32,
    width: u8,
    flags: u8,
    combining: u16,
}

#[derive(Debug)]
struct Line {
    id: u64,
    atoms: Vec<Atom>,
}

#[derive(Clone, Copy, Debug)]
struct Anchor {
    line_id: u64,
    atom_offset: usize,
}

#[derive(Debug)]
struct Fragment {
    line_id: u64,
    start: usize,
    atoms: Vec<Atom>,
}

#[derive(Debug)]
struct RowStore {
    frags: VecDeque<Fragment>,
    limit: usize,
    atoms: usize,
    alloc_units: usize,
}

impl RowStore {
    fn new(limit: usize) -> Self {
        Self { frags: VecDeque::new(), limit, atoms: 0, alloc_units: 0 }
    }

    fn append(&mut self, line: Line) {
        let id = line.id;
        let mut start = 0usize;
        let mut col = 0usize;
        let mut frag = Vec::new();
        for (index, atom) in line.atoms.into_iter().enumerate() {
            let width = usize::from(atom.width.max(1));
            if !frag.is_empty() && col + width > BASE_WIDTH {
                self.push(Fragment { line_id: id, start, atoms: std::mem::take(&mut frag) });
                start = index;
                col = 0;
            }
            frag.push(atom);
            col += width;
            if col == BASE_WIDTH {
                self.push(Fragment { line_id: id, start, atoms: std::mem::take(&mut frag) });
                start = index + 1;
                col = 0;
            }
        }
        if !frag.is_empty() {
            self.push(Fragment { line_id: id, start, atoms: frag });
        }
        while self.atoms > self.limit {
            let Some(first_id) = self.frags.front().map(|frag| frag.line_id) else { break; };
            while self.frags.front().is_some_and(|frag| frag.line_id == first_id) {
                if let Some(frag) = self.frags.pop_front() {
                    self.atoms = self.atoms.saturating_sub(frag.atoms.len());
                }
            }
        }
    }

    fn push(&mut self, frag: Fragment) {
        self.atoms += frag.atoms.len();
        self.alloc_units += 1;
        self.frags.push_back(frag);
    }

    fn line_count(&self) -> usize {
        let mut count = 0usize;
        let mut previous = None;
        for frag in &self.frags {
            if previous != Some(frag.line_id) {
                count += 1;
                previous = Some(frag.line_id);
            }
        }
        count
    }

    fn reflow(&self, width: usize) -> usize {
        let mut rows = 0usize;
        let mut line_id = None;
        let mut col = 0usize;
        for frag in &self.frags {
            if line_id != Some(frag.line_id) {
                line_id = Some(frag.line_id);
                col = 0;
            }
            for atom in &frag.atoms {
                let atom_width = usize::from(atom.width.max(1));
                if col == 0 { rows += 1; }
                if col != 0 && col + atom_width > width {
                    rows += 1;
                    col = 0;
                }
                col += atom_width;
                if col == width { col = 0; }
            }
        }
        rows
    }

    fn search(&self) -> usize {
        let mut hits = 0usize;
        let mut matched = 0usize;
        let mut line_id = None;
        for frag in &self.frags {
            if line_id != Some(frag.line_id) {
                line_id = Some(frag.line_id);
                matched = 0;
            }
            for atom in &frag.atoms {
                matched = advance_match(matched, atom.scalar, &mut hits);
            }
        }
        hits
    }

    fn resolve(&self, anchor: Anchor) -> bool {
        self.frags.iter().any(|frag| {
            frag.line_id == anchor.line_id
                && anchor.atom_offset >= frag.start
                && anchor.atom_offset < frag.start + frag.atoms.len()
        })
    }

    fn serialize(&self, rle: bool) -> Vec<u8> {
        let mut output = Vec::with_capacity(self.atoms * 12);
        let mut line_id = None;
        for frag in &self.frags {
            if line_id != Some(frag.line_id) {
                line_id = Some(frag.line_id);
                encode_header(&mut output, frag.line_id);
            }
            encode_atoms(&mut output, &frag.atoms, rle);
        }
        output
    }
}

#[derive(Debug)]
struct RingStore {
    lines: VecDeque<Line>,
    limit: usize,
    atoms: usize,
    alloc_units: usize,
}

impl RingStore {
    fn new(limit: usize) -> Self {
        Self { lines: VecDeque::new(), limit, atoms: 0, alloc_units: 0 }
    }

    fn append(&mut self, line: Line) {
        self.atoms += line.atoms.len();
        self.alloc_units += 1;
        self.lines.push_back(line);
        while self.atoms > self.limit {
            let Some(line) = self.lines.pop_front() else { break; };
            self.atoms = self.atoms.saturating_sub(line.atoms.len());
        }
    }

    fn reflow(&self, width: usize) -> usize {
        self.lines.iter().map(|line| wrapped_rows(&line.atoms, width)).sum()
    }

    fn search(&self) -> usize {
        self.lines.iter().map(|line| search_atoms(&line.atoms)).sum()
    }

    fn resolve(&self, anchor: Anchor) -> bool {
        self.lines.iter().find(|line| line.id == anchor.line_id)
            .is_some_and(|line| anchor.atom_offset < line.atoms.len())
    }

    fn serialize(&self, rle: bool) -> Vec<u8> {
        let mut output = Vec::with_capacity(self.atoms * 12);
        for line in &self.lines {
            encode_header(&mut output, line.id);
            encode_atoms(&mut output, &line.atoms, rle);
        }
        output
    }
}

#[derive(Clone, Copy, Debug)]
struct Meta { id: u64, start: u32, len: u32 }

#[derive(Debug)]
struct Segment {
    metas: Box<[Meta]>,
    atoms: Box<[Atom]>,
    min_id: u64,
    max_id: u64,
}

#[derive(Debug)]
struct Builder {
    metas: Vec<Meta>,
    atoms: Vec<Atom>,
}

impl Builder {
    fn new() -> Self {
        Self { metas: Vec::with_capacity(SEGMENT_LINES), atoms: Vec::with_capacity(SEGMENT_LINES * 128) }
    }
}

#[derive(Debug)]
struct SegmentedStore {
    segments: VecDeque<Segment>,
    tail: Builder,
    limit: usize,
    atoms: usize,
    alloc_units: usize,
}

impl SegmentedStore {
    fn new(limit: usize) -> Self {
        Self { segments: VecDeque::new(), tail: Builder::new(), limit, atoms: 0, alloc_units: 2 }
    }

    fn append(&mut self, line: Line) {
        if self.tail.metas.len() == SEGMENT_LINES { self.seal(); }
        let start = self.tail.atoms.len();
        let len = line.atoms.len();
        self.tail.metas.push(Meta {
            id: line.id,
            start: u32::try_from(start).expect("segment offset fits u32"),
            len: u32::try_from(len).expect("line length fits u32"),
        });
        self.tail.atoms.extend(line.atoms);
        self.atoms += len;
    }

    fn finish(&mut self) {
        self.seal();
        self.evict();
    }

    fn seal(&mut self) {
        if self.tail.metas.is_empty() { return; }
        let old = std::mem::replace(&mut self.tail, Builder::new());
        self.alloc_units += 2;
        let min_id = old.metas.first().expect("non-empty segment").id;
        let max_id = old.metas.last().expect("non-empty segment").id;
        self.segments.push_back(Segment {
            metas: old.metas.into_boxed_slice(),
            atoms: old.atoms.into_boxed_slice(),
            min_id,
            max_id,
        });
        self.evict();
    }

    fn evict(&mut self) {
        while self.atoms > self.limit && self.segments.len() > 1 {
            if let Some(segment) = self.segments.pop_front() {
                self.atoms = self.atoms.saturating_sub(segment.atoms.len());
            }
        }
    }

    fn line_count(&self) -> usize {
        self.segments.iter().map(|segment| segment.metas.len()).sum()
    }

    fn reflow(&self, width: usize) -> usize {
        self.segments.iter().map(|segment| {
            segment.metas.iter().map(|meta| wrapped_rows(segment_slice(segment, *meta), width)).sum::<usize>()
        }).sum()
    }

    fn search(&self) -> usize {
        self.segments.iter().map(|segment| {
            segment.metas.iter().map(|meta| search_atoms(segment_slice(segment, *meta))).sum::<usize>()
        }).sum()
    }

    fn resolve(&self, anchor: Anchor) -> bool {
        for segment in &self.segments {
            if anchor.line_id < segment.min_id || anchor.line_id > segment.max_id { continue; }
            return segment.metas.iter().find(|meta| meta.id == anchor.line_id)
                .is_some_and(|meta| anchor.atom_offset < meta.len as usize);
        }
        false
    }

    fn serialize(&self, rle: bool) -> Vec<u8> {
        let mut output = Vec::with_capacity(self.atoms * 12);
        for segment in &self.segments {
            for meta in &segment.metas {
                encode_header(&mut output, meta.id);
                encode_atoms(&mut output, segment_slice(segment, *meta), rle);
            }
        }
        output
    }
}

fn segment_slice(segment: &Segment, meta: Meta) -> &[Atom] {
    let start = meta.start as usize;
    &segment.atoms[start..start + meta.len as usize]
}

#[derive(Debug)]
enum Store {
    Rows(RowStore),
    Ring(RingStore),
    Segmented(SegmentedStore),
}

impl Store {
    fn new(kind: &str, limit: usize) -> Self {
        match kind {
            "rows" => Self::Rows(RowStore::new(limit)),
            "ring" => Self::Ring(RingStore::new(limit)),
            "segmented" => Self::Segmented(SegmentedStore::new(limit)),
            _ => panic!("unknown candidate"),
        }
    }

    fn append(&mut self, line: Line) {
        match self { Self::Rows(s) => s.append(line), Self::Ring(s) => s.append(line), Self::Segmented(s) => s.append(line) }
    }
    fn finish(&mut self) { if let Self::Segmented(s) = self { s.finish(); } }
    fn atoms(&self) -> usize { match self { Self::Rows(s) => s.atoms, Self::Ring(s) => s.atoms, Self::Segmented(s) => s.atoms } }
    fn lines(&self) -> usize { match self { Self::Rows(s) => s.line_count(), Self::Ring(s) => s.lines.len(), Self::Segmented(s) => s.line_count() } }
    fn alloc_units(&self) -> usize { match self { Self::Rows(s) => s.alloc_units, Self::Ring(s) => s.alloc_units, Self::Segmented(s) => s.alloc_units } }
    fn reflow(&self, width: usize) -> usize { match self { Self::Rows(s) => s.reflow(width), Self::Ring(s) => s.reflow(width), Self::Segmented(s) => s.reflow(width) } }
    fn search(&self) -> usize { match self { Self::Rows(s) => s.search(), Self::Ring(s) => s.search(), Self::Segmented(s) => s.search() } }
    fn resolve(&self, anchor: Anchor) -> bool { match self { Self::Rows(s) => s.resolve(anchor), Self::Ring(s) => s.resolve(anchor), Self::Segmented(s) => s.resolve(anchor) } }
    fn serialize(&self, rle: bool) -> Vec<u8> { match self { Self::Rows(s) => s.serialize(rle), Self::Ring(s) => s.serialize(rle), Self::Segmented(s) => s.serialize(rle) } }
}

fn advance_match(matched: usize, scalar: u32, hits: &mut usize) -> usize {
    if scalar == NEEDLE[matched] {
        let next = matched + 1;
        if next == NEEDLE.len() { *hits += 1; 0 } else { next }
    } else {
        usize::from(scalar == NEEDLE[0])
    }
}

fn search_atoms(atoms: &[Atom]) -> usize {
    let mut hits = 0usize;
    let mut matched = 0usize;
    for atom in atoms { matched = advance_match(matched, atom.scalar, &mut hits); }
    hits
}

fn wrapped_rows(atoms: &[Atom], width: usize) -> usize {
    if atoms.is_empty() { return 1; }
    let mut rows = 0usize;
    let mut col = 0usize;
    for atom in atoms {
        let atom_width = usize::from(atom.width.max(1));
        if col == 0 { rows += 1; }
        if col != 0 && col + atom_width > width {
            rows += 1;
            col = 0;
        }
        col += atom_width;
        if col == width { col = 0; }
    }
    rows
}

fn encode_header(output: &mut Vec<u8>, id: u64) {
    output.extend_from_slice(&id.to_le_bytes());
    output.push(1);
}

fn encode_atoms(output: &mut Vec<u8>, atoms: &[Atom], rle: bool) {
    let mut index = 0usize;
    while index < atoms.len() {
        let atom = atoms[index];
        if rle && atom.scalar == 32 && atom.combining == 0 {
            let mut run = 1usize;
            while index + run < atoms.len() && atoms[index + run] == atom && run < u32::MAX as usize { run += 1; }
            output.push(0);
            output.extend_from_slice(&(run as u32).to_le_bytes());
            output.extend_from_slice(&atom.style.to_le_bytes());
            output.push(atom.width);
            index += run;
        } else {
            output.push(1);
            output.extend_from_slice(&atom.scalar.to_le_bytes());
            output.extend_from_slice(&atom.style.to_le_bytes());
            output.push(atom.width);
            output.push(atom.flags);
            output.extend_from_slice(&atom.combining.to_le_bytes());
            index += 1;
        }
    }
}

fn make_line(id: u64, ordinal: usize) -> Line {
    let body = if ordinal % 97 == 0 { BASE_WIDTH * 8 } else { 64 + ordinal % 97 };
    let marker = ordinal % 16 == 0;
    let mut atoms = Vec::with_capacity(body + if marker { NEEDLE.len() } else { 0 });
    if marker {
        for &scalar in NEEDLE {
            atoms.push(Atom { scalar, style: 7, width: 1, flags: 0, combining: 0 });
        }
    }
    for index in 0..body {
        let (scalar, width, flags, combining) = if index % 53 == 0 {
            ('界' as u32, 2, 0, 0)
        } else if index % 47 == 0 {
            ('e' as u32, 1, 1, 1)
        } else if index % 7 <= 1 {
            (32, 1, 0, 0)
        } else {
            ((b'a' + (index % 26) as u8) as u32, 1, 0, 0)
        };
        atoms.push(Atom { scalar, style: (ordinal % 8) as u32, width, flags, combining });
    }
    Line { id, atoms }
}

fn build(kind: &str, limit: usize, seed: u64) -> (Store, Vec<Anchor>) {
    let mut store = Store::new(kind, limit);
    let mut anchors = Vec::new();
    let mut produced = 0usize;
    let mut ordinal = 0usize;
    let mut id = seed;
    while produced < limit + 512 {
        if ordinal != 0 && ordinal % 64 == 0 { id += 24; }
        let line = make_line(id, ordinal);
        if ordinal % 8 == 0 && line.atoms.len() > 4 {
            anchors.push(Anchor { line_id: id, atom_offset: 3 });
        }
        produced += line.atoms.len();
        store.append(line);
        id += 1;
        ordinal += 1;
    }
    store.finish();
    (store, anchors)
}

fn rss_kib() -> u64 {
    let Ok(status) = fs::read_to_string("/proc/self/status") else { return 0; };
    status.lines().find_map(|line| {
        line.strip_prefix("VmRSS:")?.split_whitespace().next()?.parse::<u64>().ok()
    }).unwrap_or(0)
}

fn percentile(mut values: Vec<u128>, percent: usize) -> u128 {
    values.sort_unstable();
    values[(values.len().saturating_sub(1) * percent) / 100]
}

fn bench(kind: &str, limit: usize) {
    let baseline_rss = rss_kib();
    let started = Instant::now();
    let (store, anchors) = build(kind, limit, 1);
    let append_ns = started.elapsed().as_nanos();
    let rss_kib = rss_kib().saturating_sub(baseline_rss);

    let search_hits = store.search();
    assert!(search_hits > 0, "retained search fixture missing");
    let surviving: Vec<_> = anchors.into_iter().filter(|anchor| store.resolve(*anchor)).collect();
    assert!(!surviving.is_empty(), "retained selection anchor missing");

    let widths = [40usize, 80, 132, 64, 160, 96, 48];
    let mut reflow_ns = Vec::new();
    let mut checksum = 0usize;
    for iteration in 0..35 {
        let started = Instant::now();
        checksum ^= black_box(store.reflow(widths[iteration % widths.len()]));
        reflow_ns.push(started.elapsed().as_nanos());
    }

    let mut search_ns = Vec::new();
    for _ in 0..15 {
        let started = Instant::now();
        black_box(store.search());
        search_ns.push(started.elapsed().as_nanos());
    }

    let started = Instant::now();
    let mut selection_hits = 0usize;
    for _ in 0..100 {
        for &anchor in &surviving { selection_hits += usize::from(store.resolve(anchor)); }
    }
    let selection_ns = started.elapsed().as_nanos() / (surviving.len() * 100) as u128;

    let started = Instant::now();
    let raw = store.serialize(false);
    let raw_ns = started.elapsed().as_nanos();
    let started = Instant::now();
    let rle = store.serialize(true);
    let rle_ns = started.elapsed().as_nanos();

    println!("kind=bench|candidate={kind}|target={limit}|atoms={}|lines={}|rss_kib={rss_kib}|append_ns={append_ns}|alloc_units={}|reflow_p50_ns={}|reflow_p95_ns={}|reflow_p99_ns={}|search_p50_ns={}|search_p95_ns={}|search_hits={search_hits}|selection_ns={selection_ns}|selection_hits={selection_hits}|raw_bytes={}|raw_ns={raw_ns}|rle_bytes={}|rle_ns={rle_ns}|checksum={checksum}",
        store.atoms(), store.lines(), store.alloc_units(),
        percentile(reflow_ns.clone(), 50), percentile(reflow_ns.clone(), 95), percentile(reflow_ns, 99),
        percentile(search_ns.clone(), 50), percentile(search_ns, 95), raw.len(), rle.len());
}

fn population(kind: &str, executions: usize, atoms_each: usize) {
    let baseline_rss = rss_kib();
    let started = Instant::now();
    let mut stores = Vec::with_capacity(executions);
    for execution in 0..executions {
        let (store, _) = build(kind, atoms_each, 1 + execution as u64 * 10_000_000);
        stores.push(store);
    }
    let build_ns = started.elapsed().as_nanos();
    let rss_kib = rss_kib().saturating_sub(baseline_rss);
    let atoms: usize = stores.iter().map(Store::atoms).sum();
    black_box(&stores);
    println!("kind=population|candidate={kind}|executions={executions}|atoms_each={atoms_each}|atoms={atoms}|rss_kib={rss_kib}|build_ns={build_ns}");
}

fn run_child(args: &[String]) -> String {
    let output = Command::new(env::current_exe().expect("current exe")).args(args).output().expect("spawn child");
    assert!(output.status.success(), "child failed: {}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout).expect("UTF-8 output").trim().to_owned()
}

fn field<'a>(record: &'a str, key: &str) -> &'a str {
    record.split('|').find_map(|part| part.strip_prefix(&format!("{key}="))).unwrap_or("")
}

fn micros(ns: &str) -> f64 { ns.parse::<f64>().unwrap_or(0.0) / 1_000.0 }

fn mib_per_second(bytes: &str, ns: &str) -> f64 {
    let bytes = bytes.parse::<f64>().unwrap_or(0.0);
    let ns = ns.parse::<f64>().unwrap_or(1.0).max(1.0);
    bytes / 1_048_576.0 / (ns / 1_000_000_000.0)
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.get(1).is_some_and(|arg| arg == "--bench") {
        bench(&args[2], args[3].parse().expect("limit"));
        return;
    }
    if args.get(1).is_some_and(|arg| arg == "--population") {
        population(&args[2], args[3].parse().expect("executions"), args[4].parse().expect("atoms"));
        return;
    }

    let candidates = ["rows", "ring", "segmented"];
    let sizes = [10_000usize, 100_000, 1_000_000];
    let populations = [1usize, 10, 50, 100];
    let mut benches = Vec::new();
    let mut population_rows = Vec::new();
    for candidate in candidates {
        for size in sizes {
            benches.push(run_child(&["--bench".into(), candidate.into(), size.to_string()]));
        }
        for executions in populations {
            population_rows.push(run_child(&["--population".into(), candidate.into(), executions.to_string(), "10000".into()]));
        }
    }

    let rustc = Command::new("rustc").arg("--version").output().ok()
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_owned()).unwrap_or_default();
    let uname = Command::new("uname").arg("-a").output().ok()
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_owned()).unwrap_or_default();

    let mut report = String::new();
    report.push_str("# Issue #685 scrollback/history spike measurements\n\nExperimental evidence only; no prototype code is production-intent.\n\n");
    report.push_str(&format!("- Rust: `{rustc}`\n- Runner: `{uname}`\n- Synthetic retained atom model includes style, wide-cell and combining metadata.\n- LineId gaps simulate alternate-screen allocations from the one TerminalState allocator.\n- RSS cases run in fresh child processes.\n\n"));
    report.push_str("## Candidate matrix\n\n| candidate | target atoms | retained atoms | lines | RSS KiB | append ms | repr alloc units | reflow p50 us | p95 us | p99 us | search p50 us | p95 us | selection ns/op | raw MiB/s | RLE ratio | RLE MiB/s |\n|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|\n");
    for row in &benches {
        let raw = field(row, "raw_bytes").parse::<f64>().unwrap_or(0.0);
        let rle = field(row, "rle_bytes").parse::<f64>().unwrap_or(0.0);
        let ratio = if raw == 0.0 { 1.0 } else { rle / raw };
        report.push_str(&format!("| {} | {} | {} | {} | {} | {:.2} | {} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {} | {:.1} | {:.3} | {:.1} |\n",
            field(row, "candidate"), field(row, "target"), field(row, "atoms"), field(row, "lines"), field(row, "rss_kib"),
            micros(field(row, "append_ns")) / 1000.0, field(row, "alloc_units"),
            micros(field(row, "reflow_p50_ns")), micros(field(row, "reflow_p95_ns")), micros(field(row, "reflow_p99_ns")),
            micros(field(row, "search_p50_ns")), micros(field(row, "search_p95_ns")), field(row, "selection_ns"),
            mib_per_second(field(row, "raw_bytes"), field(row, "raw_ns")), ratio,
            mib_per_second(field(row, "rle_bytes"), field(row, "rle_ns"))));
    }

    report.push_str("\n## Detached/hidden population slope\n\nEach execution retains approximately 10k atoms.\n\n| candidate | executions | retained atoms | RSS KiB | build ms |\n|---|---:|---:|---:|---:|\n");
    for row in &population_rows {
        report.push_str(&format!("| {} | {} | {} | {} | {:.2} |\n",
            field(row, "candidate"), field(row, "executions"), field(row, "atoms"), field(row, "rss_kib"),
            micros(field(row, "build_ns")) / 1000.0));
    }

    report.push_str("\n## Correctness fixtures exercised\n\n- long logical lines spanning eight 80-column rows;\n- wide cells plus combining/grapheme-side metadata;\n- non-contiguous LineIds caused by alternate-screen allocator consumption;\n- repeated 40/48/64/80/96/132/160-column reflow oscillation;\n- search across retained logical history;\n- selection anchors as `(LineId, atom offset)` surviving reflow;\n- bounded whole-line or whole-segment eviction;\n- 10k/100k/1M retained-atom scales and 1/10/50/100 hidden execution populations;\n- raw persistence serialization and simple space-RLE compression seam.\n\n## Interpretation\n\nAbsolute RSS is runner/allocator dependent and the synthetic atom is not the final Unicode cell representation. Treat relative memory slope, allocation structure, append/reflow/search behavior and persistence shape as R&D evidence. Production gates require macOS measurements against the final implementation and repository VT fixtures.\n");
    fs::write("spike-report.md", report).expect("write report");
    println!("wrote spike-report.md");
}
