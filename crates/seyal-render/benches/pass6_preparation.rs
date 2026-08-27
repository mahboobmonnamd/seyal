use std::{hint::black_box, time::Instant};

use seyal_render::{CommittedDisplay, CursorState, PreparedSurface, RenderCell, RowDamage};

const ROWS: u16 = 40;
const COLUMNS: u16 = 120;
const REPS: usize = 2_000;

fn main() {
    println!(
        "pass6_preparation performance_claim=false boundary=committed_display_to_prepared_surface"
    );
    println!(
        "geometry={}x{} repetitions={} percentile_method=nearest_rank commit_sha={} os={} arch={}",
        COLUMNS,
        ROWS,
        REPS,
        std::env::var("GITHUB_SHA").unwrap_or_else(|_| "unknown".to_owned()),
        std::env::consts::OS,
        std::env::consts::ARCH
    );

    let mut cells = vec![RenderCell::default(); ROWS as usize * COLUMNS as usize];
    let mut surface = PreparedSurface::default();
    surface
        .prepare(
            CommittedDisplay {
                generation: 1,
                rows: ROWS,
                columns: COLUMNS,
                cursor: CursorState::new(0, 0, true),
                alternate_screen: false,
                cells: cells.as_slice(),
            },
            RowDamage::full(ROWS),
            true,
        )
        .expect("initial preparation");

    let mut sparse = Vec::with_capacity(REPS);
    for iteration in 0..REPS {
        let row = (iteration % ROWS as usize) as u16;
        let cell_index = row as usize * COLUMNS as usize;
        cells[cell_index].scalar = char::from_u32(b'a' as u32 + (iteration % 26) as u32)
            .expect("ASCII scalar");
        let started = Instant::now();
        let result = surface
            .prepare(
                CommittedDisplay {
                    generation: iteration as u64 + 2,
                    rows: ROWS,
                    columns: COLUMNS,
                    cursor: CursorState::new(row, 0, true),
                    alternate_screen: false,
                    cells: cells.as_slice(),
                },
                RowDamage::from_range(row, 1).expect("row damage"),
                false,
            )
            .expect("sparse preparation");
        black_box(result);
        sparse.push(started.elapsed().as_nanos());
    }
    report("one_row_plus_cursor", &mut sparse);

    let mut full = Vec::with_capacity(REPS / 10);
    for iteration in 0..REPS / 10 {
        let started = Instant::now();
        let result = surface
            .prepare(
                CommittedDisplay {
                    generation: REPS as u64 + iteration as u64 + 2,
                    rows: ROWS,
                    columns: COLUMNS,
                    cursor: CursorState::new(0, 0, true),
                    alternate_screen: false,
                    cells: cells.as_slice(),
                },
                RowDamage::full(ROWS),
                true,
            )
            .expect("full preparation");
        black_box(result);
        full.push(started.elapsed().as_nanos());
    }
    report("full_120x40", &mut full);
}

fn report(name: &str, samples: &mut [u128]) {
    samples.sort_unstable();
    println!(
        "scenario={name} samples={} p50_ns={} p95_ns={} p99_ns={} max_ns={}",
        samples.len(),
        percentile(samples, 50),
        percentile(samples, 95),
        percentile(samples, 99),
        samples.last().copied().unwrap_or_default()
    );
}

fn percentile(samples: &[u128], percentile: usize) -> u128 {
    if samples.is_empty() {
        return 0;
    }
    let rank = (samples.len() * percentile).div_ceil(100).max(1);
    samples[rank.saturating_sub(1).min(samples.len() - 1)]
}
