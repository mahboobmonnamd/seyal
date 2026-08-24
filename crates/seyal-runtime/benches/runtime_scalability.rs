#[cfg(target_os = "macos")]
use std::{env, process::Command, time::Instant};

#[cfg(target_os = "macos")]
use seyal_exec::{CommandSpec, WindowSize};
#[cfg(target_os = "macos")]
use seyal_runtime::{LocalIpcMode, Runtime, RuntimeConfig};
#[cfg(target_os = "macos")]
use std::{fs, process, thread, time::Duration};

#[cfg(target_os = "macos")]
#[derive(Clone, Copy)]
enum TransportMode {
    SocketOnly,
    HybridProjection,
}

#[cfg(target_os = "macos")]
impl TransportMode {
    fn from_arg(value: &str) -> Self {
        match value {
            "socket-only" => Self::SocketOnly,
            "hybrid" => Self::HybridProjection,
            _ => panic!("unknown transport mode {value}"),
        }
    }

    fn as_arg(self) -> &'static str {
        match self {
            Self::SocketOnly => "socket-only",
            Self::HybridProjection => "hybrid",
        }
    }
}

fn main() {
    #[cfg(not(target_os = "macos"))]
    {
        println!(
            "seyal-runtime scalability: PLATFORM_LIMITED target_os!=macos; no performance claim"
        );
    }

    #[cfg(target_os = "macos")]
    run_macos();
}

#[cfg(target_os = "macos")]
fn run_macos() {
    if env::args().nth(1).as_deref() == Some("--worker") {
        worker();
        return;
    }

    println!("seyal-runtime Pass 5 transport comparator benchmark; performance_claim=false");
    println!("method=fresh_worker_per_population_and_transport release_bench_profile=true");
    println!("profile=80x24 primary-active alternate-inactive minimal-scrollback");
    print_host_metadata();

    let executable = env::current_exe().expect("current benchmark executable");
    for transport in [TransportMode::SocketOnly, TransportMode::HybridProjection] {
        for population in [1usize, 10, 50, 100] {
            let status = Command::new(&executable)
                .args([
                    "--worker",
                    &population.to_string(),
                    "80",
                    "24",
                    "primary",
                    transport.as_arg(),
                ])
                .status()
                .expect("launch fresh benchmark worker");
            assert!(status.success(), "runtime benchmark worker failed");
        }
    }

    for transport in [TransportMode::SocketOnly, TransportMode::HybridProjection] {
        for (columns, rows, screen) in [
            (120, 40, "primary"),
            (200, 60, "primary"),
            (80, 24, "alternate"),
        ] {
            let status = Command::new(&executable)
                .args([
                    "--worker",
                    "1",
                    &columns.to_string(),
                    &rows.to_string(),
                    screen,
                    transport.as_arg(),
                ])
                .status()
                .expect("launch representative geometry worker");
            assert!(status.success(), "representative geometry worker failed");
        }
    }
}

