use std::{env, hint::black_box, time::Instant};

use seyal_terminal::TerminalState;

const COLS: u16 = 120;
const ROWS: u16 = 40;
const DEFAULT_ITERATIONS: u64 = 20_000;
const WORKLOAD: &[u8] = concat!(
    "\x1b[H",
    "prompt$ cargo test --workspace\r\n",
    "\x1b[32mPASS\x1b[0m 128 tests in 0.42s\r\n",
    "\x1b[38;2;80;160;240mstatus\x1b[0m ",
    "unicode: € λ ✓ ",
    "\x1b[2;40Hdone",
    "\x1b[?25l\x1b[?25h",
)
.as_bytes();

fn main() {
    let iterations = env::var("SEYAL_VT_BENCH_ITERATIONS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_ITERATIONS);

    let mut terminal = TerminalState::new(COLS, ROWS).expect("benchmark dimensions are valid");
    let _ = terminal.take_damage();

    // Warm the production path without publishing a performance claim from
    // initialization/cache effects.
    for _ in 0..100 {
        terminal
            .feed(black_box(WORKLOAD))
            .expect("benchmark feed succeeds");
    }
    let _ = terminal.take_damage();

    let started = Instant::now();
    for _ in 0..iterations {
        terminal
            .feed(black_box(WORKLOAD))
            .expect("benchmark feed succeeds");
    }
    let elapsed = started.elapsed();

    let bytes = (WORKLOAD.len() as u128) * u128::from(iterations);
    let nanos = elapsed.as_nanos().max(1);
    let bytes_per_second = bytes.saturating_mul(1_000_000_000) / nanos;

    // Consume authoritative state so the benchmark cannot be reduced to a
    // dead input loop by optimization.
    black_box(terminal.cursor());
    black_box(terminal.cell(0, 0));
    black_box(terminal.damage_generation());

    println!("[seyal vt benchmark] workload=m001-vt-parser-state");
    println!("[seyal vt benchmark] dimensions={COLS}x{ROWS}");
    println!("[seyal vt benchmark] iterations={iterations}");
    println!("[seyal vt benchmark] workload_bytes={}", WORKLOAD.len());
    println!("[seyal vt benchmark] total_bytes={bytes}");
    println!("[seyal vt benchmark] elapsed_ns={nanos}");
    println!("[seyal vt benchmark] bytes_per_second={bytes_per_second}");
    println!("[seyal vt benchmark] performance_claim=false baseline_measurement=true");
}
