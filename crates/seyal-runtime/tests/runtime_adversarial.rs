#![cfg(target_os = "macos")]
#![allow(unsafe_code)]

use std::{
    sync::{
        Mutex, MutexGuard,
        atomic::{AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

#[cfg(feature = "test-fault-injection")]
use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
};

use seyal_exec::{CommandSpec, WindowSize};
use seyal_runtime::{ExecutionLifecycle, LocalIpcMode, Runtime, RuntimeConfig};

#[cfg(feature = "test-fault-injection")]
use seyal_exec::test_fault::{self as exec_fault, FaultPoint as ExecFaultPoint};
#[cfg(feature = "test-fault-injection")]
use seyal_runtime::{
    local_ipc::framing::{ClientHello, FrameHeader, HEADER_LEN, MessageType, encode_frame},
    test_fault::{self, FaultPoint},
};

const PTY_EOF_HELPER_ENV: &str = "SEYAL_PTY_EOF_HELPER_MS";
const PTY_EOF_HELPER_TEST: &str = "pty_eof_live_child_helper";

// All tests in this integration-test binary share one process. FD-baseline
// assertions are meaningful only when sibling tests cannot open/close
// descriptors concurrently.
static TEST_SERIAL: Mutex<()> = Mutex::new(());
static IPC_SCOPE_COUNTER: AtomicU64 = AtomicU64::new(0);

fn serialized() -> MutexGuard<'static, ()> {
    TEST_SERIAL
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}

fn unique_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}

fn size() -> WindowSize {
    WindowSize::new(80, 24, 0, 0).expect("valid terminal size")
}

fn config(test: &str, local_ipc: bool) -> RuntimeConfig {
    let suffix = unique_suffix();
    let mut config = RuntimeConfig::m001().expect("M001 config");
    config.singleton_path = std::env::temp_dir().join(format!(
        "seyal-adversarial-{}-{suffix:x}-{test}.lock",
        std::process::id()
    ));
    config.local_ipc = if local_ipc {
        let ipc_suffix = IPC_SCOPE_COUNTER.fetch_add(1, Ordering::Relaxed);
        LocalIpcMode::Enabled {
            // Darwin sockaddr_un.sun_path is only 104 bytes including NUL.
            // Keep the test leaf deliberately compact so a long per-user temp
            // directory cannot prevent this test from reaching the listener
            // resource-pressure behavior it is meant to exercise.
            runtime_dir_override: Some(
                std::env::temp_dir().join(format!("s5ad-{:x}-{ipc_suffix:x}", std::process::id())),
            ),
        }
    } else {
        LocalIpcMode::Disabled
    };
    config.graceful_termination = Duration::from_millis(50);
    config.forced_reap = Duration::from_millis(250);
    config.final_drain = Duration::from_millis(100);
    config
}

fn fd_count() -> usize {
    (0..1024)
        .filter(|fd| {
            // SAFETY: F_GETFD only observes the integer descriptor and reports
            // EBADF for values that are not open in this process.
            (unsafe { libc::fcntl(*fd, libc::F_GETFD) }) >= 0
        })
        .count()
}

fn shutdown(runtime: &mut Runtime) {
    runtime.begin_shutdown().expect("begin shutdown");
    runtime
        .run_until_empty(Instant::now() + Duration::from_secs(3))
        .expect("shutdown completes");
    assert_eq!(runtime.execution_count(), 0);
    assert_eq!(runtime.aggregate_accepted_but_unwritten_bytes(), 0);
}

fn pty_eof_helper_command(live_for: Duration) -> CommandSpec {
    CommandSpec::new(std::env::current_exe().expect("current adversarial test executable"))
        .args(["--exact", PTY_EOF_HELPER_TEST, "--nocapture"])
        .env(PTY_EOF_HELPER_ENV, live_for.as_millis().to_string())
}

// This test is also a subprocess fixture. Normal workspace execution returns
// immediately; a parent adversarial test re-execs this test binary with the
// environment variable set so the primary child can create a real Darwin
// PTY-EOF/live-process state without relying on shell redirection semantics.
#[test]
fn pty_eof_live_child_helper() {
    let Ok(raw_delay) = std::env::var(PTY_EOF_HELPER_ENV) else {
        return;
    };
    let delay_ms = raw_delay
        .parse::<u64>()
        .expect("valid PTY EOF helper delay");

    // The helper is exec'd directly, so the only PTY slave descriptors in this
    // process are the stdin/stdout/stderr descriptors installed by
    // TerminalExecution. Closing exactly those descriptors makes the PTY
    // endpoint disappear while this same primary process deliberately remains
    // alive. Do not call TIOCNOTTY here: detaching the controlling terminal is
    // a separate lifecycle action and can itself cause the session to observe
    // hangup/exit behavior, which would destroy the state this test must hold.
    //
    // SAFETY: this code runs only in the dedicated subprocess fixture. The
    // descriptor changes are confined to that subprocess, and _exit avoids the
    // libtest harness trying to write results through the intentionally closed
    // stdout/stderr descriptors.
    unsafe {
        libc::close(libc::STDIN_FILENO);
        libc::close(libc::STDOUT_FILENO);
        libc::close(libc::STDERR_FILENO);
    }

    thread::sleep(Duration::from_millis(delay_ms));
    // SAFETY: this is the end of the dedicated fixture subprocess.
    unsafe { libc::_exit(0) }
}

