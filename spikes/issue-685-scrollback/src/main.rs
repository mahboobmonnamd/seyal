use std::{
    collections::VecDeque,
    env, fs,
    hint::black_box,
    process::Command,
    time::Instant,
};

const INITIAL_WIDTH: usize = 80;
const SEGMENT_LINES: usize = 64;
const NEEDLE: &[u32] = &[b'E' as u32, b'R' as u32, b'R' as u32, b'O' as u32, b'R' as u32];

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
struct LogicalLine {
    id: u64,
    atoms: Vec<Atom>,
    hard_break: bool,
}

#[derive(Clone, Copy, Debug)]
struct Anchor {
    line_id: u64,
    atom_offset: usize,
}

#[derive(Debug)]
struct Row {
    line_id: u64,
    start_atom: usize,
    atoms: Vec<Atom>,
    hard_break: bool,
}

#[derive(Debug)]
struct RowStore {
    rows: VecDeque<Row>,
    max_atoms: usize,
    retained_atoms: usize,
    allocation_events: usize,
}

impl RowStore {
    fn new(max_atoms: usize) -> Self {
        Self {
            rows: VecDeque::new(),
            max_atoms,
            retained_atoms: 0,
            allocation_events: 0,
        }
    }

    fn append(&mut self, line: LogicalLine) {
        let line_id = line.id;
        let hard_break = line.hard_break;
        let mut current = Vec::new();
        let mut start_atom = 0usize;
        let mut col = 0usize;
        let total_atoms = line.atoms.len();

        for (index, atom) in line.atoms.into_iter().enumerate() {
            let width = usize::from(atom.width.max(1));
            if !current.is_empty() && col + width > INITIAL_WIDTH {
                self.push_row(Row {
                    line_id,
                    start_atom,
                    atoms: std::mem::take(&mut current),
                    hard_break: false,
                });
                start_atom = index;
                col = 0;
            }
            current.push(atom);
            col += width;
            if col == INITIAL_WIDTH {
                self.push_row(Row {
                    line_id,
                    start_atom,
                    atoms: std::mem::take(&mut current),
                    hard_break: false,
                });
                start_atom = index + 1;
                col = 0;
            }
        }

        if !current.is_empty() || total_atoms == 0 {
            self.push_row(Row {
                line_id,
                start_atom,
                atoms: current,
                hard_break,
            });
        } else if let Some(last) = self.rows.back_mut()
            && last.line_id == line_id
        {
            last.hard_break = hard_break;
        }

        self.evict_whole_lines();
    }

    fn push_row(&mut self, row: Row) {
        self.retained_atoms += row.atoms.len();
        self.allocation_events += 1;
        self.rows.push_back(row);
    }

    fn evict_whole_lines(&mut self) {
        while self.retained_atoms > self.max_atoms {
            let Some(first_id) = self.rows.front().map(|row| row.line_id) else {
                break;
            };
            while self.rows.front().is_some_and(|row| row.line_id == first_id) {
                if let Some(row) = self.rows.pop_front() {
                    self.retained_atoms = self.retained_atoms.saturating_sub(row.atoms.len());
                }
            }
        }
    }

    fn retained_lines(&self) -> usize {
        let mut count = 0usize;
        let mut previous = None;
        for row in &self.rows {
            if previous != Some(row.line_id) {
                count += 1;
                previous = Some(row.line_id);
            }
        }
        count
    }

    fn reflow_rows(&self, width: usize) -> usize {
        let mut visual_rows = 0usize;
        let mut current_id = None;
        let mut col = 0usize;
        let mut row_started = false;

        for row in &self.rows {
            if current_id != Some(row.line_id) {
                if current_id.is_some() && !row_started && col == 0 {
                    // The prior logical line ended exactly at the viewport edge.
                }
                current_id = Some(row.line_id);
                col = 0;
                row_started = false;
            }
            if row.atoms.is_empty() && !row_started {
                visual_rows += 1;
                row_started = true;
            }
            for atom in &row.atoms {
                let atom_width = usize::from(atom.width.max(1));
                if !row_started {
                    visual_rows += 1;
                    row_started = true;
                }
                if col != 0 && col + atom_width > width {
                    visual_rows += 1;
                    col = 0;
                }
                col += atom_width;
                if col == width {
                    col = 0;
                    row_started = false;
                }
            }
        }
        visual_rows
    }

