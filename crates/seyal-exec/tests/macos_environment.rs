#![cfg(target_os = "macos")]

use std::time::{Duration, Instant};

use seyal_exec::{ChildExit, CommandSpec, ReadOutcome, TerminalEndpoint, WindowSize};

const IO_TIMEOUT: Duration = Duration::from_secs(3);

#[test]
fn command_environment_is_explicit_and_pty_injects_no_terminal_markers() {
    let command = CommandSpec::new("/usr/bin/env")
        .clear_environment()
        .env("SEYAL_TEST_VALUE", "explicit");
    let mut endpoint =
        TerminalEndpoint::spawn(&command, WindowSize::default()).expect("spawn PTY command");

    let deadline = Instant::now() + IO_TIMEOUT;
    let mut output = Vec::new();
    let mut buffer = [0_u8; 256];
    let exit = loop {
        match endpoint.read(&mut buffer).expect("read environment output") {
            ReadOutcome::Bytes(count) => output.extend_from_slice(&buffer[..count]),
            ReadOutcome::WouldBlock => {
                let _ = endpoint
                    .wait_readable(Duration::from_millis(100))
                    .expect("wait readable");
            }
            ReadOutcome::Eof => {}
        }

        if let Some(exit) = endpoint.try_wait().expect("wait child") {
            break exit;
        }
        assert!(Instant::now() < deadline, "environment child did not exit");
    };

    assert_eq!(exit, ChildExit::Exited(0));
    let output = String::from_utf8(output).expect("env output is utf-8");
    assert!(
        output.lines().any(|line| line == "SEYAL_TEST_VALUE=explicit\r" || line == "SEYAL_TEST_VALUE=explicit"),
        "explicit environment override missing: {output:?}"
    );
    assert!(!output.lines().any(|line| line.starts_with("TERM=")));
    assert!(!output.lines().any(|line| line.starts_with("SEYAL_INSIDE=")));
    assert!(!output.lines().any(|line| line.starts_with("RILL_INSIDE=")));
}