#[test]
fn pty_eof_from_live_children_stays_running_without_a_reap_poll_loop_and_remains_terminable() {
    let _guard = serialized();
    let baseline_fds = fd_count();
    {
        let mut runtime = Runtime::new(config("pty-eof-live-child", false)).expect("Runtime");
        let mut ids = Vec::new();
        for _ in 0..8 {
            ids.push(
                runtime
                    .create_execution(pty_eof_helper_command(Duration::from_secs(30)), size())
                    .expect("live execution"),
            );
        }

        let close_deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let all_closed = ids.iter().all(|id| runtime.input_ingress(*id).is_err());
            if all_closed {
                break;
            }
            assert!(Instant::now() < close_deadline, "PTY EOF was not observed");
            runtime
                .poll_once(Some(Duration::from_millis(50)))
                .expect("Runtime poll");
        }

        for id in &ids {
            assert_eq!(
                runtime
                    .lookup(*id)
                    .expect("execution remains tracked")
                    .lifecycle,
                ExecutionLifecycle::Running,
                "PTY EOF must not pretend that the primary child exited"
            );
        }

        // Allow the bounded EOF/NOTE_EXIT race probes (10..320 ms) to exhaust.
        let probe_window = Instant::now() + Duration::from_millis(750);
        while Instant::now() < probe_window {
            runtime
                .poll_once(Some(Duration::from_millis(50)))
                .expect("bounded reap probe poll");
        }

        // With read interest disarmed and the bounded probe budget exhausted,
        // an otherwise idle Runtime must sleep for the caller's wait rather
        // than returning on a 10 ms PrimaryExitPending deadline forever.
        let idle_start = Instant::now();
        runtime
            .poll_once(Some(Duration::from_millis(80)))
            .expect("idle Runtime poll");
        assert!(
            idle_start.elapsed() >= Duration::from_millis(50),
            "closed PTYs still cause periodic/busy Runtime wakeups"
        );

        // Exercise both public lifecycle controls that the old
        // PrimaryExitPending misclassification broke. Explicitly terminate
        // half the live children, then let Runtime shutdown terminate the
        // remainder (while remaining idempotent for the already-terminating
        // half).
        for id in ids.iter().take(4) {
            runtime
                .request_termination(*id)
                .expect("PTY-closed live child remains explicitly terminable");
        }
        shutdown(&mut runtime);
    }
    assert_eq!(fd_count(), baseline_fds, "PTY EOF path leaked descriptors");
}

#[test]
fn pty_eof_live_child_is_finalized_when_the_primary_later_exits_naturally() {
    let _guard = serialized();
    let baseline_fds = fd_count();
    {
        let mut runtime = Runtime::new(config("pty-eof-natural-exit", false)).expect("Runtime");
        let id = runtime
            .create_execution(pty_eof_helper_command(Duration::from_secs(5)), size())
            .expect("execution");

        let close_deadline = Instant::now() + Duration::from_secs(3);
        while runtime.input_ingress(id).is_ok() {
            assert!(Instant::now() < close_deadline, "PTY EOF was not observed");
            runtime
                .poll_once(Some(Duration::from_millis(50)))
                .expect("Runtime poll");
        }
        assert_eq!(
            runtime
                .lookup(id)
                .expect("live child remains tracked after PTY EOF")
                .lifecycle,
            ExecutionLifecycle::Running
        );

        let exit_deadline = Instant::now() + Duration::from_secs(7);
        while runtime.lookup(id).is_some() {
            assert!(
                Instant::now() < exit_deadline,
                "natural child exit was not observed"
            );
            runtime
                .poll_once(Some(Duration::from_millis(100)))
                .expect("Runtime poll");
        }
    }
    assert_eq!(
        fd_count(),
        baseline_fds,
        "natural PTY EOF path leaked descriptors"
    );
}