    fn search(&self, needle: &[u32]) -> usize {
        let mut hits = 0usize;
        let mut matched = 0usize;
        let mut previous_id = None;
        for row in &self.rows {
            if previous_id != Some(row.line_id) {
                matched = 0;
                previous_id = Some(row.line_id);
            }
            for atom in &row.atoms {
                if atom.scalar == needle[matched] {
                    matched += 1;
                    if matched == needle.len() {
                        hits += 1;
                        matched = 0;
                    }
                } else {
                    matched = usize::from(atom.scalar == needle[0]);
                }
            }
        }
        hits
    }

    fn resolve(&self, anchor: Anchor) -> bool {
        self.rows.iter().any(|row| {
            row.line_id == anchor.line_id
                && anchor.atom_offset >= row.start_atom
                && anchor.atom_offset < row.start_atom + row.atoms.len()
        })
    }

    fn serialize(&self, rle_spaces: bool) -> Vec<u8> {
        let mut output = Vec::with_capacity(self.retained_atoms.saturating_mul(12));
        let mut current_id = None;
        for row in &self.rows {
            if current_id != Some(row.line_id) {
                current_id = Some(row.line_id);
                encode_line_header(&mut output, row.line_id, row.hard_break);
            }
            encode_atoms(&mut output, &row.atoms, rle_spaces);
        }
        output
    }
}

#[derive(Debug)]
struct RingStore {
    lines: VecDeque<LogicalLine>,
    max_atoms: usize,
    retained_atoms: usize,
    allocation_events: usize,
}

impl RingStore {
    fn new(max_atoms: usize) -> Self {
        Self {
            lines: VecDeque::new(),
            max_atoms,
            retained_atoms: 0,
            allocation_events: 0,
        }
    }

    fn append(&mut self, line: LogicalLine) {
        self.retained_atoms += line.atoms.len();
        self.allocation_events += 1;
        self.lines.push_back(line);
        while self.retained_atoms > self.max_atoms {
            let Some(line) = self.lines.pop_front() else {
                break;
            };
            self.retained_atoms = self.retained_atoms.saturating_sub(line.atoms.len());
        }
    }

    fn retained_lines(&self) -> usize {
        self.lines.len()
    }

    fn reflow_rows(&self, width: usize) -> usize {
        self.lines
            .iter()
            .map(|line| wrapped_rows(&line.atoms, width))
            .sum()
    }

    fn search(&self, needle: &[u32]) -> usize {
        self.lines
            .iter()
            .map(|line| search_atoms(&line.atoms, needle))
            .sum()
    }

    fn resolve(&self, anchor: Anchor) -> bool {
        self.lines
            .iter()
            .find(|line| line.id == anchor.line_id)
            .is_some_and(|line| anchor.atom_offset < line.atoms.len())
    }

    fn serialize(&self, rle_spaces: bool) -> Vec<u8> {
        let mut output = Vec::with_capacity(self.retained_atoms.saturating_mul(12));
        for line in &self.lines {
            encode_line_header(&mut output, line.id, line.hard_break);
            encode_atoms(&mut output, &line.atoms, rle_spaces);
        }
        output
    }
}

#[derive(Clone, Copy, Debug)]
struct LineMeta {
    id: u64,
    start: u32,
    len: u32,
    hard_break: bool,
}

#[derive(Debug)]
struct Segment {
    lines: Box<[LineMeta]>,
    atoms: Box<[Atom]>,
    min_id: u64,
    max_id: u64,
}

