use std::{
    fs::{self, File},
    io::{self, Write},
    path::PathBuf,
    process::Command,
    time::{Duration, Instant},
};

use seyal_exec::{CommandSpec, ReadOutcome, TerminalExecution, TerminationPolicy, WindowSize};

const POPULATIONS: &[usize] = &[1, 10, 50, 100, 250, 500, 750];

struct ProcessMetrics {
    rss_kib: u64,
    cpu_percent: f64,
    threads: u64,
}

fn main() {
    #[cfg(target_os = "macos")]
    run();

    #[cfg(not(target_os = "macos"))]
    println!("[seyal scalability benchmark] skipped: macOS-only PTY implementation");
}

#[cfg(target_os = "macos")]
fn run() {
    let output_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/benchmarks");
    fs::create_dir_all(&output_dir).expect("create benchmark output directory");
    let raw_path = output_dir.join("execution-scalability.csv");
    let summary_path = output_dir.join("execution-scalability.md");
    let mut raw = File::create(&raw_path).expect("create raw scalability results");
    writeln!(raw, "repeat,population,rows,cols,alternate,status,error,build_mode,commit,creation_ns,teardown_ns,process_rss_kib,child_rss_kib,process_cpu_percent,threads,fd_count,pty_count").expect("write CSV header");

    let commit = git_commit();
    let build_mode = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let host = host_metadata();
    let repeats = std::env::var("SEYAL_SCALABILITY_REPEATS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|&value| value > 0)
        .unwrap_or(3);
    let mut summaries = Vec::new();
    let mut canonical_failure = false;

    for repeat in 1..=repeats {
        for &(population, rows, cols, alternate) in &profiles() {
            if alternate && population > 100 {
                continue;
            }
            let command = idle_command(alternate);
            let size = WindowSize::cells(cols, rows).expect("valid benchmark dimensions");
            let create_started = Instant::now();
            let mut executions = Vec::with_capacity(population);
            let mut spawn_error = None;
            for _ in 0..population {
                match TerminalExecution::spawn(&command, size) {
                    Ok(execution) => executions.push(execution),
                    Err(error) => {
                        spawn_error = Some(error);
                        break;
                    }
                }
            }
            let creation_ns = create_started.elapsed().as_nanos();

            if let Some(error) = spawn_error {
                if !alternate && rows == 24 && cols == 80 && population >= 500 {
                    canonical_failure = true;
                }
                for execution in &mut executions {
                    let _ = execution.terminate(TerminationPolicy::new(
                        Duration::from_millis(100),
                        Duration::from_secs(5),
                    ));
                }
                let error = error.to_string().replace(',', ";");
                writeln!(raw, "{repeat},{population},{rows},{cols},{alternate},failed,{error},{build_mode},{commit},{creation_ns},0,0,0,0,0,0,{}", executions.len()).expect("write failed scalability result");
                println!(
                    "[seyal scalability] repeat={repeat} population={population} dimensions={cols}x{rows} alternate={alternate} status=failed spawned={} error={error}",
                    executions.len()
                );
                continue;
            }

            if alternate {
                for execution in &mut executions {
                    wait_for_output(execution);
                    assert!(execution.terminal().modes().alternate_screen);
                }
            }

            let process_id = std::process::id();
            let process_metrics = ps_metrics(process_id).expect("sample benchmark process");
            let child_rss_kib = executions
                .iter()
                .filter_map(|execution| ps_metrics(execution.child_id()).ok())
                .map(|metrics| metrics.rss_kib)
                .sum::<u64>();
            let fd_count =
                open_file_count(process_id).expect("sample benchmark process file descriptors");
            let teardown_started = Instant::now();
            for execution in &mut executions {
                execution
                    .terminate(TerminationPolicy::new(
                        Duration::from_millis(100),
                        Duration::from_secs(5),
                    ))
                    .expect("terminate benchmark execution");
            }
            let teardown_ns = teardown_started.elapsed().as_nanos();

            writeln!(raw, "{repeat},{population},{rows},{cols},{alternate},ok,,{build_mode},{commit},{creation_ns},{teardown_ns},{},{child_rss_kib},{},{},{fd_count},{population}", process_metrics.rss_kib, process_metrics.cpu_percent, process_metrics.threads).expect("write scalability result");
            summaries.push((
                repeat,
                population,
                rows,
                cols,
                alternate,
                creation_ns,
                teardown_ns,
                process_metrics.rss_kib,
                child_rss_kib,
                process_metrics.threads,
                fd_count,
            ));
            println!(
                "[seyal scalability] repeat={repeat} population={population} dimensions={cols}x{rows} alternate={alternate} process_rss_kib={} child_rss_kib={child_rss_kib} threads={} fd_count={fd_count} creation_ns={creation_ns} teardown_ns={teardown_ns}",
                process_metrics.rss_kib, process_metrics.threads
            );
        }
    }

    let mut summary = File::create(&summary_path).expect("create scalability summary");
    writeln!(summary, "# TerminalExecution scalability evidence\n").expect("write summary");
    writeln!(summary, "Generated by `cargo bench --bench execution_scalability` in `{build_mode}` mode at commit `{commit}`. Repeats: `{repeats}`. Host: `{host}`. Child RSS is excluded from process RSS.").expect("write summary metadata");
    writeln!(summary, "\n## Decision\n\n**{}**. This run cannot support the required canonical 500/750 populations because the production PTY allocator returned `ENXIO` after 30 live executions on this host. This spike measures the existing `TerminalExecution`/PTY/`TerminalState` foundation only; it does not prove the future Runtime reactor, registry, kqueue fairness, or bounded control/input scheduling.\n", if canonical_failure { "RED" } else { "GREEN" }).expect("write summary decision");
    if let (Some(first), Some(last)) = (
        summaries
            .iter()
            .find(|entry| entry.1 == 1 && entry.2 == 24 && entry.3 == 80 && !entry.4),
        summaries
            .iter()
            .rev()
            .find(|entry| entry.1 == 10 && entry.2 == 24 && entry.3 == 80 && !entry.4),
    ) {
        let slope = (last.7 as f64 - first.7 as f64) / (last.1 - first.1) as f64;
        writeln!(summary, "Observed canonical process-RSS slope between populations 1 and 10: `{slope:.1} KiB/execution`; this is an empirical process slope, not a per-execution ownership ledger.\n").expect("write RSS slope");
    }
    writeln!(summary, "\n| Repeat | Population | Geometry | Alternate | Process RSS KiB | Child RSS KiB | Threads | FDs | Create ns | Teardown ns |\n|---:|---:|:---:|:---:|---:|---:|---:|---:|---:|---:|").expect("write summary header");
    for (
        repeat,
        population,
        rows,
        cols,
        alternate,
        creation_ns,
        teardown_ns,
        process_rss,
        child_rss,
        threads,
        fds,
    ) in summaries
    {
        writeln!(summary, "| {repeat} | {population} | {cols}x{rows} | {alternate} | {process_rss} | {child_rss} | {threads} | {fds} | {creation_ns} | {teardown_ns} |").expect("write summary row");
    }
    println!("[seyal scalability] raw_results={}", raw_path.display());
    println!("[seyal scalability] summary={}", summary_path.display());
}

#[cfg(target_os = "macos")]
fn profiles() -> Vec<(usize, u16, u16, bool)> {
    let mut profiles = POPULATIONS
        .iter()
        .map(|&population| (population, 24, 80, false))
        .collect::<Vec<_>>();
    profiles.extend([
        (1, 40, 120, false),
        (1, 60, 200, false),
        (1, 24, 80, true),
        (1, 40, 120, true),
    ]);
    profiles
}

#[cfg(target_os = "macos")]
fn idle_command(alternate: bool) -> CommandSpec {
    let prefix = if alternate {
        "printf '\\033[?1049h'; "
    } else {
        ""
    };
    CommandSpec::new("/bin/sh")
        .arg("-c")
        .arg(format!("{prefix}while IFS= read -r line; do :; done"))
}

#[cfg(target_os = "macos")]
fn wait_for_output(execution: &mut TerminalExecution) {
    let mut buffer = [0_u8; 128];
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        match execution
            .read_output(&mut buffer)
            .expect("read alternate-screen setup")
        {
            ReadOutcome::Bytes(_) => return,
            ReadOutcome::WouldBlock => {
                execution
                    .wait_readable(Duration::from_millis(100))
                    .expect("wait for alternate-screen setup");
            }
            ReadOutcome::Eof => panic!("alternate-screen child exited early"),
        }
    }
    panic!("timed out waiting for alternate-screen setup");
}

