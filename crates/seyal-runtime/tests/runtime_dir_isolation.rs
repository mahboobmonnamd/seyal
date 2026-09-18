#![cfg(target_os = "macos")]

use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

use seyal_exec::{CommandSpec, WindowSize};
use seyal_runtime::{LocalIpcMode, Runtime, RuntimeConfig};

fn unique_dir(tag: &str) -> PathBuf {
    PathBuf::from(format!(
        "/tmp/s860{tag}{}{:x}",
        std::process::id() % 100_000,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            % 0xFFFF
    ))
}

fn isolated_config(tag: &str) -> RuntimeConfig {
    RuntimeConfig::m001()
        .expect("M001 Runtime config")
        .isolated_to(unique_dir(tag))
}

fn shutdown(runtime: &mut Runtime) {
    runtime.begin_shutdown().expect("begin shutdown");
    runtime
        .run_until_empty(Instant::now() + Duration::from_secs(2))
        .expect("shutdown");
}

#[test]
fn m001_defaults_remain_the_canonical_user_scope() {
    let config = RuntimeConfig::m001().expect("M001 Runtime config");
    assert!(config.singleton_path.ends_with("seyal/runtime.lock"));
    match config.local_ipc {
        LocalIpcMode::Enabled {
            runtime_dir_override: None,
        } => {}
        other => panic!("production M001 config must not isolate IPC: {other:?}"),
    }
}

#[test]
fn two_isolated_runtimes_coexist_and_do_not_bind_the_canonical_socket() {
    let canonical = seyal_runtime::local_ipc::discovery::darwin_user_runtime_dir()
        .expect("canonical runtime dir")
        .join("control.sock");
    let existed_before = canonical.exists();

    let mut first = Runtime::new(isolated_config("a")).expect("first isolated Runtime");
    let mut second = Runtime::new(isolated_config("b")).expect("second isolated Runtime");
    let first_socket = first
        .local_ipc_socket_path()
        .expect("first socket")
        .to_path_buf();
    let second_socket = second
        .local_ipc_socket_path()
        .expect("second socket")
        .to_path_buf();

    assert_ne!(first_socket, second_socket);
    assert_ne!(first_socket, canonical);
    assert_ne!(second_socket, canonical);
    assert!(first_socket.ends_with("control.sock"));
    assert!(second_socket.ends_with("control.sock"));
    assert_eq!(
        canonical.exists(),
        existed_before,
        "isolated fixtures must not create or remove the user control socket"
    );

    shutdown(&mut first);
    shutdown(&mut second);
}

#[test]
fn incorrect_isolated_endpoint_is_rejected_without_using_production() {
    let mut runtime = Runtime::new(isolated_config("live")).expect("isolated Runtime");
    let live = runtime
        .local_ipc_socket_path()
        .expect("live socket")
        .to_path_buf();
    runtime
        .create_execution(
            CommandSpec::new("/bin/sh").args(["-c", "sleep 30"]),
            WindowSize::new(80, 24, 0, 0).expect("geometry"),
        )
        .expect("execution");

    let deadline = Instant::now() + Duration::from_secs(2);
    while !live.exists() && Instant::now() < deadline {
        runtime
            .poll_once(Some(Duration::from_millis(10)))
            .expect("poll");
    }
    assert!(live.exists(), "isolated control socket must exist");
    std::os::unix::net::UnixStream::connect(&live).expect("live isolated endpoint is connectable");

    let missing = unique_dir("missing").join("control.sock");
    assert!(std::os::unix::net::UnixStream::connect(&missing).is_err());

    shutdown(&mut runtime);
}

#[test]
fn isolated_helper_binary_does_not_occupy_the_canonical_socket() {
    let canonical = seyal_runtime::local_ipc::discovery::darwin_user_runtime_dir()
        .expect("canonical runtime dir")
        .join("control.sock");
    let existed_before = canonical.exists();
    let dir = unique_dir("bin");
    let socket = dir.join("control.sock");
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_seyal-runtime"))
        .args([
            "--runtime-dir",
            dir.to_str().expect("utf8 runtime dir"),
            "/bin/sleep",
            "30",
        ])
        .env("SEYAL_RUNTIME_DIR", "/tmp/seyal-should-ignore")
        .spawn()
        .expect("spawn isolated seyal-runtime");

    let deadline = Instant::now() + Duration::from_secs(3);
    while !socket.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    let started = socket.exists();
    let _ = child.kill();
    let _ = child.wait();
    assert!(started, "isolated helper must bind its own control socket");
    assert_eq!(
        canonical.exists(),
        existed_before,
        "helper --runtime-dir must not bind the user control socket"
    );
}