#[derive(Debug)]
struct SegmentBuilder {
    lines: Vec<LineMeta>,
    atoms: Vec<Atom>,
}

impl SegmentBuilder {
    fn new() -> Self {
        Self {
            lines: Vec::with_capacity(SEGMENT_LINES),
            atoms: Vec::with_capacity(SEGMENT_LINES * 128),
        }
    }

    fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}

#[derive(Debug)]
struct SegmentedStore {
    segments: VecDeque<Segment>,
    tail: SegmentBuilder,
    max_atoms: usize,
    retained_atoms: usize,
    allocation_events: usize,
}

impl SegmentedStore {
    fn new(max_atoms: usize) -> Self {
        Self {
            segments: VecDeque::new(),
            tail: SegmentBuilder::new(),
            max_atoms,
            retained_atoms: 0,
            allocation_events: 2,
        }
    }

    fn append(&mut self, line: LogicalLine) {
        if self.tail.lines.len() == SEGMENT_LINES {
            self.seal_tail();
        }
        let start = self.tail.atoms.len();
        let len = line.atoms.len();
        self.tail.lines.push(LineMeta {
            id: line.id,
            start: u32::try_from(start).expect("spike segment atom offset fits u32"),
            len: u32::try_from(len).expect("spike line length fits u32"),
            hard_break: line.hard_break,
        });
        self.tail.atoms.extend(line.atoms);
        self.retained_atoms += len;
    }

    fn finish(&mut self) {
        self.seal_tail();
        self.evict_segments();
    }

    fn seal_tail(&mut self) {
        if self.tail.is_empty() {
            return;
        }
        let replacement = SegmentBuilder::new();
        self.allocation_events += 2;
        let old = std::mem::replace(&mut self.tail, replacement);
        let min_id = old.lines.first().expect("non-empty segment").id;
        let max_id = old.lines.last().expect("non-empty segment").id;
        self.segments.push_back(Segment {
            lines: old.lines.into_boxed_slice(),
            atoms: old.atoms.into_boxed_slice(),
            min_id,
            max_id,
        });
        self.evict_segments();
    }

    fn evict_segments(&mut self) {
        while self.retained_atoms > self.max_atoms && self.segments.len() > 1 {
            if let Some(segment) = self.segments.pop_front() {
                self.retained_atoms = self.retained_atoms.saturating_sub(segment.atoms.len());
            }
        }
    }

    fn retained_lines(&self) -> usize {
        self.segments.iter().map(|segment| segment.lines.len()).sum()
    }

    fn reflow_rows(&self, width: usize) -> usize {
        let mut rows = 0usize;
        for segment in &self.segments {
            for meta in &segment.lines {
                let start = meta.start as usize;
                let end = start + meta.len as usize;
                rows += wrapped_rows(&segment.atoms[start..end], width);
            }
        }
        rows
    }

    fn search(&self, needle: &[u32]) -> usize {
        let mut hits = 0usize;
        for segment in &self.segments {
            for meta in &segment.lines {
                let start = meta.start as usize;
                let end = start + meta.len as usize;
                hits += search_atoms(&segment.atoms[start..end], needle);
            }
        }
        hits
    }

    fn resolve(&self, anchor: Anchor) -> bool {
        let mut low = 0usize;
        let mut high = self.segments.len();
        while low < high {
            let mid = (low + high) / 2;
            let segment = &self.segments[mid];
            if anchor.line_id < segment.min_id {
                high = mid;
            } else if anchor.line_id > segment.max_id {
                low = mid + 1;
            } else {
                return segment
                    .lines
                    .iter()
                    .find(|meta| meta.id == anchor.line_id)
                    .is_some_and(|meta| anchor.atom_offset < meta.len as usize);
            }
        }
        false
    }

