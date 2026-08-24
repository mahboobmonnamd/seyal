#![cfg(target_os = "macos")]

use std::time::{Duration, Instant};

use seyal_exec::{CommandSpec, WindowSize};
use seyal_runtime::{Runtime, RuntimeConfig};

#[test]
fn bundled_terminfo_resolves_with_cleared_parent_environment() {
    let mut config = RuntimeConfig::m001().unwrap();
    config.singleton_path = std::env::temp_dir().join(format!(
        "seyal-pass4-clean-terminfo-{}-{}.lock",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    config.final_drain = Duration::from_millis(100);
    let mut runtime = Runtime::new(config).unwrap();
    let id = runtime
        .create_execution(
            CommandSpec::new("/bin/sh")
                .clear_environment()
                .args([
                    "-c",
                    "printf '%s ' \"$TERM\"; /usr/bin/infocmp \"$TERM\" >/dev/null && printf terminfo-clean-ok; sleep 30",
                ]),
            WindowSize::new(80, 24, 0, 0).unwrap(),
        )
        .unwrap();

    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if runtime
            .execution(id)
            .and_then(|execution| execution.terminal().row_text(0))
            .is_some_and(|row| row.contains("seyal-m001") && row.contains("terminfo-clean-ok"))
        {
            break;
        }
        assert!(Instant::now() < deadline, "clean terminfo lookup timed out");
        runtime.poll_once(Some(Duration::from_millis(50))).unwrap();
    }

    runtime.begin_shutdown().unwrap();
    runtime
        .run_until_empty(Instant::now() + Duration::from_secs(2))
        .unwrap();
}