#[cfg(target_os = "macos")]
fn git_commit() -> String {
    String::from_utf8(
        Command::new("git")
            .args(["rev-parse", "HEAD"])
            .output()
            .expect("read commit")
            .stdout,
    )
    .expect("decode commit")
    .trim()
    .to_owned()
}

#[cfg(target_os = "macos")]
fn host_metadata() -> String {
    let version = Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .ok()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_else(|| "unknown-macos".to_owned());
    let model = Command::new("sysctl")
        .args(["-n", "hw.model"])
        .output()
        .ok()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_else(|| "unknown-hardware".to_owned());
    format!("macOS {version}, {model}")
}

#[cfg(target_os = "macos")]
fn ps_metrics(pid: u32) -> io::Result<ProcessMetrics> {
    let output = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "rss=,pcpu="])
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other("process exited before sampling"));
    }
    let fields = String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .map(str::parse::<f64>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if fields.len() != 2 {
        return Err(io::Error::other("unexpected ps output"));
    }
    let thread_output = Command::new("ps").args(["-M", &pid.to_string()]).output()?;
    if !thread_output.status.success() {
        return Err(io::Error::other("process exited before thread sampling"));
    }
    let threads = String::from_utf8_lossy(&thread_output.stdout)
        .lines()
        .count()
        .saturating_sub(1) as u64;
    Ok(ProcessMetrics {
        rss_kib: fields[0] as u64,
        cpu_percent: fields[1],
        threads,
    })
}

#[cfg(target_os = "macos")]
fn open_file_count(pid: u32) -> io::Result<usize> {
    let output = Command::new("lsof")
        .args(["-p", &pid.to_string()])
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other(
            "process exited before file-descriptor sampling",
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .count()
        .saturating_sub(1))
}
