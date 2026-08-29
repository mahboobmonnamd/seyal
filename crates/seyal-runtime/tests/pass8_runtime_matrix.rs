#![cfg(target_os = "macos")]

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use seyal_exec::{CommandSpec, WindowSize};
use seyal_runtime::{LocalIpcMode, Runtime, RuntimeConfig};

fn size(columns: u16, rows: u16) -> WindowSize {
    WindowSize::new(columns, rows, 0, 0).expect("valid terminal size")
}

fn config(label: &str) -> RuntimeConfig {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let mut config = RuntimeConfig::m001().expect("M001 config");
    config.singleton_path = std::env::temp_dir().join(format!(
        "seyal-pass8-{label}-{}-{nonce:x}.lock",
        std::process::id()
    ));
    config.local_ipc = LocalIpcMode::Disabled;
    config.graceful_termination = Duration::from_millis(50);
    config.forced_reap = Duration::from_millis(250);
    config.final_drain = Duration::from_millis(100);
    config
}

fn stop(runtime: &mut Runtime) {
    runtime.begin_shutdown().expect("begin shutdown");
    runtime
        .run_until_empty(Instant::now() + Duration::from_secs(15))
        .expect("runtime drains");
    assert_eq!(runtime.execution_count(), 0);
    assert_eq!(runtime.block_count(), 0);
}

#[test]
fn block_anchor_and_identity_survive_scroll_alt_screen_resize_and_detach_reattach() {
    let mut runtime = Runtime::new(config("anchor-matrix")).expect("runtime");
    let execution_id = runtime
        .create_execution(
            CommandSpec::new("/bin/sh").args([
                "-c",
                "printf '\033[?1049hALT\033[?1049l'; i=0; while [ $i -lt 256 ]; do printf 'line-%s\\n' \"$i\"; i=$((i+1)); done; sleep 5",
            ]),
            size(16, 4),
        )
        .expect("execution");

    let original = runtime.block(execution_id).expect("current block");
    let first_attachment = runtime.attach(execution_id).expect("attach");

    for _ in 0..20 {
        runtime
            .poll_once(Some(Duration::from_millis(10)))
            .expect("poll output");
    }

    runtime
        .resize(execution_id, size(120, 40))
        .expect("resize");
    runtime
        .detach(execution_id, first_attachment)
        .expect("detach");
    let second_attachment = runtime.attach(execution_id).expect("reattach");

    let after = runtime.block(execution_id).expect("block after mutations");
    assert_eq!(after.id, original.id);
    assert_eq!(after.execution_id, original.execution_id);
    assert_eq!(after.workspace_id, original.workspace_id);
    assert_eq!(after.start_line_id, original.start_line_id);
    assert_eq!(after.revision, original.revision);
    assert_eq!(after.lifecycle, original.lifecycle);

    runtime
        .detach(execution_id, second_attachment)
        .expect("second detach");
    stop(&mut runtime);
}

#[test]
fn block_identity_is_not_reused_across_runtime_incarnations() {
    let first_id = {
        let mut runtime = Runtime::new(config("runtime-a")).expect("runtime A");
        let execution_id = runtime
            .create_execution(
                CommandSpec::new("/bin/sh").args(["-c", "sleep 5"]),
                size(80, 24),
            )
            .expect("execution A");
        let id = runtime.block(execution_id).expect("block A").id;
        stop(&mut runtime);
        id
    };

    let second_id = {
        let mut runtime = Runtime::new(config("runtime-b")).expect("runtime B");
        let execution_id = runtime
            .create_execution(
                CommandSpec::new("/bin/sh").args(["-c", "sleep 5"]),
                size(80, 24),
            )
            .expect("execution B");
        let id = runtime.block(execution_id).expect("block B").id;
        stop(&mut runtime);
        id
    };

    assert_ne!(first_id, second_id);
}

#[test]
fn real_runtime_population_lifecycle_covers_1_10_50_100_and_512_records() {
    for population in [1usize, 10, 50, 100, 512] {
        let mut runtime = Runtime::new(config(&format!("population-{population}"))).expect("runtime");
        for _ in 0..population {
            runtime
                .create_execution(
                    CommandSpec::new("/bin/sh").args(["-c", "sleep 5"]),
                    size(80, 24),
                )
                .expect("execution admission");
        }
        assert_eq!(runtime.execution_count(), population);
        assert_eq!(runtime.block_count(), population);
        stop(&mut runtime);
    }
}