    fn serialize(&self, rle_spaces: bool) -> Vec<u8> {
        let mut output = Vec::with_capacity(self.retained_atoms.saturating_mul(12));
        for segment in &self.segments {
            for meta in &segment.lines {
                encode_line_header(&mut output, meta.id, meta.hard_break);
                let start = meta.start as usize;
                let end = start + meta.len as usize;
                encode_atoms(&mut output, &segment.atoms[start..end], rle_spaces);
            }
        }
        output
    }
}

#[derive(Debug)]
enum Store {
    Rows(RowStore),
    Ring(RingStore),
    Segmented(SegmentedStore),
}

impl Store {
    fn new(candidate: &str, max_atoms: usize) -> Self {
        match candidate {
            "rows" => Self::Rows(RowStore::new(max_atoms)),
            "ring" => Self::Ring(RingStore::new(max_atoms)),
            "segmented" => Self::Segmented(SegmentedStore::new(max_atoms)),
            other => panic!("unknown candidate {other}"),
        }
    }

    fn append(&mut self, line: LogicalLine) {
        match self {
            Self::Rows(store) => store.append(line),
            Self::Ring(store) => store.append(line),
            Self::Segmented(store) => store.append(line),
        }
    }

    fn finish(&mut self) {
        if let Self::Segmented(store) = self {
            store.finish();
        }
    }

    fn retained_atoms(&self) -> usize {
        match self {
            Self::Rows(store) => store.retained_atoms,
            Self::Ring(store) => store.retained_atoms,
            Self::Segmented(store) => store.retained_atoms,
        }
    }

    fn retained_lines(&self) -> usize {
        match self {
            Self::Rows(store) => store.retained_lines(),
            Self::Ring(store) => store.retained_lines(),
            Self::Segmented(store) => store.retained_lines(),
        }
    }

    fn allocation_events(&self) -> usize {
        match self {
            Self::Rows(store) => store.allocation_events,
            Self::Ring(store) => store.allocation_events,
            Self::Segmented(store) => store.allocation_events,
        }
    }

    fn reflow_rows(&self, width: usize) -> usize {
        match self {
            Self::Rows(store) => store.reflow_rows(width),
            Self::Ring(store) => store.reflow_rows(width),
            Self::Segmented(store) => store.reflow_rows(width),
        }
    }

    fn search(&self, needle: &[u32]) -> usize {
        match self {
            Self::Rows(store) => store.search(needle),
            Self::Ring(store) => store.search(needle),
            Self::Segmented(store) => store.search(needle),
        }
    }

    fn resolve(&self, anchor: Anchor) -> bool {
        match self {
            Self::Rows(store) => store.resolve(anchor),
            Self::Ring(store) => store.resolve(anchor),
            Self::Segmented(store) => store.resolve(anchor),
        }
    }

    fn serialize(&self, rle_spaces: bool) -> Vec<u8> {
        match self {
            Self::Rows(store) => store.serialize(rle_spaces),
            Self::Ring(store) => store.serialize(rle_spaces),
            Self::Segmented(store) => store.serialize(rle_spaces),
        }
    }
}

fn wrapped_rows(atoms: &[Atom], width: usize) -> usize {
    if atoms.is_empty() {
        return 1;
    }
    let mut rows = 0usize;
    let mut col = 0usize;
    let mut row_started = false;
    for atom in atoms {
        let atom_width = usize::from(atom.width.max(1));
        if !row_started {
            rows += 1;
            row_started = true;
        }
        if col != 0 && col + atom_width > width {
            rows += 1;
            col = 0;
        }
        col += atom_width;
        if col == width {
            col = 0;
            row_started = false;
        }
    }
    rows
}

fn search_atoms(atoms: &[Atom], needle: &[u32]) -> usize {
    let mut hits = 0usize;
    let mut matched = 0usize;
    for atom in atoms {
        if atom.scalar == needle[matched] {
            matched += 1;
            if matched == needle.len() {
                hits += 1;
                matched = 0;
            }
        } else {
            matched = usize::from(atom.scalar == needle[0]);
        }
    }
    hits
}

