use std::{env, hint::black_box, time::Instant};

use seyal_terminal::TerminalState;

const DEFAULT_LINES: &[usize] = &[100_000];
const DEFAULT_EXECUTIONS: &[usize] = &[1];
const SAMPLES: usize = 32;

fn parse_scales(name: &str, defaults: &[usize]) -> Vec<usize> {
    env::var(name)
        .ok()
        .map(|value| {
            value
                .split(',')
                .filter_map(|part| part.trim().parse().ok())
                .filter(|value: &usize| *value > 0)
                .collect()
        })
        .filter(|values: &Vec<usize>| !values.is_empty())
        .unwrap_or_else(|| defaults.to_vec())
}

fn measure(lines: usize, executions: usize) {
    let mut terminals = Vec::with_capacity(executions);
    for execution in 0..executions {
        let mut terminal = TerminalState::new(120, 40).expect("valid benchmark geometry");
        for index in 0..lines {
            let line = format!("history-line-{execution:03}-{index:08}\r\n");
            terminal
                .feed(line.as_bytes())
                .expect("history feed succeeds");
        }
        let _ = terminal.take_damage();
        terminals.push(terminal);
    }

    let mut samples = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let started = Instant::now();
        for terminal in &terminals {
            black_box(terminal.primary_history_reflow(80, 200));
        }
        samples.push(started.elapsed().as_nanos());
    }
    samples.sort_unstable();
    let nearest_rank = |percent: usize| {
        let index = ((SAMPLES * percent).saturating_add(99) / 100)
            .saturating_sub(1)
            .min(SAMPLES - 1);
        samples[index]
    };
    let resident_bytes: usize = terminals
        .iter()
        .map(TerminalState::primary_history_resident_bytes)
        .sum();
    println!("[seyal history benchmark] dimensions=120x40");
    println!("[seyal history benchmark] retained_source_lines={lines}");
    println!("[seyal history benchmark] executions={executions}");
    println!("[seyal history benchmark] reflow_columns=80");
    println!("[seyal history benchmark] resident_bytes={resident_bytes}");
    println!("[seyal history benchmark] p50_ns={}", nearest_rank(50));
    println!("[seyal history benchmark] p95_ns={}", nearest_rank(95));
    println!("[seyal history benchmark] p99_ns={}", nearest_rank(99));
    println!("[seyal history benchmark] percentile_method=nearest-rank");
    println!("[seyal history benchmark] performance_claim=false");
}

fn main() {
    let lines = parse_scales("SEYAL_HISTORY_BENCH_LINES", DEFAULT_LINES);
    let executions = parse_scales("SEYAL_HISTORY_BENCH_EXECUTIONS", DEFAULT_EXECUTIONS);
    for execution_count in executions {
        for line_count in &lines {
            measure(*line_count, execution_count);
        }
    }
}
