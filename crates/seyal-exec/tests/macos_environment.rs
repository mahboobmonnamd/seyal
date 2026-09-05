#![cfg(target_os = "macos")]

use std::time::{Duration, Instant};

use seyal_exec::{ChildExit, CommandSpec, ReadOutcome, TerminalExecution, WindowSize};

const IO_TIMEOUT: Duration = Duration::from_secs(3);

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
    let mut exit = None;
    let mut eof = false;

    // Drain until EOF after the child exits. Fast hosts can observe Exited(0)
    // before the PTY master has delivered the final env lines; stopping at the
    // first exit observation can yield an empty buffer without proving env
    // injection failed.
    while Instant::now() < deadline && !eof {
        match execution
            .read_output(&mut buffer)
            .expect("read environment output")
        {
            ReadOutcome::Bytes(count) => output.extend_from_slice(&buffer[..count]),
            ReadOutcome::WouldBlock => {
                if exit.is_none() {
                    exit = execution.try_wait().expect("wait child");
                }
                let wait = if exit.is_some() {
                    Duration::from_millis(20)
                } else {
                    Duration::from_millis(100)
                };
                let _ = execution.wait_readable(wait).expect("wait readable");
            }
            ReadOutcome::Eof => eof = true,
        }
        if exit.is_none() {
            exit = execution.try_wait().expect("wait child");
        }
    }

    assert_eq!(exit, Some(ChildExit::Exited(0)));
    assert!(eof, "environment PTY did not reach EOF before deadline");
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