fn encode_line_header(output: &mut Vec<u8>, id: u64, hard_break: bool) {
    output.extend_from_slice(&id.to_le_bytes());
    output.push(u8::from(hard_break));
}

fn encode_atoms(output: &mut Vec<u8>, atoms: &[Atom], rle_spaces: bool) {
    let mut index = 0usize;
    while index < atoms.len() {
        let atom = atoms[index];
        if rle_spaces && atom.scalar == b' ' as u32 && atom.combining == 0 {
            let mut run = 1usize;
            while index + run < atoms.len()
                && atoms[index + run].scalar == atom.scalar
                && atoms[index + run].style == atom.style
                && atoms[index + run].width == atom.width
                && atoms[index + run].combining == 0
                && run < u32::MAX as usize
            {
                run += 1;
            }
            output.push(0);
            output.extend_from_slice(&(run as u32).to_le_bytes());
            output.extend_from_slice(&atom.style.to_le_bytes());
            output.push(atom.width);
            index += run;
            continue;
        }
        output.push(1);
        output.extend_from_slice(&atom.scalar.to_le_bytes());
        output.extend_from_slice(&atom.style.to_le_bytes());
        output.push(atom.width);
        output.push(atom.flags);
        output.extend_from_slice(&atom.combining.to_le_bytes());
        index += 1;
    }
}

fn make_line(id: u64, ordinal: usize) -> LogicalLine {
    let base_len = if ordinal % 257 == 0 {
        INITIAL_WIDTH * 8
    } else {
        64 + (ordinal % 97)
    };
    let marker = ordinal % 997 == 0;
    let mut atoms = Vec::with_capacity(base_len + usize::from(marker) * NEEDLE.len());
    if marker {
        for scalar in NEEDLE {
            atoms.push(Atom {
                scalar: *scalar,
                style: 3,
                width: 1,
                flags: 0,
                combining: 0,
            });
        }
    }
    for index in 0..base_len {
        let (scalar, width, combining, flags) = if index % 53 == 0 {
            ('界' as u32, 2, 0, 0)
        } else if index % 47 == 0 {
            ('e' as u32, 1, 1, 1)
        } else if index % 7 == 0 || index % 7 == 1 {
            (b' ' as u32, 1, 0, 0)
        } else {
            ((b'a' + (index % 26) as u8) as u32, 1, 0, 0)
        };
        atoms.push(Atom {
            scalar,
            style: (ordinal % 8) as u32,
            width,
            flags,
            combining,
        });
    }
    LogicalLine {
        id,
        atoms,
        hard_break: true,
    }
}

fn next_line_id(current: &mut u64, ordinal: usize) -> u64 {
    if ordinal != 0 && ordinal % 256 == 0 {
        // Simulate IDs consumed by an alternate-screen lifetime. Primary
        // history must remain correct even when its IDs are not contiguous.
        *current += 24;
    }
    let id = *current;
    *current += 1;
    id
}

fn rss_kib() -> u64 {
    let Ok(status) = fs::read_to_string("/proc/self/status") else {
        return 0;
    };
    status
        .lines()
        .find_map(|line| {
            let rest = line.strip_prefix("VmRSS:")?;
            rest.split_whitespace().next()?.parse::<u64>().ok()
        })
        .unwrap_or(0)
}

fn percentile(values: &mut [u128], numerator: usize, denominator: usize) -> u128 {
    values.sort_unstable();
    let index = ((values.len().saturating_sub(1)) * numerator) / denominator;
    values[index]
}

fn build_store(candidate: &str, target_atoms: usize, id_seed: u64) -> (Store, Vec<Anchor>) {
    let mut store = Store::new(candidate, target_atoms);
    let mut anchors = Vec::new();
    let mut produced = 0usize;
    let mut ordinal = 0usize;
    let mut next_id = id_seed;
    while produced < target_atoms {
        let id = next_line_id(&mut next_id, ordinal);
        let line = make_line(id, ordinal);
        if ordinal % 128 == 0 && line.atoms.len() > 4 {
            anchors.push(Anchor {
                line_id: id,
                atom_offset: 3,
            });
        }
        produced += line.atoms.len();
        store.append(line);
        ordinal += 1;
    }
    store.finish();
    (store, anchors)
}

