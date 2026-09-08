use std::{env, hint::black_box, process::Command, time::Instant};

use seyal_terminal::TerminalState;

const DEFAULT_LINES: &[usize] = &[100_000];
const DEFAULT_EXECUTIONS: &[usize] = &[1];
const DEFAULT_COLUMNS: &[u16] = &[80];
const DEFAULT_WORKLOADS: &[&str] = &["ascii"];
const FULL_LINES: &[usize] = &[10_000, 100_000, 1_000_000];
const FULL_EXECUTIONS: &[usize] = &[1, 10, 50, 100];
const FULL_COLUMNS: &[u16] = &[40, 48, 64, 80, 96, 132, 160];
const FULL_WORKLOADS: &[&str] = &["ascii", "styled", "cjk", "emoji-combining"];
const DEFAULT_SAMPLES: usize = 32;
const ACTIVE_WINDOW_ROWS: usize = 120;

fn parse_scales<T>(name: &str, defaults: &[T]) -> Vec<T>
where
    T: Copy + From<u8> + PartialOrd + std::str::FromStr,
{
    env::var(name)
        .ok()
        .map(|value| {
            value
                .split(',')
                .filter_map(|part| part.trim().parse().ok())
                .filter(|value: &T| *value > T::from(0))
                .collect::<Vec<T>>()
        })
        .filter(|values| !values.is_empty())
        .unwrap_or_else(|| defaults.to_vec())
}

fn full_matrix_enabled() -> bool {
    env::var("SEYAL_HISTORY_BENCH_FULL").as_deref() == Ok("1")
}

fn parse_samples() -> usize {
    env::var("SEYAL_HISTORY_BENCH_SAMPLES")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|value: &usize| *value > 0)
        .unwrap_or(DEFAULT_SAMPLES)
}

fn workload_names() -> Vec<&'static str> {
    let defaults = if full_matrix_enabled() {
        FULL_WORKLOADS
    } else {
        DEFAULT_WORKLOADS
    };
    env::var("SEYAL_HISTORY_BENCH_WORKLOADS")
        .ok()
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| match value {
                    "ascii" => "ascii",
                    "styled" => "styled",
                    "cjk" => "cjk",
                    "emoji-combining" => "emoji-combining",
                    other => panic!("unknown history benchmark workload {other:?}"),
                })
                .collect()
        })
        .filter(|values: &Vec<&str>| !values.is_empty())
        .unwrap_or_else(|| defaults.to_vec())
}

fn line_for(workload: &str, execution: usize) -> (Vec<u8>, String) {
    let marker = format!("needle-{execution:03}");
    let line = match workload {
        "ascii" => format!("ascii-{marker}-history\r\n"),
        "styled" => format!("\x1b[31mstyled-{marker}-history\x1b[0m\r\n"),
        "cjk" => format!("cjk-界-{marker}-界\r\n"),
        "emoji-combining" => format!("emoji-👨‍👩‍👧‍👦-e\u{301}-{marker}\r\n"),
        _ => unreachable!("workload names are validated by workload_names"),
    };
    (line.into_bytes(), marker)
}

fn percentile(samples: &mut [u128], percent: usize) -> u128 {
    assert!(
        !samples.is_empty(),
        "benchmark must collect at least one sample"
    );
    samples.sort_unstable();
    let index = ((samples.len() * percent).saturating_add(99) / 100)
        .saturating_sub(1)
        .min(samples.len() - 1);
    samples[index]
}

fn process_rss_kib() -> Option<u64> {
    let output = Command::new("ps")
        .args(["-p", &std::process::id().to_string(), "-o", "rss="])
        .output()
        .ok()?;
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .next()?
        .parse()
        .ok()
}

fn benchmark_commit() -> String {
    env::var("SEYAL_BENCH_COMMIT")
        .or_else(|_| env::var("GITHUB_SHA"))
        .unwrap_or_else(|_| "unknown".to_owned())
}

