#![cfg(target_os = "macos")]

use std::time::{Duration, Instant};

use seyal_exec::{CommandSpec, WindowSize};
use seyal_runtime::{ExecutionLifecycle, Runtime, RuntimeConfig, RuntimeError};

fn config(test: &str) -> RuntimeConfig {
    let mut config = RuntimeConfig::m001().expect("bundled capability profile");
    config.singleton_path = std::env::temp_dir().join(format!(
        "seyal-pass4-failure-{}-{}-{test}.lock",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    config.graceful_termination = Duration::from_millis(25);
    config.forced_reap = Duration::from_millis(500);
    config.final_drain = Duration::from_millis(100);
    config
}

fn size() -> WindowSize {
    WindowSize::new(80, 24, 0, 0).unwrap()
}

fn live_shell(runtime: &mut Runtime) -> seyal_runtime::ExecutionId {
    runtime
        .create_execution(CommandSpec::new("/bin/sh").args(["-c", "sleep 30"]), size())
        .unwrap()
}

fn shutdown(runtime: &mut Runtime) {
    runtime.begin_shutdown().unwrap();
    runtime
        .run_until_empty(Instant::now() + Duration::from_secs(2))
        .unwrap();
}

#[test]
fn invalid_command_rolls_back_registry_and_workspace_publication() {
    let mut runtime = Runtime::new(config("invalid-command")).unwrap();
    assert!(
        runtime
            .create_execution(CommandSpec::new("/definitely/not/a/seyal-command"), size())
            .is_err()
    );
    assert_eq!(runtime.execution_count(), 0);
    assert!(runtime.list().is_empty());
    assert_eq!(runtime.aggregate_accepted_but_unwritten_bytes(), 0);
}

#[test]
fn multiple_attachments_are_independent_and_duplicate_detach_is_rejected() {
    let mut runtime = Runtime::new(config("attachments")).unwrap();
    let id = live_shell(&mut runtime);
    let first = runtime.attach(id).unwrap();
    let second = runtime.attach(id).unwrap();
    assert_eq!(runtime.lookup(id).unwrap().attachment_count, 2);
    runtime.detach(id, first).unwrap();
    assert_eq!(runtime.lookup(id).unwrap().attachment_count, 1);
    assert!(matches!(
        runtime.detach(id, first),
        Err(RuntimeError::UnknownAttachment)
    ));
    runtime.detach(id, second).unwrap();
    assert_eq!(runtime.lookup(id).unwrap().attachment_count, 0);
    assert_eq!(
        runtime.lookup(id).unwrap().lifecycle,
        ExecutionLifecycle::Running
    );
    shutdown(&mut runtime);
}

#[test]
fn per_execution_input_limit_rejects_without_leaking_global_reservation() {
    let mut config = config("per-exec-budget");
    config.per_execution_input_bytes = 8;
    config.aggregate_input_bytes = 64;
    let mut runtime = Runtime::new(config).unwrap();
    let id = runtime
        .create_execution(CommandSpec::new("/bin/cat"), size())
        .unwrap();
    let ingress = runtime.input_ingress(id).unwrap();
    assert!(matches!(
        ingress.try_submit(vec![b'x'; 9]),
        Err(RuntimeError::InputBackpressure)
    ));
    assert_eq!(ingress.accepted_but_unwritten_bytes(), 0);
    assert_eq!(runtime.aggregate_accepted_but_unwritten_bytes(), 0);
    shutdown(&mut runtime);
}

#[test]
fn control_queue_full_rolls_back_second_reservation() {
    let mut config = config("queue-rollback");
    config.control_queue_capacity = 1;
    config.per_execution_input_bytes = 64;
    config.aggregate_input_bytes = 64;
    let mut runtime = Runtime::new(config).unwrap();
    let first = runtime
        .create_execution(CommandSpec::new("/bin/cat"), size())
        .unwrap();
    let second = runtime
        .create_execution(CommandSpec::new("/bin/cat"), size())
        .unwrap();
    let first_ingress = runtime.input_ingress(first).unwrap();
    let second_ingress = runtime.input_ingress(second).unwrap();

    first_ingress.try_submit(vec![b'a'; 8]).unwrap();
    assert!(matches!(
        second_ingress.try_submit(vec![b'b'; 8]),
        Err(RuntimeError::ControlQueueFull)
    ));
    assert_eq!(second_ingress.accepted_but_unwritten_bytes(), 0);
    assert_eq!(runtime.aggregate_accepted_but_unwritten_bytes(), 8);

    runtime.poll_once(Some(Duration::from_secs(1))).unwrap();
    assert_eq!(runtime.aggregate_accepted_but_unwritten_bytes(), 0);
    shutdown(&mut runtime);
}

#[test]
fn graceful_deadline_escalates_to_forced_without_sleeping_reactor() {
    let mut runtime = Runtime::new(config("forced-transition")).unwrap();
    let id = runtime
        .create_execution(
            CommandSpec::new("/bin/sh")
                .args(["-c", "trap '' TERM; printf READY; while :; do sleep 1; done"]),
            size(),
        )
        .unwrap();

    let ready_deadline = Instant::now() + Duration::from_secs(1);
    loop {
        if runtime
            .execution(id)
            .and_then(|execution| execution.terminal().row_text(0))
            .is_some_and(|row| row.contains("READY"))
        {
            break;
        }
        assert!(Instant::now() < ready_deadline, "TERM trap was not established");
        runtime.poll_once(Some(Duration::from_millis(50))).unwrap();
    }

    runtime.request_termination(id).unwrap();
    assert_eq!(
        runtime.lookup(id).unwrap().lifecycle,
        ExecutionLifecycle::TerminatingGraceful
    );

    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        runtime.poll_once(Some(Duration::from_millis(50))).unwrap();
        match runtime.lookup(id).map(|summary| summary.lifecycle) {
            Some(ExecutionLifecycle::TerminatingForced) => break,
            None => panic!("execution finalized before forced state was observable"),
            _ if Instant::now() < deadline => {}
            state => panic!("forced transition not observed: {state:?}"),
        }
    }
    runtime
        .run_until_empty(Instant::now() + Duration::from_secs(2))
        .unwrap();
}

#[test]
fn shutdown_discards_pending_input_and_releases_reservations() {
    let mut config = config("shutdown-pending");
    config.per_execution_input_bytes = 1024 * 1024;
    config.aggregate_input_bytes = 1024 * 1024;
    let mut runtime = Runtime::new(config).unwrap();
    let id = runtime
        .create_execution(CommandSpec::new("/bin/cat"), size())
        .unwrap();
    let ingress = runtime.input_ingress(id).unwrap();
    ingress.try_submit(vec![b'x'; 128 * 1024]).unwrap();
    assert!(runtime.aggregate_accepted_but_unwritten_bytes() > 0);
    shutdown(&mut runtime);
    assert_eq!(runtime.aggregate_accepted_but_unwritten_bytes(), 0);
    assert!(matches!(
        ingress.try_submit(vec![b'x']),
        Err(RuntimeError::ExecutionNotRunning)
    ));
}