fn child_bench(candidate: &str, target_atoms: usize) {
    let baseline_rss = rss_kib();
    let started = Instant::now();
    let (store, anchors) = build_store(candidate, target_atoms, 1);
    let append_ns = started.elapsed().as_nanos();
    let rss_delta = rss_kib().saturating_sub(baseline_rss);

    assert!(store.retained_atoms() <= target_atoms + SEGMENT_LINES * INITIAL_WIDTH * 8);
    let search_hits = store.search(NEEDLE);
    assert!(search_hits > 0, "search fixture must remain discoverable");

    let surviving_anchors: Vec<_> = anchors
        .iter()
        .copied()
        .filter(|anchor| store.resolve(*anchor))
        .collect();
    assert!(!surviving_anchors.is_empty(), "selection anchors must survive retention");
    let deliberately_missing = Anchor {
        line_id: surviving_anchors[0].line_id + 1_000_000_000,
        atom_offset: 0,
    };
    assert!(!store.resolve(deliberately_missing));

    let widths = [40usize, 80, 132, 64, 160, 96, 48];
    let mut reflow_ns = Vec::with_capacity(35);
    for iteration in 0..35 {
        let width = widths[iteration % widths.len()];
        let started = Instant::now();
        black_box(store.reflow_rows(width));
        reflow_ns.push(started.elapsed().as_nanos());
    }
    let reflow_p50 = percentile(&mut reflow_ns.clone(), 50, 100);
    let reflow_p95 = percentile(&mut reflow_ns.clone(), 95, 100);
    let reflow_p99 = percentile(&mut reflow_ns, 99, 100);

    let mut search_ns = Vec::with_capacity(15);
    for _ in 0..15 {
        let started = Instant::now();
        black_box(store.search(NEEDLE));
        search_ns.push(started.elapsed().as_nanos());
    }
    let search_p50 = percentile(&mut search_ns.clone(), 50, 100);
    let search_p95 = percentile(&mut search_ns, 95, 100);

    let started = Instant::now();
    let mut selection_hits = 0usize;
    for _ in 0..100 {
        for anchor in &surviving_anchors {
            selection_hits += usize::from(store.resolve(*anchor));
        }
    }
    let selection_total_ns = started.elapsed().as_nanos();
    let selection_ops = surviving_anchors.len() * 100;
    let selection_ns_per_op = if selection_ops == 0 {
        0
    } else {
        selection_total_ns / selection_ops as u128
    };

    let started = Instant::now();
    let raw = store.serialize(false);
    let raw_ns = started.elapsed().as_nanos();
    let started = Instant::now();
    let rle = store.serialize(true);
    let rle_ns = started.elapsed().as_nanos();

    println!(
        "kind=bench|candidate={candidate}|target_atoms={target_atoms}|retained_atoms={}|retained_lines={}|rss_kib={rss_delta}|append_ns={append_ns}|alloc_events={}|reflow_p50_ns={reflow_p50}|reflow_p95_ns={reflow_p95}|reflow_p99_ns={reflow_p99}|search_p50_ns={search_p50}|search_p95_ns={search_p95}|search_hits={search_hits}|selection_ns_per_op={selection_ns_per_op}|selection_hits={selection_hits}|raw_bytes={}|raw_ns={raw_ns}|rle_bytes={}|rle_ns={rle_ns}",
        store.retained_atoms(),
        store.retained_lines(),
        store.allocation_events(),
        raw.len(),
        rle.len()
    );
}

