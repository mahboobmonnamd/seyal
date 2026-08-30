use std::{
    env,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use super::config::{
    COHORT_PROCESS_TIMEOUT, COHORTS, Geometry, LossMode, MEASURED_CYCLES, QUIESCENT_SAMPLE_COUNT,
    QUIESCENT_SAMPLE_INTERVAL, WARMUP_CYCLES,
};
use super::process_io::spawn_line_reader;
use crate::PERFORMANCE_CLAIM;

pub(crate) fn run_orchestrator() {
    println!(
        "pass9_preimplementation_calibration architecture=separate_client_cohort_process_plus_fresh_Runtime_worker_process production_Runtime_local_IPC_PTY warmup_cycles={} measured_cycles={} cohorts={} geometries=120x40,80x24 percentile_method=nearest_rank rss_samples={} rss_interval_ms={} cleanup_measurement=Runtime_disconnect_or_Detach_dispatch_to_attachment_controller_cleanup exact_dispatch_timer=true {}",
        WARMUP_CYCLES,
        MEASURED_CYCLES,
        COHORTS,
        QUIESCENT_SAMPLE_COUNT,
        QUIESCENT_SAMPLE_INTERVAL.as_millis(),
        PERFORMANCE_CLAIM,
    );
    print_host_metadata();

    for geometry in [Geometry::PRIMARY, Geometry::REPRESENTATIVE] {
        for mode in [LossMode::Graceful, LossMode::Abrupt] {
            let mut reconnect_p99 = Vec::with_capacity(COHORTS);
            let mut renderer_ready_p99 = Vec::with_capacity(COHORTS);
            let mut cleanup_p99 = Vec::with_capacity(COHORTS);
            let mut runtime_rss_delta = Vec::with_capacity(COHORTS);
            let mut client_rss_delta = Vec::with_capacity(COHORTS);

            for cohort in 1..=COHORTS {
                let result = run_cohort_process(mode, geometry, cohort);
                reconnect_p99.push(result.reconnect_p99_us);
                renderer_ready_p99.push(result.renderer_ready_p99_us);
                cleanup_p99.push(result.cleanup_p99_us);
                runtime_rss_delta.push(result.runtime_rss_delta_kib);
                client_rss_delta.push(result.client_rss_delta_kib);
            }

            reconnect_p99.sort_by(|a, b| a.total_cmp(b));
            renderer_ready_p99.sort_by(|a, b| a.total_cmp(b));
            cleanup_p99.sort_by(|a, b| a.total_cmp(b));
            runtime_rss_delta.sort_unstable();
            client_rss_delta.sort_unstable();
            println!(
                "pass9_calibration_summary mode={} geometry={} reconnect_boundary=local_connect_hello_resolve_attach_to_complete_authoritative_client_commit median_cohort_p99_us={:.3} renderer_ready_boundary=committed_client_state_to_PreparedSurface_ready median_renderer_cohort_p99_us={:.3} cleanup_boundary=Runtime_disconnect_or_Detach_dispatch_to_attachment_controller_cleanup median_cleanup_cohort_p99_us={:.3} cleanup_classification=MEASURED_EXACT_RUNTIME_DISPATCH runtime_median_cohort_rss_delta_kib={} client_median_cohort_rss_delta_kib={} cohorts={} cycles_per_cohort={} {}",
                mode.label(),
                geometry.label(),
                reconnect_p99[COHORTS / 2],
                renderer_ready_p99[COHORTS / 2],
                cleanup_p99[COHORTS / 2],
                runtime_rss_delta[COHORTS / 2],
                client_rss_delta[COHORTS / 2],
                COHORTS,
                MEASURED_CYCLES,
                PERFORMANCE_CLAIM,
            );
        }
    }
}

#[derive(Clone, Copy)]
struct CohortResult {
    reconnect_p99_us: f64,
    renderer_ready_p99_us: f64,
    cleanup_p99_us: f64,
    runtime_rss_delta_kib: i64,
    client_rss_delta_kib: i64,
}

// Spawns one isolated `--cohort` process, streaming its stdout/stderr live
// instead of buffering to the end via `Command::output()`. `output()` blocks
// until both pipes see EOF, which requires every process holding a copy of
// those fds to exit — including any inherited-fd grandchild the cohort
// process spawned. A cohort-side panic that leaves such a grandchild alive
// would otherwise hang this wait forever with no diagnostic (see
// `worker::RuntimeWorker::start` for the matching fix on the other end of
// that hazard). Polling `try_wait` alongside a deadline bounds this
// regardless of what goes wrong downstream.
fn run_cohort_process(mode: LossMode, geometry: Geometry, cohort: usize) -> CohortResult {
    let mut child = Command::new(env::current_exe().expect("benchmark executable"))
        .arg("--cohort")
        .arg(mode.label())
        .arg(geometry.label())
        .arg(cohort.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn isolated client cohort process");
    let stdout_rx = spawn_line_reader(child.stdout.take().expect("cohort stdout"));
    let stderr_rx = spawn_line_reader(child.stderr.take().expect("cohort stderr"));

    let mut result_line = None;
    let mut record = |line: String| {
        if line.starts_with("PASS9_RESULT ") {
            result_line = Some(line.clone());
        }
        println!("{line}");
    };
    let deadline = Instant::now() + COHORT_PROCESS_TIMEOUT;
    let status = loop {
        let mut progressed = false;
        while let Ok(line) = stdout_rx.try_recv() {
            progressed = true;
            record(line);
        }
        while let Ok(line) = stderr_rx.try_recv() {
            progressed = true;
            eprintln!("{line}");
        }
        if let Some(status) = child.try_wait().expect("cohort wait") {
            // The reader threads exit on EOF once the process is gone, so
            // draining the remainder here cannot block.
            while let Ok(line) = stdout_rx.recv() {
                record(line);
            }
            while let Ok(line) = stderr_rx.recv() {
                eprintln!("{line}");
            }
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "Pass 9 isolated cohort mode={} geometry={} cohort={cohort} did not exit within {COHORT_PROCESS_TIMEOUT:?}",
                mode.label(),
                geometry.label()
            );
        }
        if !progressed {
            thread::sleep(Duration::from_millis(20));
        }
    };
    assert!(
        status.success(),
        "Pass 9 isolated cohort failed: mode={} geometry={} cohort={cohort}",
        mode.label(),
        geometry.label()
    );
    let line = result_line.expect("cohort result line");
    parse_result_line(&line)
}

fn parse_result_line(line: &str) -> CohortResult {
    let field = |name: &str| -> &str {
        line.split_whitespace()
            .find_map(|part| part.strip_prefix(&format!("{name}=")))
            .unwrap_or_else(|| panic!("missing cohort result field {name}"))
    };
    CohortResult {
        reconnect_p99_us: field("reconnect_p99_us").parse().expect("reconnect p99"),
        renderer_ready_p99_us: field("renderer_ready_p99_us")
            .parse()
            .expect("renderer p99"),
        cleanup_p99_us: field("cleanup_p99_us").parse().expect("cleanup p99"),
        runtime_rss_delta_kib: field("runtime_rss_delta_kib")
            .parse()
            .expect("runtime RSS delta"),
        client_rss_delta_kib: field("client_rss_delta_kib")
            .parse()
            .expect("client RSS delta"),
    }
}

fn print_host_metadata() {
    let product = command_text("/usr/bin/sw_vers", &["-productVersion"]);
    let build = command_text("/usr/bin/sw_vers", &["-buildVersion"]);
    let model = command_text("/usr/sbin/sysctl", &["-n", "hw.model"]);
    let hardware = command_text("/usr/sbin/sysctl", &["-n", "machdep.cpu.brand_string"]);
    let rust = command_text("rustc", &["--version"]);
    let commit = command_text("git", &["rev-parse", "HEAD"]);
    let master_baseline = git_merge_base_with_master();
    println!(
        "pass9_calibration_host macos_version={} macos_build={} model={:?} hardware={:?} arch={} rust={:?} build_mode=release commit={} master_baseline={} pass8_baseline=d9d21187e8429bbd3dbeb3e1c7cc4d05c1d147e6 {}",
        product,
        build,
        model,
        hardware,
        env::consts::ARCH,
        rust,
        commit,
        master_baseline,
        PERFORMANCE_CLAIM,
    );
}

fn command_text(program: &str, args: &[&str]) -> String {
    Command::new(program)
        .args(args)
        .output()
        .ok()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_else(|| "unavailable".to_owned())
}

// The fork point this calibration ran against, derived at run time. A fixed
// literal SHA here would go stale silently the moment this branch rebases or
// `master` advances, misattributing the evidence in a later re-run — unlike
// `pass8_baseline` above, which is a fixed historical fact (Pass 8 is already
// merged and never moves) and is correctly a constant.
fn git_merge_base_with_master() -> String {
    for reference in ["origin/master", "master"] {
        let Ok(output) = Command::new("git")
            .args(["merge-base", "HEAD", reference])
            .output()
        else {
            continue;
        };
        if !output.status.success() {
            continue;
        }
        let sha = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if !sha.is_empty() {
            return sha;
        }
    }
    "unavailable".to_owned()
}
