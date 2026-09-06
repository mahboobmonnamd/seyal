use seyal_terminal::{Cell, LineId, TerminalState};
use std::{collections::VecDeque, fs};

const FIXTURES: &[(&str, &[u8])] = &[
    ("m001-basic", include_bytes!("../../../tests/fixtures/vt/m001-basic.input")),
    ("m001-deferred-osc", include_bytes!("../../../tests/fixtures/vt/m001-deferred-osc.input")),
    ("m001-ecma48-core", include_bytes!("../../../tests/fixtures/vt/m001-ecma48-core.input")),
    ("m001-ecma48-erase-save", include_bytes!("../../../tests/fixtures/vt/m001-ecma48-erase-save.input")),
    ("m001-xterm-private", include_bytes!("../../../tests/fixtures/vt/m001-xterm-private.input")),
    ("m001-utf8", include_bytes!("../../../tests/fixtures/vt/m001-utf8.input")),
];

#[derive(Clone, Copy)]
struct Meta {
    id: LineId,
    start: u32,
    len: u32,
}

struct Segment {
    metas: Vec<Meta>,
    cells: Vec<Cell>,
}

struct Segmented {
    segments: Vec<Segment>,
    tail_metas: Vec<Meta>,
    tail_cells: Vec<Cell>,
    target_cells: usize,
}

impl Segmented {
    fn new(target_cells: usize) -> Self {
        Self {
            segments: Vec::new(),
            tail_metas: Vec::new(),
            tail_cells: Vec::with_capacity(target_cells),
            target_cells,
        }
    }

    fn append(&mut self, id: LineId, cells: &[Cell]) {
        if !self.tail_cells.is_empty() && self.tail_cells.len() + cells.len() > self.target_cells {
            self.seal();
        }
        let start = self.tail_cells.len();
        self.tail_metas.push(Meta {
            id,
            start: u32::try_from(start).expect("fixture segment offset fits u32"),
            len: u32::try_from(cells.len()).expect("fixture row length fits u32"),
        });
        self.tail_cells.extend_from_slice(cells);
        if self.tail_cells.len() >= self.target_cells {
            self.seal();
        }
    }

    fn seal(&mut self) {
        if self.tail_metas.is_empty() {
            return;
        }
        self.segments.push(Segment {
            metas: std::mem::take(&mut self.tail_metas),
            cells: std::mem::replace(&mut self.tail_cells, Vec::with_capacity(self.target_cells)),
        });
    }

    fn finish(&mut self) {
        self.seal();
    }

    fn reconstruct(&self) -> Vec<(LineId, Vec<Cell>)> {
        let mut rows = Vec::new();
        for segment in &self.segments {
            for meta in &segment.metas {
                let start = meta.start as usize;
                rows.push((meta.id, segment.cells[start..start + meta.len as usize].to_vec()));
            }
        }
        rows
    }
}

fn history(terminal: &TerminalState) -> Vec<(LineId, Vec<Cell>)> {
    let max_id = (0..terminal.rows())
        .filter_map(|row| terminal.line_id(row))
        .max()
        .unwrap_or(LineId(1));
    terminal.primary_history_range(LineId(1), max_id, 8_192)
}

fn retained_rows_unchanged(before: &[(LineId, Vec<Cell>)], after: &[(LineId, Vec<Cell>)]) -> bool {
    before.iter().all(|expected| after.iter().any(|actual| actual == expected))
}

fn main() {
    let mut terminal = TerminalState::new(8, 3).expect("valid fixture terminal");
    for _ in 0..128 {
        for (_, fixture) in FIXTURES {
            terminal.feed(fixture).expect("retained fixture feed succeeds");
            terminal.feed(b"\r\n").expect("fixture separator succeeds");
        }
    }

    let canonical = history(&terminal);
    assert!(canonical.len() > 100, "fixture replay must create retained history");

    let ring: VecDeque<(LineId, Vec<Cell>)> = canonical.iter().cloned().collect();
    assert_eq!(ring.iter().cloned().collect::<Vec<_>>(), canonical);

    let mut segmented = Segmented::new(512);
    for (id, cells) in &canonical {
        segmented.append(*id, cells);
    }
    segmented.finish();
    assert_eq!(segmented.reconstruct(), canonical, "segmented storage must preserve fixture cells and IDs exactly");

    let gap_count = canonical.windows(2).filter(|pair| pair[1].0.0 > pair[0].0.0 + 1).count();
    let cell_count: usize = canonical.iter().map(|(_, cells)| cells.len()).sum();
    let styled_cells = canonical.iter().flat_map(|(_, cells)| cells)
        .filter(|cell| cell.style != Cell::default().style).count();
    let utf8_cells = canonical.iter().flat_map(|(_, cells)| cells)
        .filter(|cell| !cell.character.is_ascii()).count();

    let before_resize = canonical.clone();
    for (cols, rows) in [(40, 6), (5, 2), (32, 4), (8, 3)] {
        terminal.resize(cols, rows).expect("fixture resize succeeds");
    }
    let after_resize = history(&terminal);
    assert!(retained_rows_unchanged(&before_resize, &after_resize), "pre-existing retained rows must keep exact LineId + Cell payload across resize");
    let resize_added_rows = after_resize.len().saturating_sub(before_resize.len());

    terminal.feed(b"\x1b[?1049h").expect("enter alternate screen");
    assert!(terminal.primary_history_range(LineId(1), LineId(u64::MAX), 32).is_empty(), "primary history projection is hidden while alternate screen is active");
    terminal.feed(b"\x1b[?1049l").expect("leave alternate screen");
    assert_eq!(history(&terminal), after_resize, "alternate screen must not contaminate primary retained history");

    let mut report = String::from("# Issue #685 retained VT fixture replay\n\n");
    report.push_str("This check replays Seyal's existing retained M001 VT corpus through the real `TerminalState`, then verifies exact `Cell` + `LineId` preservation through ring and compact segmented candidate storage. Synthetic M002 stress fixtures remain separate because M001 does not yet expose production soft-wrap lineage or #684 grapheme/width semantics.\n\n");
    report.push_str("## Corpus\n\n");
    for (name, _) in FIXTURES {
        report.push_str(&format!("- `{name}`\n"));
    }
    report.push_str(&format!("\n## Result\n\n- Replay rounds: 128\n- Retained primary rows before resize: {}\n- Retained cells before resize: {}\n- `Cell` size on runner: {} bytes\n- Styled retained cells: {}\n- Non-ASCII retained cells: {}\n- Non-contiguous retained `LineId` gaps: {}\n- 512-cell-target immutable segments: {}\n- Ring reconstruction equals canonical fixture projection: yes\n- Segmented reconstruction equals canonical fixture projection: yes\n- Pre-existing retained `(LineId, Cell[])` payload survives resize oscillation unchanged: yes\n- Additional rows legitimately moved into history by row-count shrink: {}\n- Alternate-screen content excluded from primary history: yes\n\n", canonical.len(), cell_count, std::mem::size_of::<Cell>(), styled_cells, utf8_cells, gap_count, segmented.segments.len(), resize_added_rows));
    report.push_str("## Boundary exposed by the replay\n\nThe M001 history API retains visual rows but does not expose whether adjacent rows are connected by soft autowrap. Therefore a production M002 reflow implementation cannot reconstruct logical lines correctly from the current history projection alone. M002 must record hard-break versus soft-wrap lineage in canonical terminal state at mutation time; it must not infer that relationship later from row text.\n");
    fs::write("fixture-replay.md", report).expect("write fixture replay report");
}