fn child_population(candidate: &str, executions: usize, atoms_each: usize) {
    let baseline_rss = rss_kib();
    let started = Instant::now();
    let mut stores = Vec::with_capacity(executions);
    for execution in 0..executions {
        let id_seed = 1 + (execution as u64) * 10_000_000;
        let (store, _) = build_store(candidate, atoms_each, id_seed);
        stores.push(store);
    }
    let build_ns = started.elapsed().as_nanos();
    let rss_delta = rss_kib().saturating_sub(baseline_rss);
    let retained_atoms: usize = stores.iter().map(Store::retained_atoms).sum();
    black_box(&stores);
    println!(
        "kind=population|candidate={candidate}|executions={executions}|atoms_each={atoms_each}|retained_atoms={retained_atoms}|rss_kib={rss_delta}|build_ns={build_ns}"
    );
}

fn run_child(args: &[String]) -> String {
    let exe = env::current_exe().expect("current executable");
    let output = Command::new(exe)
        .args(args)
        .output()
        .expect("spawn child benchmark");
    if !output.status.success() {
        panic!(
            "child failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    String::from_utf8(output.stdout)
        .expect("child output is UTF-8")
        .trim()
        .to_owned()
}

fn field<'a>(record: &'a str, key: &str) -> &'a str {
    record
        .split('|')
        .find_map(|part| part.strip_prefix(&format!("{key}=")))
        .unwrap_or("")
}

fn nanos_to_us(value: &str) -> f64 {
    value.parse::<f64>().unwrap_or(0.0) / 1_000.0
}