#[cfg(feature = "test-fault-injection")]
#[test]
fn repeated_listener_resource_pressure_backs_off_without_starving_active_pty_work() {
    let _guard = serialized();
    let baseline_fds = fd_count();
    {
        let mut runtime = Runtime::new(config("accept-pressure", true)).expect("Runtime");
        let execution_id = runtime
            .create_execution(
                CommandSpec::new("/bin/sh").args(["-c", "printf PTY_PROGRESS; sleep 30"]),
                size(),
            )
            .expect("active PTY execution");

        test_fault::fail_times(FaultPoint::AcceptResourcePressure, 4);
        let mut client = UnixStream::connect(runtime.local_ipc_socket_path().unwrap())
            .expect("client enters listener backlog");
        client.set_nonblocking(true).unwrap();
        client
            .write_all(&encode_frame(
                MessageType::ClientHello,
                &ClientHello {
                    client_capabilities: 0,
                }
                .encode(),
            ))
            .expect("queue hello before accept");

        let started = Instant::now();
        let deadline = started + Duration::from_secs(3);
        let mut bytes = Vec::new();
        let mut pty_progress_while_pressured = false;
        let mut got_server_hello = false;
        while !got_server_hello {
            assert!(
                Instant::now() < deadline,
                "listener did not recover after pressure"
            );
            runtime
                .poll_once(Some(Duration::from_millis(25)))
                .expect("resource pressure is non-fatal");

            if test_fault::remaining(FaultPoint::AcceptResourcePressure) > 0
                && runtime
                    .execution(execution_id)
                    .and_then(|execution| execution.terminal().row_text(0))
                    .is_some_and(|row| row.contains("PTY_PROGRESS"))
            {
                pty_progress_while_pressured = true;
            }

            let mut chunk = [0u8; 4096];
            loop {
                match client.read(&mut chunk) {
                    Ok(0) => panic!("listener closed client during recoverable pressure"),
                    Ok(count) => bytes.extend_from_slice(&chunk[..count]),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(error) => panic!("client read failed: {error}"),
                }
            }
            if bytes.len() >= HEADER_LEN {
                let header = FrameHeader::decode(&bytes[..HEADER_LEN]).expect("valid server frame");
                let total = HEADER_LEN + header.payload_len as usize;
                if bytes.len() >= total {
                    assert_eq!(
                        MessageType::from_u16(header.message_type),
                        Some(MessageType::ServerHello)
                    );
                    got_server_hello = true;
                }
            }
        }

        // Four no-progress turns require 10 + 20 + 40 + 80 ms backoffs before
        // the pending client can be admitted. This catches immediate
        // level-triggered retry loops without relying on CPU sampling noise.
        assert!(
            started.elapsed() >= Duration::from_millis(100),
            "listener pressure retries were not throttled"
        );
        assert!(
            pty_progress_while_pressured,
            "listener pressure starved unrelated PTY progress"
        );
        assert_eq!(
            test_fault::remaining(FaultPoint::AcceptResourcePressure),
            0,
            "all deterministic pressure attempts should have been consumed"
        );

        drop(client);
        for _ in 0..4 {
            runtime
                .poll_once(Some(Duration::from_millis(10)))
                .expect("disconnect cleanup");
        }
        shutdown(&mut runtime);
    }
    assert_eq!(
        fd_count(),
        baseline_fds,
        "listener resource-pressure recovery leaked descriptors"
    );
}

#[cfg(feature = "test-fault-injection")]
#[test]
fn termination_failed_recovers_after_forced_reap_miss() {
    let _guard = serialized();
    let baseline_fds = fd_count();
    {
        let mut cfg = config("termination-failed-recover", false);
        cfg.graceful_termination = Duration::from_millis(20);
        cfg.forced_reap = Duration::from_millis(20);
        let mut runtime = Runtime::new(cfg).expect("Runtime");

        let id = runtime
            .create_execution(CommandSpec::new("/bin/sh").args(["-c", "sleep 30"]), size())
            .expect("live execution");

        // After SIGKILL stores an exit, keep pretending reap is not ready so the
        // forced-reap deadline must enter recoverable TerminationFailed instead
        // of silently draining — then clear the fault and prove recovery.
        exec_fault::fail_times(ExecFaultPoint::ChildTryWait, 64);
        runtime
            .request_termination(id)
            .expect("termination request");

        let failed_deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let lifecycle = runtime
                .lookup(id)
                .expect("execution remains owned through TerminationFailed")
                .lifecycle;
            if lifecycle == ExecutionLifecycle::TerminationFailed {
                break;
            }
            assert!(
                Instant::now() < failed_deadline,
                "forced-reap miss never entered TerminationFailed (last={lifecycle:?})"
            );
            runtime
                .poll_once(Some(Duration::from_millis(10)))
                .expect("TerminationFailed path remains pollable");
        }

        exec_fault::fail_times(ExecFaultPoint::ChildTryWait, 0);
        let recover_deadline = Instant::now() + Duration::from_secs(3);
        while runtime.lookup(id).is_some() {
            assert!(
                Instant::now() < recover_deadline,
                "TerminationFailed did not recover after reap became available"
            );
            runtime
                .poll_once(Some(Duration::from_millis(10)))
                .expect("recovery poll");
        }
        assert_eq!(runtime.execution_count(), 0);
    }
    assert_eq!(
        fd_count(),
        baseline_fds,
        "TerminationFailed recovery leaked descriptors"
    );
}