fn measure(
    lines: usize,
    executions: usize,
    columns: u16,
    workload: &str,
    samples: usize,
    commit: &str,
) {
    let rss_before = process_rss_kib();
    let append_samples_per_execution = samples.min(lines);
    let mut append_samples = Vec::with_capacity(executions * append_samples_per_execution);
    let mut terminals = Vec::with_capacity(executions);

    for execution in 0..executions {
        let (line, needle) = line_for(workload, execution);
        let mut terminal = TerminalState::new(120, 40).expect("valid benchmark geometry");
        let base_lines_per_sample = lines / append_samples_per_execution;
        let remainder = lines % append_samples_per_execution;
        for sample in 0..append_samples_per_execution {
            let lines_for_sample = base_lines_per_sample + usize::from(sample < remainder);
            let started = Instant::now();
            for _ in 0..lines_for_sample {
                terminal.feed(&line).expect("history feed succeeds");
            }
            append_samples.push(started.elapsed().as_nanos());
        }
        let _ = terminal.take_damage();
        terminals.push((terminal, needle));
    }

    let mut reflow_samples = Vec::with_capacity(executions * samples);
    for _ in 0..samples {
        for (terminal, _) in &mut terminals {
            terminal.drop_primary_history_derived_cache();
            let started = Instant::now();
            black_box(terminal.primary_history_reflow(columns, ACTIVE_WINDOW_ROWS));
            reflow_samples.push(started.elapsed().as_nanos());
        }
    }

    let mut search_samples = Vec::with_capacity(executions * samples);
    let mut anchor_samples = Vec::with_capacity(executions * samples);
    let mut resolved_anchors = 0usize;
    for _ in 0..samples {
        for (terminal, needle) in &terminals {
            let started = Instant::now();
            let matches = terminal.primary_history_search(needle, 1);
            search_samples.push(started.elapsed().as_nanos());
            let Some(found) = matches.first() else {
                anchor_samples.push(0);
                continue;
            };
            let started = Instant::now();
            black_box(terminal.primary_history_unit(found.start));
            anchor_samples.push(started.elapsed().as_nanos());
            resolved_anchors = resolved_anchors.saturating_add(1);
        }
    }

    let resident_history_bytes: usize = terminals
        .iter()
        .map(|(terminal, _)| terminal.primary_history_resident_bytes())
        .sum();
    let derived_cache_bytes: usize = terminals
        .iter()
        .map(|(terminal, _)| terminal.primary_history_derived_cache_bytes())
        .sum();
    let rss_after = process_rss_kib();
    let rss_before_value = rss_before.unwrap_or(0);
    let rss_after_value = rss_after.unwrap_or(0);
    let rss_delta = rss_after_value.saturating_sub(rss_before_value);

    println!(
        "[seyal history benchmark] case workload={workload} lines={lines} executions={executions} columns={columns} commit={commit} resident_history_bytes={resident_history_bytes} derived_cache_bytes={derived_cache_bytes} rss_before_kib={rss_before_value} rss_after_kib={rss_after_value} rss_delta_kib={rss_delta} rss_available={} append_observations={} append_samples_per_execution={append_samples_per_execution} append_p50_ns={} append_p95_ns={} append_p99_ns={} reflow_p50_ns={} reflow_p95_ns={} reflow_p99_ns={} search_p50_ns={} search_p95_ns={} search_p99_ns={} anchor_p50_ns={} anchor_p95_ns={} anchor_p99_ns={} resolved_anchors={resolved_anchors} allocation_calls=not-instrumented allocated_bytes=not-instrumented deallocated_bytes=not-instrumented allocation_status=not-instrumented samples={samples} percentile_method=nearest-rank performance_claim=false evidence_scope=TerminalState-comparative",
        rss_before.is_some(),
        append_samples.len(),
        percentile(&mut append_samples, 50),
        percentile(&mut append_samples, 95),
        percentile(&mut append_samples, 99),
        percentile(&mut reflow_samples, 50),
        percentile(&mut reflow_samples, 95),
        percentile(&mut reflow_samples, 99),
        percentile(&mut search_samples, 50),
        percentile(&mut search_samples, 95),
        percentile(&mut search_samples, 99),
        percentile(&mut anchor_samples, 50),
        percentile(&mut anchor_samples, 95),
        percentile(&mut anchor_samples, 99),
    );
}

fn main() {
    let full = full_matrix_enabled();
    let lines = parse_scales(
        "SEYAL_HISTORY_BENCH_LINES",
        if full { FULL_LINES } else { DEFAULT_LINES },
    );
    let executions = parse_scales(
        "SEYAL_HISTORY_BENCH_EXECUTIONS",
        if full {
            FULL_EXECUTIONS
        } else {
            DEFAULT_EXECUTIONS
        },
    );
    let columns = parse_scales(
        "SEYAL_HISTORY_BENCH_COLUMNS",
        if full { FULL_COLUMNS } else { DEFAULT_COLUMNS },
    );
    let workloads = workload_names();
    let samples = parse_samples();
    let commit = benchmark_commit();

    println!(
        "[seyal history benchmark] build_mode={}",
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
    );
    println!("[seyal history benchmark] target_os={}", env::consts::OS);
    println!(
        "[seyal history benchmark] target_arch={}",
        env::consts::ARCH
    );
    println!("[seyal history benchmark] commit={commit}");
    println!("[seyal history benchmark] full_matrix={full}");
    println!("[seyal history benchmark] percentile_method=nearest-rank");
    println!("[seyal history benchmark] performance_claim=false");
    println!("[seyal history benchmark] evidence_scope=TerminalState-comparative");

    for workload in workloads {
        for &execution_count in &executions {
            for &line_count in &lines {
                for &column_count in &columns {
                    measure(
                        line_count,
                        execution_count,
                        column_count,
                        workload,
                        samples,
                        &commit,
                    );
                }
            }
        }
    }
}
