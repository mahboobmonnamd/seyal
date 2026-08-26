#![cfg(target_os = "macos")]
#![allow(unsafe_code)]

use std::{
    sync::{Mutex, MutexGuard},
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
use seyal_runtime::{
    local_ipc::framing::{ClientHello, FrameHeader, HEADER_LEN, MessageType, encode_frame},
    test_fault::{self, FaultPoint},
};

// All tests in this integration-test binary share one process. FD-baseline
// assertions are meaningful only when sibling tests cannot open/close
// descriptors concurrently.
static TEST_SERIAL: Mutex<()> = Mutex::new(());

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
        LocalIpcMode::Enabled {
            runtime_dir_override: Some(std::env::temp_dir().join(format!(
                "seyal-adversarial-ipc-{}-{suffix:x}-{test}",
                std::process::id()
            ))),
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
                    .create_execution(
                        CommandSpec::new("/bin/sh").args([
                            "-c",
                            "exec 0<&-; exec 1>&-; exec 2>&-; sleep 30",
                        ]),
                        size(),
                    )
                    .expect("live execution"),
            );
        }

        let close_deadline = Instant::now() + Duration::from_secs(2);
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
                runtime.lookup(*id).expect("execution remains tracked").lifecycle,
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
            .create_execution(
                CommandSpec::new("/bin/sh")
                    .args(["-c", "exec 0<&-; exec 1>&-; exec 2>&-; sleep 1"]),
                size(),
            )
            .expect("execution");

        let close_deadline = Instant::now() + Duration::from_secs(2);
        while runtime.input_ingress(id).is_ok() {
            assert!(Instant::now() < close_deadline, "PTY EOF was not observed");
            runtime
                .poll_once(Some(Duration::from_millis(50)))
                .expect("Runtime poll");
        }
        assert_eq!(runtime.lookup(id).unwrap().lifecycle, ExecutionLifecycle::Running);

        let exit_deadline = Instant::now() + Duration::from_secs(3);
        while runtime.lookup(id).is_some() {
            assert!(Instant::now() < exit_deadline, "natural child exit was not observed");
            runtime
                .poll_once(Some(Duration::from_millis(100)))
                .expect("Runtime poll");
        }
    }
    assert_eq!(fd_count(), baseline_fds, "natural PTY EOF path leaked descriptors");
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
            assert!(Instant::now() < deadline, "listener did not recover after pressure");
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