#[cfg(feature = "test-fault-injection")]
#[test]
fn primary_exit_pending_escalates_to_termination_failed_then_recovers() {
    let _guard = serialized();
    let baseline_fds = fd_count();
    {
        let mut runtime =
            Runtime::new(config("primary-exit-pending-bound", false)).expect("Runtime");

        // Child exits immediately; unreapable try_wait faults force PrimaryExitPending
        // then the hard attempt bound must escalate into TerminationFailed.
        exec_fault::fail_times(ExecFaultPoint::ChildTryWait, 32);
        let id = runtime
            .create_execution(CommandSpec::new("/bin/sh").args(["-c", "true"]), size())
            .expect("short-lived execution");

        let escalate_deadline = Instant::now() + Duration::from_secs(3);
        let mut saw_pending = false;
        loop {
            match runtime.lookup(id).map(|summary| summary.lifecycle) {
                Some(ExecutionLifecycle::PrimaryExitPending) => saw_pending = true,
                Some(ExecutionLifecycle::TerminationFailed) => break,
                Some(other) => {
                    assert!(
                        Instant::now() < escalate_deadline,
                        "PrimaryExitPending never escalated (last={other:?})"
                    );
                }
                None => panic!("execution finalized before PrimaryExitPending bound escalated"),
            }
            runtime
                .poll_once(Some(Duration::from_millis(5)))
                .expect("bounded PrimaryExitPending poll");
        }
        assert!(
            saw_pending,
            "escalation must pass through PrimaryExitPending"
        );

        exec_fault::fail_times(ExecFaultPoint::ChildTryWait, 0);
        let recover_deadline = Instant::now() + Duration::from_secs(3);
        while runtime.lookup(id).is_some() {
            assert!(
                Instant::now() < recover_deadline,
                "escalated TerminationFailed never recovered"
            );
            runtime
                .poll_once(Some(Duration::from_millis(10)))
                .expect("escalation recovery poll");
        }
    }
    assert_eq!(
        fd_count(),
        baseline_fds,
        "PrimaryExitPending escalation recovery leaked descriptors"
    );
}

#[cfg(feature = "test-fault-injection")]
#[test]
fn request_termination_from_termination_failed_rearms_graceful_path() {
    let _guard = serialized();
    let baseline_fds = fd_count();
    {
        let mut cfg = config("termination-failed-rearm", false);
        cfg.graceful_termination = Duration::from_millis(20);
        cfg.forced_reap = Duration::from_millis(20);
        let mut runtime = Runtime::new(cfg).expect("Runtime");

        let id = runtime
            .create_execution(CommandSpec::new("/bin/sh").args(["-c", "sleep 30"]), size())
            .expect("live execution");

        exec_fault::fail_times(ExecFaultPoint::ChildTryWait, 64);
        runtime
            .request_termination(id)
            .expect("initial termination");

        let failed_deadline = Instant::now() + Duration::from_secs(3);
        while runtime.lookup(id).map(|summary| summary.lifecycle)
            != Some(ExecutionLifecycle::TerminationFailed)
        {
            assert!(
                Instant::now() < failed_deadline,
                "did not reach TerminationFailed before re-arm"
            );
            runtime
                .poll_once(Some(Duration::from_millis(10)))
                .expect("poll toward TerminationFailed");
        }

        // Re-arm while unreapability persists: must leave the sink and re-enter
        // the graceful→forced signalling ladder rather than no-op.
        runtime
            .request_termination(id)
            .expect("TerminationFailed remains explicitly terminable");
        assert_eq!(
            runtime
                .lookup(id)
                .expect("still owned after re-arm")
                .lifecycle,
            ExecutionLifecycle::TerminatingGraceful
        );

        exec_fault::fail_times(ExecFaultPoint::ChildTryWait, 0);
        shutdown(&mut runtime);
    }
    assert_eq!(
        fd_count(),
        baseline_fds,
        "TerminationFailed re-arm path leaked descriptors"
    );
}