#[cfg(target_os = "macos")]
fn worker() {
    let args = env::args().collect::<Vec<_>>();
    let requested = args[2].parse::<usize>().expect("population");
    let columns = args[3].parse::<u16>().expect("columns");
    let rows = args[4].parse::<u16>().expect("rows");
    let alternate = args[5] == "alternate";
    let transport = TransportMode::from_arg(&args[6]);

    let mut config = RuntimeConfig::m001().expect("bundled capability policy");
    config.singleton_path = env::temp_dir().join(format!(
        "seyal-runtime-bench-{}-{requested}-{columns}x{rows}.lock",
        process::id()
    ));
    let local_ipc_runtime_dir = match transport {
        TransportMode::SocketOnly => None,
        TransportMode::HybridProjection => Some(env::temp_dir().join(format!(
            "seyal-runtime-bench-ipc-{}-{requested}-{columns}x{rows}",
            process::id()
        ))),
    };
    config.local_ipc = match &local_ipc_runtime_dir {
        None => LocalIpcMode::Disabled,
        Some(path) => LocalIpcMode::Enabled {
            runtime_dir_override: Some(path.clone()),
        },
    };
    config.max_executions = requested.max(1);
    let mut runtime = Runtime::new(config).expect("headless Runtime");
    let baseline = process_metrics(&runtime);

    let creation_start = Instant::now();
    let mut ids = Vec::with_capacity(requested);
    let mut platform_limit = None;
    for index in 0..requested {
        let command = if alternate && index == 0 {
            CommandSpec::new("/bin/sh").args(["-c", "printf '\x1b[?1049hALT'; exec /bin/cat"])
        } else {
            CommandSpec::new("/bin/cat")
        };
        match runtime.create_execution(
            command,
            WindowSize::new(columns, rows, 0, 0).expect("valid geometry"),
        ) {
            Ok(id) => ids.push(id),
            Err(error) => {
                platform_limit = Some(error.to_string());
                break;
            }
        }
    }
    let creation_us = creation_start.elapsed().as_micros();

    for _ in 0..4 {
        let _ = runtime.poll_once(Some(Duration::from_millis(2)));
    }
    thread::sleep(Duration::from_millis(250));
    let populated = process_metrics(&runtime);

    let registry_start = Instant::now();
    for _ in 0..100 {
        let summaries = runtime.list();
        for summary in &summaries {
            std::hint::black_box(runtime.lookup(summary.id));
        }
    }
    let registry_us = registry_start.elapsed().as_micros();

    let progress_us = ids.first().and_then(|id| {
        let ingress = runtime.input_ingress(*id).ok()?;
        let before_generation = runtime.execution(*id)?.terminal().damage_generation();
        let start = Instant::now();
        ingress.try_submit(b"z".to_vec()).ok()?;
        let deadline = Instant::now() + Duration::from_secs(1);
        while Instant::now() < deadline {
            runtime.poll_once(Some(Duration::from_millis(20))).ok()?;
            if runtime.execution(*id)?.terminal().damage_generation() > before_generation {
                return Some(start.elapsed().as_micros());
            }
        }
        None
    });

    let teardown_start = Instant::now();
    runtime.begin_shutdown().expect("begin shutdown");
    let shutdown = runtime.run_until_empty(Instant::now() + Duration::from_secs(8));
    let teardown_us = teardown_start.elapsed().as_micros();
    let final_metrics = process_metrics(&runtime);

    let classification = if ids.len() == requested {
        "MEASURED"
    } else {
        "PLATFORM_LIMITED"
    };
    println!(
        "runtime_resource transport={} population_requested={requested} population_created={} geometry={}x{} screen={} classification={classification} create_us={creation_us} teardown_us={teardown_us} registry_100x_us={registry_us} control_to_state_us={:?} rss_baseline_kib={} rss_populated_kib={} rss_final_kib={} incremental_runtime_kib={} child_rss_kib={} idle_cpu_percent={} threads_baseline={} threads_populated={} threads_final={} fd_baseline={} fd_populated={} fd_final={} pending_final={} shutdown_ok={} platform_error={:?}",
        transport.as_arg(),
        ids.len(),
        columns,
        rows,
        if alternate { "alternate" } else { "primary" },
        progress_us,
        baseline.rss_kib,
        populated.rss_kib,
        final_metrics.rss_kib,
        populated.rss_kib.saturating_sub(baseline.rss_kib),
        populated.child_rss_kib,
        populated.cpu_percent,
        baseline.threads,
        populated.threads,
        final_metrics.threads,
        baseline.fds,
        populated.fds,
        final_metrics.fds,
        runtime.aggregate_accepted_but_unwritten_bytes(),
        shutdown.is_ok(),
        platform_limit,
    );
    shutdown.expect("controlled Runtime teardown");
    if let Some(runtime_dir) = local_ipc_runtime_dir {
        let _ = fs::remove_dir_all(runtime_dir);
    }
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy)]
struct Metrics {
    rss_kib: usize,
    child_rss_kib: usize,
    cpu_percent: f32,
    threads: usize,
    fds: usize,
}

#[cfg(target_os = "macos")]
fn process_metrics(runtime: &Runtime) -> Metrics {
    let pid = process::id();
    let output = Command::new("/bin/ps")
        .args(["-o", "rss=,%cpu=,thcount=", "-p", &pid.to_string()])
        .output()
        .expect("ps Runtime metrics");
    let line = String::from_utf8_lossy(&output.stdout);
    let mut fields = line.split_whitespace();
    let rss_kib = fields.next().and_then(|v| v.parse().ok()).unwrap_or(0);
    let cpu_percent = fields.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let threads = fields.next().and_then(|v| v.parse().ok()).unwrap_or(0);
    let child_rss_kib = runtime
        .list()
        .iter()
        .filter_map(|summary| runtime.execution(summary.id))
        .map(|execution| rss_for_pid(execution.child_id()))
        .sum();
    let fds = fs::read_dir("/dev/fd")
        .map(|entries| entries.count())
        .unwrap_or(0);
    Metrics {
        rss_kib,
        child_rss_kib,
        cpu_percent,
        threads,
        fds,
    }
}

#[cfg(target_os = "macos")]
fn rss_for_pid(pid: u32) -> usize {
    let output = Command::new("/bin/ps")
        .args(["-o", "rss=", "-p", &pid.to_string()])
        .output();
    output
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(0)
}

#[cfg(target_os = "macos")]
fn print_host_metadata() {
    let product = command_text("/usr/bin/sw_vers", &["-productVersion"]);
    let build = command_text("/usr/bin/sw_vers", &["-buildVersion"]);
    let hardware = command_text("/usr/sbin/sysctl", &["-n", "machdep.cpu.brand_string"]);
    let pty_max = command_text("/usr/sbin/sysctl", &["-n", "kern.tty.ptmx_max"]);
    let rustc = command_text("rustc", &["--version"]);
    println!(
        "host macos_version={product} macos_build={build} hardware={hardware:?} rust={rustc:?} pty_max={pty_max}"
    );
}

#[cfg(target_os = "macos")]
fn command_text(program: &str, args: &[&str]) -> String {
    Command::new(program)
        .args(args)
        .output()
        .ok()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_else(|| "unavailable".to_owned())
}
