#![cfg(target_os = "macos")]

use std::time::{Duration, Instant};

use seyal_exec::{ChildExit, CommandSpec, ReadOutcome, TerminalEndpoint, WindowSize};

const IO_TIMEOUT: Duration = Duration::from_secs(3);

#[test]
fn command_environment_is_explicit_and_pty_injects_no_terminal_markers() {
    let command = CommandSpec::new("/bin/sh")
        .args([
            "-c",
            "printf '%s|%s|%s|%s' \"${SEYAL_TEST_VALUE-unset}\" \"${TERM-unset}\" \"${SEYAL_INSIDE-unset}\" \"${RILL_INSIDE-unset}\"",
        ])
        .clear_environment()
        .env("SEYAL_TEST_VALUE", "explicit");
    let mut endpoint =
        TerminalEndpoint::spawn(&command, WindowSize::default()).expect("spawn PTY command");

    let deadline = Instant::now() + IO_TIMEOUT;
    let mut output = Vec::new();
    let mut buffer = [0_u8; 256];
    while Instant::now() < deadline {
        match endpoint.read(&mut buffer).expect("read environment output") {
            ReadOutcome::Bytes(count) => {
                output.extend_from_slice(&buffer[..count]);
                if output
                    .windows(b"explicit|unset|unset|unset".len())
                    .any(|window| window == b"explicit|unset|unset|unset")
                {
                    break;
                }
            }
            ReadOutcome::WouldBlock => {
                let _ = endpoint
                    .wait_readable(Duration::from_millis(100))
                    .expect("wait readable");
            }
            ReadOutcome::Eof => break,
        }
    }

    assert!(
        output
            .windows(b"explicit|unset|unset|unset".len())
            .any(|window| window == b"explicit|unset|unset|unset"),
        "unexpected child environment: {output:?}"
    );

    let deadline = Instant::now() + IO_TIMEOUT;
    loop {
        if let Some(exit) = endpoint.try_wait().expect("wait child") {
            assert_eq!(exit, ChildExit::Exited(0));
            break;
        }
        assert!(Instant::now() < deadline, "environment child did not exit");
        std::thread::sleep(Duration::from_millis(5));
    }
}
