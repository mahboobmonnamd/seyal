#![cfg(target_os = "macos")]
#![allow(unsafe_code)]

use std::{
    sync::{Mutex, MutexGuard},
    time::{Duration, Instant},
};

use seyal_exec::{ChildExit, CommandSpec, ExecError, ReadOutcome, TerminalExecution, WindowSize};

static TEST_LOCK: Mutex<()> = Mutex::new(());
const IO_TIMEOUT: Duration = Duration::from_secs(3);

fn test_guard() -> MutexGuard<'static, ()> {
    TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

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

/// Drain PTY output until `needle` appears or EOF/timeout.
/// Do not reap the child here — early waitpid races can surface as empty
/// buffers on fast CI hosts even when the slave wrote the env lines.
fn read_until(
    execution: &mut TerminalExecution,
    needle: &[u8],
    timeout: Duration,
) -> Result<Vec<u8>, ExecError> {
    let deadline = Instant::now() + timeout;
    let mut output = Vec::new();
    let mut buffer = [0_u8; 256];

    loop {
        match execution.read_output(&mut buffer)? {
            ReadOutcome::Bytes(count) => {
                output.extend_from_slice(&buffer[..count]);
                if output.windows(needle.len()).any(|window| window == needle) {
                    return Ok(output);
                }
            }
            ReadOutcome::WouldBlock => {}
            ReadOutcome::Eof => return Ok(output),
        }

        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Ok(output);
        }
        let readiness = execution.wait_readable(remaining.min(Duration::from_millis(100)))?;
        if readiness.hangup && !readiness.ready {
            match execution.read_output(&mut buffer)? {
                ReadOutcome::Bytes(count) => output.extend_from_slice(&buffer[..count]),
                ReadOutcome::WouldBlock | ReadOutcome::Eof => {}
            }
        }
    }
}

#[test]
fn command_environment_is_explicit_and_pty_injects_no_terminal_markers() {
    let _guard = test_guard();
    let command = CommandSpec::new("/usr/bin/env")
        .clear_environment()
        .env("SEYAL_TEST_VALUE", "explicit");
    let mut execution =
        TerminalExecution::spawn(&command, WindowSize::cells(80, 24).expect("valid size"))
            .expect("spawn PTY command");

    let output =
        read_until(&mut execution, b"SEYAL_TEST_VALUE=explicit", IO_TIMEOUT).expect("read env");
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

    assert_eq!(wait_exit(&mut execution, IO_TIMEOUT), ChildExit::Exited(0));
}

#[test]
fn inherited_descriptor_reaches_child_and_nothing_else_leaks() {
    use std::{
        io::Write,
        os::fd::{AsRawFd, FromRawFd, OwnedFd},
    };

    let _guard = test_guard();
    let mut fds = [0i32; 2];
    // SAFETY: `fds` is a valid two-element array for pipe(2).
    assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
    // SAFETY: both descriptors were just returned by pipe(2) and are owned here.
    let (read_end, write_end) =
        unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) };
    let read_raw = read_end.as_raw_fd();
    // Mark the read end close-on-exec in the parent: only the child-side
    // clearing performed by CommandSpec::inherit_fd may let it cross exec.
    // SAFETY: fcntl on an owned descriptor.
    assert!(unsafe { libc::fcntl(read_raw, libc::F_SETFD, libc::FD_CLOEXEC) } >= 0);
    let mut write_end = std::fs::File::from(write_end);
    write_end.write_all(b"secret-line\n").expect("write secret");
    drop(write_end);

    // A stray non-inherited descriptor must not leak: it stays close-on-exec.
    let stray = std::fs::File::open("/dev/null").expect("open /dev/null");
    let stray_raw = stray.as_raw_fd();

    let script = format!(
        "IFS= read -r line <&$SEYAL_TEST_FD; exec {{SEYAL_TEST_FD}}<&-; \
         printf 'GOT=%s\\n' \"$line\"; \
         if [ -e /dev/fd/{stray_raw} ]; then printf 'STRAY=leaked\\n'; else printf 'STRAY=closed\\n'; fi; \
         printf 'DONE\\n'"
    );
    let command = CommandSpec::new("/bin/zsh")
        .args(["-f", "-c", &script])
        .env("SEYAL_TEST_FD", read_raw.to_string())
        .inherit_fd(read_end);
    let mut execution =
        TerminalExecution::spawn(&command, WindowSize::cells(80, 24).expect("valid size"))
            .expect("spawn PTY command");
    drop(command);
    // The parent's copy of the read end must be closed once the spec is gone.
    // SAFETY: querying descriptor flags on a possibly-closed fd is safe.
    assert_eq!(unsafe { libc::fcntl(read_raw, libc::F_GETFD) }, -1);

    let output = read_until(&mut execution, b"DONE", IO_TIMEOUT).expect("read child output");
    let text = String::from_utf8_lossy(&output);
    assert!(
        text.contains("GOT=secret-line"),
        "child could not read inherited fd: {text}"
    );
    assert!(
        text.contains("STRAY=closed"),
        "non-inherited descriptor leaked: {text}"
    );
    drop(stray);
    let _ = wait_exit(&mut execution, IO_TIMEOUT);
}
