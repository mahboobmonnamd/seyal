#[cfg(target_os = "macos")]
use std::{
    hint::black_box,
    time::{Duration, Instant},
};

#[cfg(target_os = "macos")]
use seyal_exec::{CommandSpec, ReadOutcome, TerminalExecution, TerminationPolicy, WindowSize};

#[cfg(target_os = "macos")]
fn main() {
    const ITERATIONS: u64 = 256;
    const PAYLOAD: &[u8] = b"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let command = CommandSpec::new("/bin/sh").args(["-c", "stty raw -echo; printf ready; cat"]);
    let mut execution =
        TerminalExecution::spawn(&command, WindowSize::cells(120, 40).expect("size"))
            .expect("spawn benchmark PTY");

    let mut buffer = [0_u8; 4096];
    let ready_deadline = Instant::now() + Duration::from_secs(2);
    let mut ready = Vec::new();
    while Instant::now() < ready_deadline {
        match execution.read_output(&mut buffer).expect("benchmark ready read") {
            ReadOutcome::Bytes(count) => {
                ready.extend_from_slice(&buffer[..count]);
                if ready.windows(5).any(|window| window == b"ready") {
                    break;
                }
            }
            ReadOutcome::WouldBlock => {
                let readiness = execution
                    .wait_readable(Duration::from_millis(100))
                    .expect("benchmark ready wait");
                assert!(readiness.ready || !readiness.hangup);
            }
            ReadOutcome::Eof => panic!("benchmark child closed before ready"),
        }
    }
    assert!(ready.windows(5).any(|window| window == b"ready"));

    let started = Instant::now();
    let mut received = 0_u128;

    for _ in 0..ITERATIONS {
        execution
            .write_input_bounded(black_box(PAYLOAD), Duration::from_secs(2))
            .expect("benchmark write");

        let mut iteration_received = 0;
        while iteration_received < PAYLOAD.len() {
            match execution.read_output(&mut buffer).expect("benchmark read") {
                ReadOutcome::Bytes(count) => {
                    black_box(&buffer[..count]);
                    iteration_received += count;
                    received += count as u128;
                }
                ReadOutcome::WouldBlock => {
                    let readiness = execution
                        .wait_readable(Duration::from_secs(2))
                        .expect("benchmark wait");
                    assert!(readiness.ready || readiness.hangup);
                }
                ReadOutcome::Eof => panic!("benchmark child closed PTY early"),
            }
        }
    }

    let elapsed = started.elapsed();
    let nanos = elapsed.as_nanos().max(1);
    let bytes_per_second = received.saturating_mul(1_000_000_000) / nanos;

    let _ = execution.terminate(TerminationPolicy::new(
        Duration::from_millis(100),
        Duration::from_secs(2),
    ));

    println!("[seyal pty benchmark] workload=m001-terminal-execution-roundtrip");
    println!("[seyal pty benchmark] dimensions=120x40");
    println!("[seyal pty benchmark] iterations={ITERATIONS}");
    println!("[seyal pty benchmark] payload_bytes={}", PAYLOAD.len());
    println!("[seyal pty benchmark] received_bytes={received}");
    println!("[seyal pty benchmark] elapsed_ns={nanos}");
    println!("[seyal pty benchmark] bytes_per_second={bytes_per_second}");
    println!("[seyal pty benchmark] performance_claim=false baseline_measurement=true");
}

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("[seyal pty benchmark] skipped: M001 PTY implementation is macOS-only");
}