fn mib_per_second(bytes: &str, nanos: &str) -> f64 {
    let bytes = bytes.parse::<f64>().unwrap_or(0.0);
    let nanos = nanos.parse::<f64>().unwrap_or(1.0).max(1.0);
    (bytes / (1024.0 * 1024.0)) / (nanos / 1_000_000_000.0)
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.get(1).is_some_and(|arg| arg == "--child-bench") {
        let candidate = &args[2];
        let atoms = args[3].parse::<usize>().expect("target atoms");
        child_bench(candidate, atoms);
        return;
    }
    if args.get(1).is_some_and(|arg| arg == "--child-population") {
        let candidate = &args[2];
        let executions = args[3].parse::<usize>().expect("execution count");
        let atoms_each = args[4].parse::<usize>().expect("atoms per execution");
        child_population(candidate, executions, atoms_each);
        return;
    }

    let candidates = ["rows", "ring", "segmented"];
    let sizes = [10_000usize, 100_000, 1_000_000];
    let populations = [1usize, 10, 50, 100];
    let mut benches = Vec::new();
    let mut population_rows = Vec::new();

    for candidate in candidates {
        for size in sizes {
            benches.push(run_child(&[
                "--child-bench".into(),
                candidate.into(),
                size.to_string(),
            ]));
        }
        for executions in populations {
            population_rows.push(run_child(&[
                "--child-population".into(),
                candidate.into(),
                executions.to_string(),
                "10000".into(),
            ]));
        }
    }

    let rustc = Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_owned())
        .unwrap_or_else(|| "unknown".into());
    let uname = Command::new("uname")
        .arg("-a")
        .output()
        .ok()
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_owned())
        .unwrap_or_else(|| "unknown".into());

    let mut report = String::new();
    report.push_str("# Issue #685 scrollback/history spike measurements\n\n");
    report.push_str("This is experimental evidence only. The prototype is isolated and is not production code.\n\n");
    report.push_str(&format!("- Rust: `{rustc}`\n- Runner: `{uname}`\n- Atom model: compact scalar/style/width/combining metadata; wide and combining fixtures included.\n- IDs deliberately contain gaps to model alternate-screen allocation from the single TerminalState LineId allocator.\n- Sizes are retained logical atoms/cells; 10k/100k/1M cases are measured in fresh child processes for RSS isolation.\n\n"));
    report.push_str("## Candidates\n\n");
    report.push_str("1. `rows`: visual-row snapshots with repeated logical LineId/start offsets; whole logical lines are evicted together. This approximates extending the current row-oriented history direction.\n2. `ring`: one variable-length allocation per logical line in a bounded VecDeque.\n3. `segmented`: compact immutable 64-line segments (line metadata + contiguous atom arena) plus a mutable tail, with segment-granularity eviction.\n\n");
    report.push_str("## Core measurements\n\n");
    report.push_str("| candidate | target atoms | retained atoms | lines | RSS KiB | append ms | repr alloc events | reflow p50 µs | p95 µs | p99 µs | search p50 µs | p95 µs | selection ns/op | raw MiB/s | space-RLE ratio | RLE MiB/s |\n");
    report.push_str("|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|\n");
    for row in &benches {
        let raw_bytes = field(row, "raw_bytes").parse::<f64>().unwrap_or(0.0);
        let rle_bytes = field(row, "rle_bytes").parse::<f64>().unwrap_or(0.0);
        let ratio = if raw_bytes == 0.0 { 1.0 } else { rle_bytes / raw_bytes };
        report.push_str(&format!(
            "| {} | {} | {} | {} | {} | {:.2} | {} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {} | {:.1} | {:.3} | {:.1} |\n",
            field(row, "candidate"),
            field(row, "target_atoms"),
            field(row, "retained_atoms"),
            field(row, "retained_lines"),
            field(row, "rss_kib"),
            nanos_to_us(field(row, "append_ns")) / 1000.0,
            field(row, "alloc_events"),
            nanos_to_us(field(row, "reflow_p50_ns")),
            nanos_to_us(field(row, "reflow_p95_ns")),
            nanos_to_us(field(row, "reflow_p99_ns")),
            nanos_to_us(field(row, "search_p50_ns")),
            nanos_to_us(field(row, "search_p95_ns")),
            field(row, "selection_ns_per_op"),
            mib_per_second(field(row, "raw_bytes"), field(row, "raw_ns")),
            ratio,
            mib_per_second(field(row, "rle_bytes"), field(row, "rle_ns")),
        ));
    }

    report.push_str("\n## Hidden/detached execution population\n\n");
    report.push_str("Each execution retains approximately 10k logical atoms.\n\n");
    report.push_str("| candidate | executions | retained atoms | RSS KiB | build ms |\n");
    report.push_str("|---|---:|---:|---:|---:|\n");
    for row in &population_rows {
        report.push_str(&format!(
            "| {} | {} | {} | {} | {:.2} |\n",
            field(row, "candidate"),
            field(row, "executions"),
            field(row, "retained_atoms"),
            field(row, "rss_kib"),
            nanos_to_us(field(row, "build_ns")) / 1000.0,
        ));
    }

    report.push_str("\n## Fixture coverage exercised by the harness\n\n");
    report.push_str("- long logical lines wrapping across eight 80-column rows;\n- wide cells and combining/grapheme-side metadata;\n- non-contiguous LineIds caused by simulated alternate-screen lifetimes;\n- repeated 40/48/64/80/96/132/160-column resize/reflow oscillation;\n- selection anchors as `(LineId, atom offset)` across reflow;\n- search markers retained in logical history;\n- one-million-atom high-volume history;\n- bounded eviction only at whole-logical-line or whole-segment boundaries;\n- 1/10/50/100 hidden execution populations;\n- raw serialization and a deliberately simple space-RLE persistence experiment.\n\n");
    report.push_str("## Interpretation guardrail\n\nRSS includes allocator/runtime effects on the GitHub Linux runner and the atom model is intentionally smaller than the future full Unicode/style payload. Absolute values are not production budgets. Relative slopes, allocation structure, reflow behavior and anchor correctness are the decision evidence. Production acceptance still needs macOS measurements against the final Rust implementation and real retained VT fixtures.\n");

    fs::write("spike-report.md", report).expect("write spike report");
    println!("wrote spike-report.md");
}
