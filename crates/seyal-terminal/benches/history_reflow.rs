use std::{env, hint::black_box, time::Instant};

use seyal_terminal::TerminalState;

const DEFAULT_LINES: usize = 100_000;
const SAMPLES: usize = 32;

fn main() {
    let lines = env::var("SEYAL_HISTORY_BENCH_LINES")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_LINES);
    let mut terminal = TerminalState::new(120, 40).expect("valid benchmark geometry");
    for index in 0..lines {
        let line = format!("history-line-{index:08}\r\n");
        terminal
            .feed(line.as_bytes())
            .expect("history feed succeeds");
    }
    let _ = terminal.take_damage();

    let mut samples = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let started = Instant::now();
        let rows = terminal.primary_history_reflow(80, 200);
        black_box(rows);
        samples.push(started.elapsed().as_nanos());
    }
    samples.sort_unstable();
    let nearest_rank = |percent: usize| {
        let index = ((SAMPLES * percent).saturating_add(99) / 100)
            .saturating_sub(1)
            .min(SAMPLES - 1);
        samples[index]
    };
    println!("[seyal history benchmark] dimensions=120x40");
    println!("[seyal history benchmark] retained_source_lines={lines}");
    println!("[seyal history benchmark] reflow_columns=80");
    println!(
        "[seyal history benchmark] resident_bytes={}",
        terminal.primary_history_resident_bytes()
    );
    println!("[seyal history benchmark] p50_ns={}", nearest_rank(50));
    println!("[seyal history benchmark] p95_ns={}", nearest_rank(95));
    println!("[seyal history benchmark] p99_ns={}", nearest_rank(99));
    println!("[seyal history benchmark] percentile_method=nearest-rank");
    println!("[seyal history benchmark] performance_claim=false");
}
