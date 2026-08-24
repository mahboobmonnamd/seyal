#![cfg(target_os = "macos")]
#![allow(unsafe_code)]

use std::{
    fs::File,
    os::fd::AsRawFd,
    sync::{Mutex, MutexGuard},
    time::{Duration, Instant},
};

use seyal_exec::{
    ChildExit, CommandSpec, ExecError, ReadOutcome, TerminalExecution, TerminationPolicy,
    WindowSize,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());
const IO_TIMEOUT: Duration = Duration::from_secs(3);

fn test_guard() -> MutexGuard<'static, ()> {
    TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn sh(script: &str) -> CommandSpec {
    CommandSpec::new("/bin/sh").args(["-c", script])
}

fn termination_policy() -> TerminationPolicy {
    TerminationPolicy::new(Duration::from_millis(100), Duration::from_secs(2))
}

fn read_until(
    execution: &mut TerminalExecution,
    needle: &[u8],
    timeout: Duration,
) -> Result<Vec<u8>, ExecError> {
    let deadline = Instant::now() + timeout;
    let mut output = Vec::new();
    let mut buffer = [0_u8; 8192];

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

fn wait_exit(
    execution: &mut TerminalExecution,
    timeout: Duration,
) -> Result<ChildExit, ExecError> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(exit) = execution.try_wait()? {
            return Ok(exit);
        }
        if Instant::now() >= deadline {
            panic!("child did not exit before test deadline");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn fd_count() -> usize {
    std::fs::read_dir("/dev/fd")
        .expect("/dev/fd must be readable on macOS")
        .count()
}

#[test]
fn real_command_spawn_and_byte_round_trip() {
    let _guard = test_guard();
    let size = WindowSize::cells(80, 24).expect("valid size");
    let command = sh("stty raw -echo; printf ready; head -c 4");
    let mut execution = TerminalExecution::spawn(&command, size).expect("spawn PTY");

    let ready = read_until(&mut execution, b"ready", IO_TIMEOUT).expect("child ready");
    assert!(ready.windows(5).any(|window| window == b"ready"));
    execution
        .write_input_bounded(b"ping", IO_TIMEOUT)
        .expect("write input");
    let output = read_until(&mut execution, b"ping", IO_TIMEOUT).expect("read output");
    assert!(output.windows(4).any(|window| window == b"ping"));

    assert_eq!(
        wait_exit(&mut execution, IO_TIMEOUT).expect("child exit"),
        ChildExit::Exited(0)
    );
}

#[test]
fn bursty_output_is_not_truncated_by_execution_path() {
    let _guard = test_guard();
    let size = WindowSize::cells(80, 24).expect("valid size");
    let command =
        sh("i=0; while [ \"$i\" -lt 4096 ]; do printf 0123456789abcdef; i=$((i+1)); done");
    let mut execution = TerminalExecution::spawn(&command, size).expect("spawn PTY");
    let expected_len = 4096 * 16;
    let deadline = Instant::now() + IO_TIMEOUT;
    let mut output = Vec::with_capacity(expected_len);
    let mut buffer = [0_u8; 8192];

    while output.len() < expected_len && Instant::now() < deadline {
        match execution.read_output(&mut buffer).expect("read") {
            ReadOutcome::Bytes(count) => output.extend_from_slice(&buffer[..count]),
            ReadOutcome::WouldBlock => {
                let _ = execution
                    .wait_readable(Duration::from_millis(100))
                    .expect("wait readable");
            }
            ReadOutcome::Eof => break,
        }
    }

    assert_eq!(output.len(), expected_len);
    assert!(
        output
            .chunks_exact(16)
            .all(|chunk| chunk == b"0123456789abcdef")
    );
    assert_eq!(
        wait_exit(&mut execution, IO_TIMEOUT).expect("child exit"),
        ChildExit::Exited(0)
    );
}

#[test]
fn resize_is_kernel_visible_and_preserves_execution_identity() {
    let _guard = test_guard();
    let initial = WindowSize::cells(80, 24).expect("initial size");
    let resized = WindowSize::new(100, 33, 1000, 660).expect("resized");
    let command = sh("trap 'stty size; exit 0' WINCH; printf ready; while :; do sleep 1; done");
    let mut execution = TerminalExecution::spawn(&command, initial).expect("spawn PTY");
    let child_id = execution.child_id();

    let ready = read_until(&mut execution, b"ready", IO_TIMEOUT).expect("ready output");
    assert!(ready.windows(5).any(|window| window == b"ready"));

    execution.resize(resized).expect("resize execution");
    assert_eq!(execution.window_size().expect("get window size"), resized);
    assert_eq!(execution.child_id(), child_id);
    assert_eq!(execution.terminal().cols(), resized.columns());
    assert_eq!(execution.terminal().rows(), resized.rows());

    let output = read_until(&mut execution, b"33 100", IO_TIMEOUT).expect("resize output");
    assert!(
        output
            .windows(b"33 100".len())
            .any(|window| window == b"33 100"),
        "child did not observe resized rows/columns: {output:?}"
    );
    assert_eq!(
        wait_exit(&mut execution, IO_TIMEOUT).expect("child exit"),
        ChildExit::Exited(0)
    );
}

#[test]
fn normal_and_signal_exits_are_distinct() {
    let _guard = test_guard();
    let size = WindowSize::default();

    let mut normal =
        TerminalExecution::spawn(&sh("exit 23"), size).expect("spawn normal exit child");
    assert_eq!(
        wait_exit(&mut normal, IO_TIMEOUT).expect("normal exit"),
        ChildExit::Exited(23)
    );
    assert_eq!(
        normal.try_wait().expect("idempotent wait"),
        Some(ChildExit::Exited(23))
    );

    let mut signaled =
        TerminalExecution::spawn(&sh("kill -TERM $$"), size).expect("spawn signaled child");
    assert_eq!(
        wait_exit(&mut signaled, IO_TIMEOUT).expect("signal exit"),
        ChildExit::Signaled(libc::SIGTERM)
    );
}

#[test]
fn child_exit_eventually_becomes_master_eof_or_hangup() {
    let _guard = test_guard();
    let size = WindowSize::default();
    let mut execution = TerminalExecution::spawn(&sh("printf done"), size).expect("spawn PTY");

    let output = read_until(&mut execution, b"done", IO_TIMEOUT).expect("output");
    assert!(output.windows(4).any(|window| window == b"done"));
    assert_eq!(
        wait_exit(&mut execution, IO_TIMEOUT).expect("child exit"),
        ChildExit::Exited(0)
    );

    let deadline = Instant::now() + IO_TIMEOUT;
    let mut buffer = [0_u8; 32];
    let mut closed = false;
    while Instant::now() < deadline {
        let readiness = execution
            .wait_readable(Duration::from_millis(100))
            .expect("wait");
        if readiness.hangup {
            closed = true;
            break;
        }
        if matches!(
            execution.read_output(&mut buffer).expect("read"),
            ReadOutcome::Eof
        ) {
            closed = true;
            break;
        }
    }
    assert!(closed, "PTY master never reported EOF/HUP after child exit");
}

#[test]
fn explicit_terminate_reaps_only_the_owned_process_group() {
    let _guard = test_guard();
    let size = WindowSize::default();
    let mut execution = TerminalExecution::spawn(&sh("while :; do sleep 1; done"), size)
        .expect("spawn PTY");

    let exit = execution
        .terminate(termination_policy())
        .expect("terminate owned execution");
    assert!(matches!(
        exit,
        ChildExit::Signaled(libc::SIGTERM) | ChildExit::Signaled(libc::SIGKILL)
    ));
    assert_eq!(execution.try_wait().expect("idempotent reap"), Some(exit));
}

#[test]
fn repeated_spawn_terminate_does_not_accumulate_fds() {
    let _guard = test_guard();
    let before = fd_count();
    let size = WindowSize::default();

    for _ in 0..16 {
        let mut execution = TerminalExecution::spawn(&sh("while :; do sleep 1; done"), size)
            .expect("spawn");
        execution
            .terminate(termination_policy())
            .expect("terminate and reap");
    }

    let after = fd_count();
    assert!(
        after <= before + 1,
        "descriptor count grew unexpectedly: before={before} after={after}"
    );
}

#[test]
fn readiness_works_above_select_fd_limit() {
    let _guard = test_guard();

    unsafe {
        let mut limit = std::mem::MaybeUninit::<libc::rlimit>::uninit();
        assert_eq!(libc::getrlimit(libc::RLIMIT_NOFILE, limit.as_mut_ptr()), 0);
        let mut limit = limit.assume_init();
        if limit.rlim_cur < 1200 {
            limit.rlim_cur = limit.rlim_max.min(2048);
            assert!(
                limit.rlim_cur >= 1200,
                "runner hard fd limit is too low for high-fd regression"
            );
            assert_eq!(libc::setrlimit(libc::RLIMIT_NOFILE, &limit), 0);
        }
    }

    let mut held = Vec::new();
    loop {
        let file = File::open("/dev/null").expect("open fd");
        let fd = file.as_raw_fd();
        held.push(file);
        if fd >= 1100 {
            break;
        }
    }

    let mut execution = TerminalExecution::spawn(&sh("printf highfd"), WindowSize::default())
        .expect("spawn PTY");
    let output = read_until(&mut execution, b"highfd", IO_TIMEOUT).expect("high fd output");
    assert!(output.windows(6).any(|window| window == b"highfd"));
    assert_eq!(
        wait_exit(&mut execution, IO_TIMEOUT).expect("child exit"),
        ChildExit::Exited(0)
    );

    drop(held);
}

#[test]
fn invalid_command_does_not_leak_descriptors() {
    let _guard = test_guard();
    let before = fd_count();
    let result = TerminalExecution::spawn(
        &CommandSpec::new("/definitely/not/a/seyal-command"),
        WindowSize::default(),
    );
    assert!(result.is_err());
    let after = fd_count();
    assert!(
        after <= before + 1,
        "spawn failure leaked descriptors: before={before} after={after}"
    );
}

#[test]
fn terminal_execution_feeds_the_single_authoritative_terminal_state() {
    let _guard = test_guard();
    let mut execution =
        TerminalExecution::spawn(&sh("printf abc"), WindowSize::cells(80, 24).expect("size"))
            .expect("spawn execution");
    let deadline = Instant::now() + IO_TIMEOUT;
    let mut buffer = [0_u8; 128];

    while Instant::now() < deadline {
        match execution.read_output(&mut buffer).expect("read output") {
            ReadOutcome::Bytes(_) => {
                if execution
                    .terminal()
                    .row_text(0)
                    .is_some_and(|row| row.starts_with("abc"))
                {
                    break;
                }
            }
            ReadOutcome::WouldBlock => {
                let _ = execution
                    .wait_readable(Duration::from_millis(100))
                    .expect("wait");
            }
            ReadOutcome::Eof => break,
        }
    }

    assert!(
        execution
            .terminal()
            .row_text(0)
            .is_some_and(|row| row.starts_with("abc"))
    );
    assert_eq!(
        wait_exit(&mut execution, IO_TIMEOUT).expect("execution exit"),
        ChildExit::Exited(0)
    );
}
