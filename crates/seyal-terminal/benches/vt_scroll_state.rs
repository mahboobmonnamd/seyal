use std::{env, hint::black_box, process::Command, time::Instant};

use seyal_terminal::TerminalState;

const DEFAULT_ITERATIONS: u64 = 10_000;
const CONTENT: &[u8] =
    b"scroll-heavy terminal output payload 0123456789 abcdefghijklmnopqrstuvwxyz";

fn iterations() -> u64 {
    env::var("SEYAL_VT_SCROLL_BENCH_ITERATIONS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_ITERATIONS)
}

fn command_output(program: &str, args: &[&str]) -> String {
    Command::new(program)
        .args(args)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_owned())
}

fn metadata() {
    let build_mode = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let commit = env::var("SEYAL_BENCH_COMMIT")
        .or_else(|_| env::var("GITHUB_SHA"))
        .unwrap_or_else(|_| "unknown".to_owned());
    let shell = env::var("SHELL").unwrap_or_else(|_| "unknown".to_owned());

    println!(
        "[seyal vt scroll benchmark] machine_model={}",
        command_output("sysctl", &["-n", "hw.model"])
    );
    println!(
        "[seyal vt scroll benchmark] chip={}",
        command_output("sysctl", &["-n", "machdep.cpu.brand_string"])
    );
    println!(
        "[seyal vt scroll benchmark] os_version={}",
        command_output("sw_vers", &["-productVersion"])
    );
    println!("[seyal vt scroll benchmark] target_os={}", env::consts::OS);
    println!(
        "[seyal vt scroll benchmark] target_arch={}",
        env::consts::ARCH
    );
    println!("[seyal vt scroll benchmark] build_mode={build_mode}");
    println!("[seyal vt scroll benchmark] commit={commit}");
    println!("[seyal vt scroll benchmark] shell={shell}");
    println!("[seyal vt scroll benchmark] font_scale=not-applicable-vt-only");
    println!("[seyal vt scroll benchmark] percentile_method=aggregate-elapsed-no-percentiles");
}

fn workload(scroll: bool) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(CONTENT.len() + 2);
    bytes.extend_from_slice(CONTENT);
    if scroll {
        bytes.extend_from_slice(b"\r\n");
    } else {
        // Same payload length and printable writes as the scroll case. Replacing LF
        // with CR keeps the cursor on the bottom row and isolates the additional
        // full-screen row-copy/blank/LineId work without pretending this is a
        // cycle-accurate decomposition of parser cost.
        bytes.extend_from_slice(b"\r\r");
    }
    bytes
}

fn run_case(cols: u16, rows: u16, iterations: u64, scroll: bool) -> u128 {
    let mut terminal = TerminalState::new(cols, rows).expect("benchmark dimensions are valid");
    let _ = terminal.take_damage();

    let bottom = format!("\x1b[{rows};1H");
    terminal
        .feed(bottom.as_bytes())
        .expect("positioning feed succeeds");

    let workload = workload(scroll);
    assert!(
        CONTENT.len() < usize::from(cols),
        "content must not wrap by width"
    );

    for _ in 0..100 {
        terminal
            .feed(black_box(&workload))
            .expect("warmup feed succeeds");
    }
    let _ = terminal.take_damage();

    let started = Instant::now();
    for _ in 0..iterations {
        terminal
            .feed(black_box(&workload))
            .expect("benchmark feed succeeds");
    }
    let elapsed = started.elapsed();

    black_box(terminal.cell(0, rows - 1));
    black_box(terminal.line_id(rows - 1));
    black_box(terminal.damage_generation());

    let total_bytes = (workload.len() as u128) * u128::from(iterations);
    let nanos = elapsed.as_nanos().max(1);
    let iterations_per_second = u128::from(iterations).saturating_mul(1_000_000_000) / nanos;
    let bytes_per_second = total_bytes.saturating_mul(1_000_000_000) / nanos;
    let workload_name = if scroll {
        "full-screen-scroll"
    } else {
        "matched-bottom-row-overwrite"
    };

    println!("[seyal vt scroll benchmark] workload={workload_name}");
    println!("[seyal vt scroll benchmark] dimensions={cols}x{rows}");
    println!("[seyal vt scroll benchmark] iterations={iterations}");
    println!(
        "[seyal vt scroll benchmark] workload_bytes={}",
        workload.len()
    );
    println!("[seyal vt scroll benchmark] total_bytes={total_bytes}");
    println!("[seyal vt scroll benchmark] elapsed_ns={nanos}");
    println!("[seyal vt scroll benchmark] iterations_per_second={iterations_per_second}");
    println!("[seyal vt scroll benchmark] bytes_per_second={bytes_per_second}");
    println!("[seyal vt scroll benchmark] performance_claim=false baseline_measurement=true");

    nanos
}

fn main() {
    metadata();
    let iterations = iterations();

    for (cols, rows) in [(80, 24), (120, 40)] {
        let overwrite_ns = run_case(cols, rows, iterations, false);
        let scroll_ns = run_case(cols, rows, iterations, true);
        let incremental_ns = scroll_ns.saturating_sub(overwrite_ns);
        let ratio_milli = scroll_ns.saturating_mul(1000) / overwrite_ns.max(1);

        println!("[seyal vt scroll benchmark] comparison_dimensions={cols}x{rows}");
        println!("[seyal vt scroll benchmark] matched_overwrite_elapsed_ns={overwrite_ns}");
        println!("[seyal vt scroll benchmark] full_scroll_elapsed_ns={scroll_ns}");
        println!("[seyal vt scroll benchmark] incremental_scroll_elapsed_ns={incremental_ns}");
        println!("[seyal vt scroll benchmark] scroll_to_overwrite_ratio_milli={ratio_milli}");
        println!(
            "[seyal vt scroll benchmark] comparison_note=matched aggregate baseline, not cycle-accurate parser decomposition"
        );
    }
}
