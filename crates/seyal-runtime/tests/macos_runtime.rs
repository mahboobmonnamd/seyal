#![cfg(target_os = "macos")]

use std::{
    sync::{Arc, Barrier},
    thread,
    time::{Duration, Instant},
};

use seyal_exec::{CommandSpec, WindowSize};
use seyal_runtime::{
    ExecutionLifecycle, LocalIpcMode, Runtime, RuntimeConfig, RuntimeError,
};

fn config(test: &str) -> RuntimeConfig {
    let mut config = RuntimeConfig::m001().expect("bundled capability profile");
    config.singleton_path = std::env::temp_dir().join(format!(
        "seyal-pass4-{}-{}-{test}.lock",
        std::process::id(),
        unique_suffix()
    ));
    // Retained Pass-4 Runtime tests exercise headless execution ownership,
    // lifecycle, fairness, TERM/terminfo and in-process logical attachments.
    // Pass-5 local IPC has its own real-socket integration suite; keeping it
    // disabled here prevents parallel Pass-4 test Runtimes from competing for
    // the production single per-user control socket.
    config.local_ipc = LocalIpcMode::Disabled;
    config.graceful_termination = Duration::from_millis(50);
    config.forced_reap = Duration::from_millis(250);
    config.final_drain = Duration::from_millis(100);
    config
}

fn unique_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}

fn size() -> WindowSize {
    WindowSize::new(80, 24, 0, 0).expect("valid size")
}

fn wait_until(runtime: &mut Runtime, deadline: Instant, predicate: impl Fn(&Runtime) -> bool) {
    while !predicate(runtime) {
        assert!(Instant::now() < deadline, "condition timed out");
        runtime
            .poll_once(Some(Duration::from_millis(50)))
            .expect("Runtime poll");
    }
}

fn shutdown(runtime: &mut Runtime) {
    runtime.begin_shutdown().expect("begin controlled shutdown");
    runtime
        .run_until_empty(Instant::now() + Duration::from_secs(2))
        .expect("controlled shutdown completes");
    assert_eq!(runtime.aggregate_accepted_but_unwritten_bytes(), 0);
}

#[test]
fn headless_detach_preserves_execution_identity_and_terminal_state() {
    let mut runtime = Runtime::new(config("detach")).expect("headless Runtime");
    let runtime_id = runtime.id();
    let workspace = runtime.default_workspace_id();
    let id = runtime
        .create_execution(
            CommandSpec::new("/bin/sh").args(["-c", "printf 'persisted'; sleep 30"]),
            size(),
        )
        .expect("live execution");
    assert_eq!(runtime.lookup(id).unwrap().workspace_id, workspace);

    let first = runtime.attach(id).expect("first attachment");
    wait_until(
        &mut runtime,
        Instant::now() + Duration::from_secs(2),
        |runtime| {
            runtime
                .execution(id)
                .and_then(|execution| execution.terminal().row_text(0))
                .is_some_and(|row| row.contains("persisted"))
        },
    );
    let generation = runtime
        .execution(id)
        .unwrap()
        .terminal()
        .damage_generation();

    runtime.detach(id, first).expect("detach presentation");
    assert_eq!(runtime.lookup(id).unwrap().attachment_count, 0);
    assert_eq!(
        runtime.lookup(id).unwrap().lifecycle,
        ExecutionLifecycle::Running
    );
    assert_eq!(runtime.id(), runtime_id);
    assert!(runtime.execution(id).is_some());

    let second = runtime.attach(id).expect("reattach");
    assert_eq!(runtime.lookup(id).unwrap().attachment_count, 1);
    assert_eq!(
        runtime
            .execution(id)
            .unwrap()
            .terminal()
            .damage_generation(),
        generation
    );
    runtime.detach(id, second).expect("second detach");
    shutdown(&mut runtime);
}

#[test]
fn singleton_uses_live_lock_not_stale_file_metadata() {
    let config = config("singleton");
    let first = Runtime::new(config.clone()).expect("first Runtime owns scope");
    assert!(matches!(
        Runtime::new(config.clone()),
        Err(RuntimeError::AlreadyRunning)
    ));
    let first_id = first.id();
    drop(first);

    assert!(config.singleton_path.exists(), "lock metadata may remain");
    let second = Runtime::new(config).expect("stale lock file is not a live owner");
    assert_ne!(first_id, second.id());
}

