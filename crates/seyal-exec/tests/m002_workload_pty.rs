#![cfg(target_os = "macos")]

//! #824 real-PTY workload smoke. One `TerminalExecution` owns one PTY and one
//! `TerminalState`. Optional host binaries that are absent are recorded as
//! `PLATFORM_LIMITED` rather than skipped silently from the matrix doc.

use std::{
    path::PathBuf,
    process::Command,
    sync::{Mutex, MutexGuard, OnceLock},
    time::{Duration, Instant},
};

use seyal_exec::{
    CommandSpec, ExecError, ReadOutcome, TerminalExecution, TerminationPolicy, WindowSize,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());
const IO_TIMEOUT: Duration = Duration::from_secs(5);

fn test_guard() -> MutexGuard<'static, ()> {
    TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn size() -> WindowSize {
    WindowSize::cells(80, 24).expect("valid size")
}

fn compiled_seyal_terminfo() -> PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let out = std::env::temp_dir().join(format!("seyal-824-terminfo-{}", std::process::id()));
        std::fs::create_dir_all(&out).expect("terminfo output dir");
        let src = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../resources/terminfo/seyal-m001.src");
        let status = Command::new("tic")
            .args(["-x", "-o"])
            .arg(&out)
            .arg(&src)
            .status()
            .expect("tic available");
        assert!(status.success(), "tic must compile seyal-m001");
        out
    })
    .clone()
}

fn seyal_term(command: CommandSpec) -> CommandSpec {
    command
        .env("TERM", "seyal-m001")
        .env("TERMINFO", compiled_seyal_terminfo())
}

fn drain_until(
    execution: &mut TerminalExecution,
    needle: &[u8],
    timeout: Duration,
) -> Result<Vec<u8>, ExecError> {
    let deadline = Instant::now() + timeout;
    let mut output = Vec::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let _ = execution.write_protocol_replies(256)?;
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
        let readiness = execution.wait_readable(remaining.min(Duration::from_millis(50)))?;
        if readiness.hangup && !readiness.ready {
            match execution.read_output(&mut buffer)? {
                ReadOutcome::Bytes(count) => output.extend_from_slice(&buffer[..count]),
                ReadOutcome::WouldBlock | ReadOutcome::Eof => {}
            }
        }
    }
}

fn visible_contains(execution: &TerminalExecution, needle: &str) -> bool {
    (0..execution.terminal().rows()).any(|row| {
        execution
            .terminal()
            .row_text(row)
            .is_some_and(|text| text.contains(needle))
    })
}

fn terminate(execution: &mut TerminalExecution) {
    let _ = execution.terminate(TerminationPolicy::new(
        Duration::from_millis(50),
        Duration::from_secs(2),
    ));
}

#[test]
fn zsh_bash_and_optional_fish_print_through_one_pty_vt() {
    let _guard = test_guard();
    for (program, args, marker) in [
        (
            "/bin/zsh",
            &["-f", "-c", "printf 'seyal-zsh-ok\\n'"][..],
            "seyal-zsh-ok",
        ),
        (
            "/bin/bash",
            &["--norc", "--noprofile", "-c", "printf 'seyal-bash-ok\\n'"][..],
            "seyal-bash-ok",
        ),
    ] {
        let command = seyal_term(CommandSpec::new(program).args(args.iter().copied()));
        let mut execution = TerminalExecution::spawn(&command, size()).expect("spawn shell");
        let child = execution.child_id();
        let bytes = drain_until(&mut execution, marker.as_bytes(), IO_TIMEOUT).expect("drain");
        assert!(
            bytes.windows(marker.len()).any(|w| w == marker.as_bytes())
                || visible_contains(&execution, marker),
            "{program} did not emit {marker} through the single PTY/VT"
        );
        assert_eq!(execution.child_id(), child);
        terminate(&mut execution);
    }

    let Some(fish) = which("fish") else {
        eprintln!("PLATFORM_LIMITED: fish not installed; zsh/bash PTY rows still ran");
        return;
    };
    let command = seyal_term(CommandSpec::new(&fish).args([
        "--no-config",
        "-c",
        "printf 'seyal-fish-ok\\n'",
    ]));
    let mut execution = TerminalExecution::spawn(&command, size()).expect("spawn fish");
    let bytes = drain_until(&mut execution, b"seyal-fish-ok", IO_TIMEOUT).expect("drain fish");
    assert!(
        bytes.windows(13).any(|w| w == b"seyal-fish-ok")
            || visible_contains(&execution, "seyal-fish-ok"),
        "fish did not emit through the single PTY/VT"
    );
    terminate(&mut execution);
}

