#![cfg(target_os = "macos")]

use std::time::{Duration, Instant};

use seyal_exec::{ChildExit, CommandSpec, ReadOutcome, TerminalExecution, WindowSize};

const IO_TIMEOUT: Duration = Duration::from_secs(3);

fn wait_exit(execution: &mut TerminalExecution, timeout: Duration) -> ChildExit {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(exit) = execution.try_wait().expect("wait child") {
            return exit;
        }
        if Instant::now() >= deadline {
            panic!("child did not exit before test deadline");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn command_environment_is_explicit_and_pty_injects_no_terminal_markers() {
    let command = CommandSpec::new("/usr/bin/env")
        .clear_environment()
        .env("SEYAL_TEST_VALUE", "explicit");
    let mut execution =
        TerminalExecution::spawn(&command, WindowSize::default()).expect("spawn PTY command");

    let deadline = Instant::now() + IO_TIMEOUT;
    let mut output = Vec::new();
    let mut buffer = [0_u8; 256];
    let mut child_exited = false;
    let mut eof = false;

    // Drain until EOF. Fast hosts can observe child exit before the PTY master
    // delivers the final env lines; stopping at the first exit observation can
    // yield an empty buffer. Conversely, PTY EOF can arrive a beat before
    // waitpid reports Exited(0), so exit is reaped separately after drain.
    while Instant::now() < deadline && !eof {
        match execution
            .read_output(&mut buffer)
            .expect("read environment output")
        {
            ReadOutcome::Bytes(count) => output.extend_from_slice(&buffer[..count]),
            ReadOutcome::WouldBlock => {
                if !child_exited {
                    child_exited = execution.try_wait().expect("wait child").is_some();
                }
                let wait = if child_exited {
                    Duration::from_millis(20)
                } else {
                    Duration::from_millis(100)
                };
                let _ = execution.wait_readable(wait).expect("wait readable");
            }
            ReadOutcome::Eof => eof = true,
        }
        if !child_exited {
            child_exited = execution.try_wait().expect("wait child").is_some();
        }
    }

    assert!(eof, "environment PTY did not reach EOF before deadline");
    let remaining = deadline.saturating_duration_since(Instant::now());
    assert_eq!(
        wait_exit(&mut execution, remaining.max(Duration::from_millis(100))),
        ChildExit::Exited(0)
    );
    let output = String::from_utf8(output).expect("env output is utf-8");
    assert!(
        output.lines().any(
            |line| line == "SEYAL_TEST_VALUE=explicit\r" || line == "SEYAL_TEST_VALUE=explicit"
        ),
        "explicit environment override missing: {output:?}"
    );
    assert!(!output.lines().any(|line| line.starts_with("TERM=")));
    assert!(!output.lines().any(|line| line.starts_with("SEYAL_INSIDE=")));
    assert!(!output.lines().any(|line| line.starts_with("RILL_INSIDE=")));
}