#[test]
fn runtime_sets_seyal_term_and_bundled_terminfo_resolves() {
    let mut runtime = Runtime::new(config("terminfo")).expect("Runtime");
    let id = runtime
        .create_execution(
            CommandSpec::new("/bin/sh").args([
                "-c",
                "printf '%s ' \"$TERM\"; infocmp \"$TERM\" >/dev/null && printf 'terminfo-ok'; sleep 30",
            ]),
            size(),
        )
        .expect("execution");
    wait_until(
        &mut runtime,
        Instant::now() + Duration::from_secs(2),
        |runtime| {
            runtime
                .execution(id)
                .and_then(|execution| execution.terminal().row_text(0))
                .is_some_and(|row| row.contains("seyal-m001") && row.contains("terminfo-ok"))
        },
    );
    shutdown(&mut runtime);
}

#[test]
fn concurrent_producers_cannot_oversubscribe_runtime_input_budget() {
    let mut config = config("budget");
    config.aggregate_input_bytes = 16;
    config.per_execution_input_bytes = 16;
    config.control_queue_capacity = 32;
    let mut runtime = Runtime::new(config).expect("Runtime");
    let first = runtime
        .create_execution(CommandSpec::new("/bin/cat"), size())
        .expect("first cat");
    let second = runtime
        .create_execution(CommandSpec::new("/bin/cat"), size())
        .expect("second cat");
    let ingress = [
        runtime.input_ingress(first).unwrap(),
        runtime.input_ingress(second).unwrap(),
    ];
    let barrier = Arc::new(Barrier::new(9));
    let mut workers = Vec::new();
    for index in 0..8 {
        let input = ingress[index % 2].clone();
        let barrier = Arc::clone(&barrier);
        workers.push(thread::spawn(move || {
            barrier.wait();
            input.try_submit(vec![b'x'; 4]).is_ok()
        }));
    }
    barrier.wait();
    let accepted = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .filter(|accepted| *accepted)
        .count();
    assert!(accepted <= 4);
    assert!(runtime.aggregate_accepted_but_unwritten_bytes() <= 16);

    wait_until(
        &mut runtime,
        Instant::now() + Duration::from_secs(2),
        |runtime| runtime.aggregate_accepted_but_unwritten_bytes() == 0,
    );
    shutdown(&mut runtime);
}

#[test]
fn read_quantum_prevents_noisy_execution_from_starving_quiet_execution() {
    let mut config = config("read-fairness");
    config.read_dispatch_bytes = 4096;
    let mut runtime = Runtime::new(config).expect("Runtime");
    let noisy = runtime
        .create_execution(
            CommandSpec::new("/bin/sh").args(["-c", "yes N | head -c 1048576; sleep 30"]),
            size(),
        )
        .expect("noisy execution");
    let quiet = runtime
        .create_execution(
            CommandSpec::new("/bin/sh").args(["-c", "printf QUIET; sleep 30"]),
            size(),
        )
        .expect("quiet execution");

    wait_until(
        &mut runtime,
        Instant::now() + Duration::from_secs(2),
        |runtime| {
            runtime
                .execution(quiet)
                .and_then(|execution| execution.terminal().row_text(0))
                .is_some_and(|row| row.contains("QUIET"))
        },
    );
    assert!(runtime.execution(noisy).is_some());
    shutdown(&mut runtime);
}

#[test]
fn write_quantum_advances_two_backlogged_executions_in_one_control_turn() {
    let mut config = config("write-fairness");
    config.write_dispatch_bytes = 1024;
    config.per_execution_input_bytes = 64 * 1024;
    config.aggregate_input_bytes = 128 * 1024;
    let mut runtime = Runtime::new(config).expect("Runtime");
    let first = runtime
        .create_execution(CommandSpec::new("/bin/cat"), size())
        .expect("first cat");
    let second = runtime
        .create_execution(CommandSpec::new("/bin/cat"), size())
        .expect("second cat");
    let first_ingress = runtime.input_ingress(first).unwrap();
    let second_ingress = runtime.input_ingress(second).unwrap();
    first_ingress.try_submit(vec![b'a'; 16 * 1024]).unwrap();
    second_ingress.try_submit(vec![b'b'; 16 * 1024]).unwrap();

    runtime
        .poll_once(Some(Duration::from_secs(1)))
        .expect("control wake is delivered");
    let first_remaining = first_ingress.accepted_but_unwritten_bytes();
    let second_remaining = second_ingress.accepted_but_unwritten_bytes();
    assert!(first_remaining < 16 * 1024);
    assert!(second_remaining < 16 * 1024);
    shutdown(&mut runtime);
}