#[test]
fn git_representative_color_output_feeds_canonical_state() {
    let _guard = test_guard();
    let Some(git) = which("git") else {
        eprintln!("PLATFORM_LIMITED: git not installed");
        return;
    };
    let command = seyal_term(CommandSpec::new(&git).args([
        "--no-pager",
        "-c",
        "color.ui=always",
        "--version",
    ]));
    let mut execution = TerminalExecution::spawn(&command, size()).expect("spawn git");
    let bytes = drain_until(&mut execution, b"git version", IO_TIMEOUT).expect("drain git");
    assert!(
        bytes.windows(11).any(|w| w == b"git version")
            || visible_contains(&execution, "git version"),
        "git --version did not reach TerminalState"
    );
    terminate(&mut execution);
}

#[test]
fn tmux_as_child_owns_one_pty_when_present() {
    let _guard = test_guard();
    let Some(tmux) = which("tmux") else {
        eprintln!("PLATFORM_LIMITED: tmux not installed; VT tmux-child fixture remains");
        return;
    };
    let socket = format!("seyal824-{}-{}", std::process::id(), {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    });
    struct TmuxServerGuard(String);
    impl Drop for TmuxServerGuard {
        fn drop(&mut self) {
            let _ = std::process::Command::new("tmux")
                .args(["-L", &self.0, "kill-server"])
                .status();
        }
    }
    let _tmux_guard = TmuxServerGuard(socket.clone());
    let command = seyal_term(CommandSpec::new(&tmux).args([
        "-L",
        &socket,
        "-f",
        "/dev/null",
        "new-session",
        "-x",
        "80",
        "-y",
        "24",
        "printf seyal-tmux-child; sleep 2",
    ]));
    let mut execution = TerminalExecution::spawn(&command, size()).expect("spawn tmux child");
    let child = execution.child_id();
    let bytes = drain_until(&mut execution, b"seyal-tmux-child", Duration::from_secs(8))
        .expect("drain tmux");
    assert_eq!(
        execution.child_id(),
        child,
        "tmux must not create a second Seyal PTY"
    );
    assert!(
        bytes.windows(16).any(|w| w == b"seyal-tmux-child")
            || visible_contains(&execution, "seyal-tmux-child"),
        "tmux-as-child must emit the child marker on the one Seyal PTY; alternate-screen alone is not enough"
    );
    terminate(&mut execution);
}

#[test]
fn high_volume_pty_output_stays_one_execution_and_feeds_vt() {
    let _guard = test_guard();
    let command = CommandSpec::new("/bin/sh").args([
        "-c",
        "i=0; while [ \"$i\" -lt 200 ]; do printf 'hv-%03d log-line\\n' \"$i\"; i=$((i+1)); done; printf 'hv-done\\n'",
    ]);
    let mut execution = TerminalExecution::spawn(&command, size()).expect("spawn high-volume");
    let child = execution.child_id();
    let bytes = drain_until(&mut execution, b"hv-done", IO_TIMEOUT).expect("drain high-volume");
    assert_eq!(execution.child_id(), child);
    assert!(bytes.windows(7).any(|w| w == b"hv-done") || visible_contains(&execution, "hv-done"));
    assert!(
        execution
            .terminal()
            .primary_history_search("hv-000", 4)
            .len()
            + usize::from(visible_contains(&execution, "hv-000"))
            >= 1,
        "high-volume PTY bytes must enter canonical history or the live grid"
    );
    terminate(&mut execution);
}

#[test]
fn optional_docker_kubectl_terraform_are_recorded_when_absent() {
    let _guard = test_guard();
    let mut ran = 0usize;
    for (name, args, needle) in [
        ("docker", &["--version"][..], &b"Docker"[..]),
        (
            "kubectl",
            &["version", "--client", "--short"][..],
            &b"Client"[..],
        ),
        ("terraform", &["version"][..], &b"Terraform"[..]),
    ] {
        let Some(bin) = which(name) else {
            eprintln!("PLATFORM_LIMITED: {name} not installed");
            continue;
        };
        let command = seyal_term(CommandSpec::new(&bin).args(args.iter().copied()));
        let mut execution = TerminalExecution::spawn(&command, size()).expect("spawn devops cli");
        let bytes = drain_until(&mut execution, needle, IO_TIMEOUT).expect("drain devops");
        assert!(
            bytes.windows(needle.len()).any(|w| w == needle)
                || std::str::from_utf8(&bytes).is_ok_and(|text| text
                    .to_ascii_lowercase()
                    .contains(&name.to_ascii_lowercase())),
            "{name} produced no representative output"
        );
        terminate(&mut execution);
        ran += 1;
    }
    if ran == 0 {
        eprintln!(
            "PLATFORM_LIMITED: docker/kubectl/terraform all absent; ANSI CLI VT fixture remains"
        );
    }
}
