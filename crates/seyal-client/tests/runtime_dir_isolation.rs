#![cfg(target_os = "macos")]

use std::{
    path::PathBuf,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use seyal_client::{ClientError, DiscoveryFailure, LocalDisplayClient};
use seyal_exec::{CommandSpec, WindowSize};
use seyal_protocol::runtime_dir::{
    control_socket_leaf, override_test_lock, reset_explicit_runtime_dir, set_explicit_runtime_dir,
};
use seyal_runtime::{local_ipc::framing::Role, ExecutionId, Runtime, RuntimeConfig};

struct OverrideReset;

impl Drop for OverrideReset {
    fn drop(&mut self) {
        reset_explicit_runtime_dir();
    }
}

fn start_isolated_runtime() -> (PathBuf, ExecutionId, thread::JoinHandle<()>) {
    let runtime_dir = PathBuf::from(format!(
        "/tmp/s860d{}{:x}",
        std::process::id() % 100_000,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            % 0xFFFF
    ));
    let (ready_tx, ready_rx) = mpsc::channel();
    let join = thread::spawn(move || {
        let mut runtime = Runtime::new(
            RuntimeConfig::m001()
                .expect("M001 Runtime config")
                .isolated_to(runtime_dir),
        )
        .expect("isolated Runtime");
        let execution_id = runtime
            .create_execution(
                CommandSpec::new("/bin/sh").args(["-c", "printf 'SEYAL-860'; sleep 1"]),
                WindowSize::new(80, 24, 0, 0).expect("geometry"),
            )
            .expect("execution");
        let socket_path = runtime
            .local_ipc_socket_path()
            .expect("local IPC socket")
            .to_path_buf();
        ready_tx
            .send((socket_path, execution_id))
            .expect("test receiver");
        let deadline = Instant::now() + Duration::from_secs(5);
        while runtime.execution_count() != 0 && Instant::now() < deadline {
            runtime
                .poll_once(Some(Duration::from_millis(5)))
                .expect("Runtime poll");
        }
        runtime.begin_shutdown().expect("begin shutdown");
        runtime
            .run_until_empty(Instant::now() + Duration::from_secs(2))
            .expect("shutdown");
    });
    let (socket_path, execution_id) = ready_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("Runtime ready");
    (socket_path, execution_id, join)
}

#[test]
fn isolated_client_connects_only_to_its_fixture_and_rejects_the_wrong_socket() {
    let _lock = override_test_lock();
    reset_explicit_runtime_dir();
    let _reset = OverrideReset;
    let (socket_path, execution_id, runtime) = start_isolated_runtime();
    let runtime_dir = socket_path.parent().expect("socket parent").to_path_buf();

    set_explicit_runtime_dir(runtime_dir.clone()).expect("install isolated dir");
    let client = LocalDisplayClient::connect_execution_id(execution_id, Role::Observer)
        .expect("client must attach through the isolated discovery path");
    drop(client);

    reset_explicit_runtime_dir();
    let wrong = PathBuf::from(format!(
        "/tmp/s860w{}{:x}",
        std::process::id() % 100_000,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            % 0xFFFF
    ));
    set_explicit_runtime_dir(wrong).expect("wrong isolated dir");
    let error = match LocalDisplayClient::connect_execution_id(execution_id, Role::Observer) {
        Ok(_) => panic!("wrong directory must not attach to the live fixture"),
        Err(error) => error,
    };
    assert!(
        matches!(
            error,
            ClientError::Discovery(
                DiscoveryFailure::EndpointMissing | DiscoveryFailure::InvalidPath
            )
        ),
        "unexpected discovery error: {error:?}"
    );

    reset_explicit_runtime_dir();
    let client = LocalDisplayClient::connect_execution(&socket_path, execution_id, Role::Observer)
        .expect("direct isolated socket remains usable");
    drop(client);
    assert_eq!(socket_path, control_socket_leaf(&runtime_dir));
    runtime.join().expect("Runtime thread");
}