#[test]
fn immediate_primary_exit_never_becomes_a_stuck_live_execution() {
    let mut runtime = Runtime::new(config("immediate-exit")).expect("Runtime");
    for _ in 0..16 {
        match runtime.create_execution(CommandSpec::new("/usr/bin/true"), size()) {
            Ok(id) => wait_until(
                &mut runtime,
                Instant::now() + Duration::from_secs(1),
                |runtime| runtime.lookup(id).is_none(),
            ),
            Err(RuntimeError::ChildExitedBeforePublication(_)) => {}
            Err(error) => panic!("unexpected immediate-exit result: {error}"),
        }
    }
    assert_eq!(runtime.execution_count(), 0);
}

#[test]
fn descendant_held_slave_cannot_keep_primary_execution_alive() {
    let mut runtime = Runtime::new(config("descendant-drain")).expect("Runtime");
    let id = runtime
        .create_execution(
            CommandSpec::new("/bin/sh").args(["-c", "sleep 30 & printf done"]),
            size(),
        )
        .expect("execution");
    wait_until(
        &mut runtime,
        Instant::now() + Duration::from_secs(2),
        |runtime| runtime.lookup(id).is_none(),
    );
}

#[test]
fn runtime_internal_descriptors_are_not_inherited_by_child() {
    let mut runtime = Runtime::new(config("cloexec")).expect("Runtime");
    let id = runtime
        .create_execution(
            CommandSpec::new("/bin/sh").args([
                "-c",
                "for fd in 3 4 5 6 7 8 9; do if [ -e /dev/fd/$fd ]; then printf 'LEAK:%s ' $fd; fi; done; printf 'checked'; sleep 30",
            ]),
            size(),
        )
        .expect("execution");
    wait_until(
        &mut runtime,
        Instant::now() + Duration::from_secs(2),
        |runtime| {
            runtime
                .execution(id)
                .and_then(|execution| execution.terminal().row_text(0))
                .is_some_and(|row| row.contains("checked"))
        },
    );
    let row = runtime
        .execution(id)
        .unwrap()
        .terminal()
        .row_text(0)
        .unwrap();
    assert!(
        !row.contains("LEAK:"),
        "child inherited Runtime-only fd: {row:?}"
    );
    shutdown(&mut runtime);
}

#[test]
fn resize_and_new_input_are_rejected_after_termination_begins() {
    let mut runtime = Runtime::new(config("post-exit-admission")).expect("Runtime");
    let id = runtime
        .create_execution(CommandSpec::new("/bin/sh").args(["-c", "sleep 30"]), size())
        .expect("execution");
    let ingress = runtime.input_ingress(id).unwrap();
    runtime
        .request_termination(id)
        .expect("termination request");
    assert!(matches!(
        runtime.resize(id, WindowSize::new(100, 30, 0, 0).unwrap()),
        Err(RuntimeError::ExecutionNotRunning)
    ));
    assert!(matches!(
        ingress.try_submit(b"rejected".to_vec()),
        Err(RuntimeError::ExecutionNotRunning)
    ));
    runtime
        .run_until_empty(Instant::now() + Duration::from_secs(2))
        .expect("termination finalizes");
}

#[test]
fn repeated_create_and_controlled_terminate_returns_registry_and_budget_to_zero() {
    let mut runtime = Runtime::new(config("repeated-cleanup")).expect("Runtime");
    for _ in 0..20 {
        let id = runtime
            .create_execution(CommandSpec::new("/bin/sh").args(["-c", "sleep 30"]), size())
            .expect("execution");
        runtime.request_termination(id).unwrap();
        wait_until(
            &mut runtime,
            Instant::now() + Duration::from_secs(1),
            |runtime| runtime.lookup(id).is_none(),
        );
        assert_eq!(runtime.aggregate_accepted_but_unwritten_bytes(), 0);
    }
    assert_eq!(runtime.execution_count(), 0);
}
