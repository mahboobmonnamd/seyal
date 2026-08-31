use std::{
    env,
    io::{BufRead, BufReader, Read, Write, stdin, stdout},
    path::PathBuf,
    process::{self, Child, ChildStdin, Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use seyal_exec::{CommandSpec, WindowSize};
use seyal_runtime::{ExecutionId, LocalIpcMode, Runtime, RuntimeConfig};

use super::config::{
    Geometry, IDLE_CPU_SAMPLE_COUNT, IDLE_CPU_SAMPLE_WINDOW, QUIESCENT_SAMPLE_COUNT,
    QUIESCENT_SAMPLE_INTERVAL, WORKER_RESPONSE_TIMEOUT, WORKER_SHUTDOWN_TIMEOUT,
    WORKER_STARTUP_TIMEOUT,
};
use super::metrics::{ProcessMetrics, metrics_for_pid, process_cpu_seconds};
use super::process_io::spawn_line_reader;

pub(crate) struct RuntimeWorker {
    child: Child,
    stdin: ChildStdin,
    output_rx: mpsc::Receiver<WorkerOutput>,
    pub(crate) pid: u32,
    pub(crate) socket_path: PathBuf,
    pub(crate) runtime_id: u128,
    pub(crate) execution_id: ExecutionId,
    pub(crate) expect_cleanup_transition: bool,
}

pub(crate) struct IdleCpuStats {
    pub(crate) samples_percent: Vec<f64>,
    pub(crate) p50_percent: f64,
    pub(crate) p95_percent: f64,
    pub(crate) max_percent: f64,
}

#[derive(Clone, Copy)]
enum WorkerCommand {
    Lifecycle,
    Stop,
}

impl WorkerCommand {
    fn wire_name(self) -> &'static str {
        match self {
            Self::Lifecycle => "lifecycle",
            Self::Stop => "stop",
        }
    }
}

enum WorkerOutput {
    Ready {
        pid: u32,
        socket_path: PathBuf,
        runtime_id: u128,
        execution_id: u128,
    },
    Cleanup(u64),
    Lifecycle(LifecycleMetrics),
}

#[derive(Clone, Copy)]
struct LifecycleMetrics {
    attachment_count: usize,
    has_controller: bool,
    local_connection_count: usize,
    pending_resync_count: usize,
    pending_resync_set_count: usize,
    listener_backoff_active: bool,
}

fn recv_worker_output(
    rx: &mpsc::Receiver<WorkerOutput>,
    timeout: Duration,
    context: &str,
) -> WorkerOutput {
    rx.recv_timeout(timeout).unwrap_or_else(|_| {
        panic!("Runtime worker produced no output within {timeout:?} while waiting for {context}")
    })
}

fn spawn_worker_output_reader<R>(reader: R) -> mpsc::Receiver<WorkerOutput>
where
    R: Read + Send + 'static,
{
    // The bounded channel is allocated once. Each lifecycle/cleanup message is
    // parsed into a Copy value while one reusable line buffer retains its
    // capacity, so observing the Runtime does not create per-sample heap work
    // in the measured client cohort.
    let (tx, rx) = mpsc::sync_channel(4);
    thread::spawn(move || {
        let mut reader = BufReader::new(reader);
        let mut line = String::with_capacity(256);
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            let output = parse_worker_output(line.trim_end());
            if tx.send(output).is_err() {
                break;
            }
        }
    });
    rx
}

impl RuntimeWorker {
    pub(crate) fn start(geometry: Geometry) -> Self {
        let mut child = Command::new(env::current_exe().expect("benchmark executable"))
            .arg("--runtime-worker")
            .arg(geometry.label())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            // Never inherit stderr from this process: `Stdio::inherit()` here
            // would share the fd backing the parent's own stdout/stderr
            // pipe. If this worker outlives its immediate parent (e.g. the
            // parent panics before calling `finish`), it keeps that pipe's
            // write end open, and anything waiting on that pipe's EOF blocks
            // forever even though the process it actually spawned has
            // already exited. Piping instead gives this worker its own
            // independent fd whose lifetime is exactly this process's
            // lifetime.
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn fresh Runtime worker process");
        let stdin = child.stdin.take().expect("worker stdin");
        let output_rx = spawn_worker_output_reader(child.stdout.take().expect("worker stdout"));
        let stderr_rx = spawn_line_reader(child.stderr.take().expect("worker stderr"));
        thread::spawn(move || {
            while let Ok(line) = stderr_rx.recv() {
                eprintln!("[runtime-worker stderr] {line}");
            }
        });

        let WorkerOutput::Ready {
            pid,
            socket_path,
            runtime_id,
            execution_id,
        } = recv_worker_output(&output_rx, WORKER_STARTUP_TIMEOUT, "READY")
        else {
            panic!("Runtime worker emitted a non-READY record during startup");
        };
        assert_eq!(pid, child.id());
        Self {
            child,
            stdin,
            output_rx,
            pid,
            socket_path,
            runtime_id,
            execution_id: ExecutionId::from_bytes(execution_id.to_le_bytes()),
            expect_cleanup_transition: false,
        }
    }

    fn send_command(&mut self, command: WorkerCommand) {
        writeln!(self.stdin, "{}", command.wire_name()).expect("worker command write");
        self.stdin.flush().expect("worker command flush");
    }

    fn read_lifecycle(&mut self) -> LifecycleMetrics {
        loop {
            match recv_worker_output(&self.output_rx, WORKER_RESPONSE_TIMEOUT, "LIFECYCLE") {
                WorkerOutput::Lifecycle(lifecycle) => return lifecycle,
                WorkerOutput::Cleanup(cleanup_ns) if self.expect_cleanup_transition => {
                    panic!("cleanup transition consumed before explicit read: {cleanup_ns}");
                }
                WorkerOutput::Cleanup(_) => {}
                WorkerOutput::Ready { .. } => {
                    panic!("Runtime worker emitted duplicate READY output");
                }
            }
        }
    }

    pub(crate) fn metrics(&mut self) -> ProcessMetrics {
        self.send_command(WorkerCommand::Lifecycle);
        let lifecycle = self.read_lifecycle();
        // Sample after the fixed diagnostic exchange so the OS counters and
        // logical state describe the same settled observation point.
        let mut metrics = metrics_for_pid(self.pid);
        metrics.attachment_count = lifecycle.attachment_count;
        metrics.has_controller = lifecycle.has_controller;
        metrics.local_connection_count = lifecycle.local_connection_count;
        metrics.pending_resync_count = lifecycle.pending_resync_count;
        metrics.pending_resync_set_count = lifecycle.pending_resync_set_count;
        metrics.listener_backoff_active = lifecycle.listener_backoff_active;
        metrics
    }

    pub(crate) fn median_metrics(&mut self) -> ProcessMetrics {
        let mut rss = Vec::with_capacity(QUIESCENT_SAMPLE_COUNT);
        let mut threads = Vec::with_capacity(QUIESCENT_SAMPLE_COUNT);
        let mut fds = Vec::with_capacity(QUIESCENT_SAMPLE_COUNT);
        let mut attachment_count = 0;
        for sample in 0..QUIESCENT_SAMPLE_COUNT {
            let metrics = self.metrics();
            rss.push(metrics.rss_kib);
            threads.push(metrics.threads);
            fds.push(metrics.fds);
            attachment_count = metrics.attachment_count;
            if sample + 1 != QUIESCENT_SAMPLE_COUNT {
                thread::sleep(QUIESCENT_SAMPLE_INTERVAL);
            }
        }
        rss.sort_unstable();
        threads.sort_unstable();
        fds.sort_unstable();
        ProcessMetrics {
            rss_kib: rss[QUIESCENT_SAMPLE_COUNT / 2],
            threads: threads[QUIESCENT_SAMPLE_COUNT / 2],
            fds: fds[QUIESCENT_SAMPLE_COUNT / 2],
            attachment_count,
            has_controller: false,
            local_connection_count: 0,
            pending_resync_count: 0,
            pending_resync_set_count: 0,
            listener_backoff_active: false,
        }
    }

    pub(crate) fn read_cleanup_sample(&mut self) -> u64 {
        match recv_worker_output(&self.output_rx, WORKER_RESPONSE_TIMEOUT, "CLEANUP") {
            WorkerOutput::Cleanup(cleanup_ns) => {
                self.expect_cleanup_transition = false;
                cleanup_ns
            }
            WorkerOutput::Lifecycle(_) => {
                panic!("Runtime worker emitted unexpected lifecycle output before cleanup");
            }
            WorkerOutput::Ready { .. } => {
                panic!("Runtime worker emitted duplicate READY output");
            }
        }
    }

    pub(crate) fn measure_idle_cpu(&mut self) -> IdleCpuStats {
        let mut samples_percent = Vec::with_capacity(IDLE_CPU_SAMPLE_COUNT);
        for _ in 0..IDLE_CPU_SAMPLE_COUNT {
            let started_cpu = process_cpu_seconds(self.pid);
            let started = Instant::now();
            thread::sleep(IDLE_CPU_SAMPLE_WINDOW);
            let elapsed = started.elapsed().as_secs_f64();
            let cpu = (process_cpu_seconds(self.pid) - started_cpu).max(0.0);
            samples_percent.push(if elapsed == 0.0 {
                0.0
            } else {
                cpu / elapsed * 100.0
            });
        }
        let mut sorted = samples_percent.clone();
        sorted.sort_by(f64::total_cmp);
        let p95_index = (95 * sorted.len()).div_ceil(100).saturating_sub(1);
        IdleCpuStats {
            p50_percent: sorted[sorted.len() / 2],
            p95_percent: sorted[p95_index],
            max_percent: *sorted.last().expect("idle CPU samples"),
            samples_percent,
        }
    }

    pub(crate) fn finish(mut self) {
        self.send_command(WorkerCommand::Stop);
        let deadline = Instant::now() + WORKER_SHUTDOWN_TIMEOUT;
        loop {
            if let Some(status) = self.child.try_wait().expect("Runtime worker wait") {
                assert!(status.success(), "Runtime worker shutdown failed");
                return;
            }
            if Instant::now() >= deadline {
                let _ = self.child.kill();
                let _ = self.child.wait();
                panic!(
                    "Runtime worker pid {} did not exit within {WORKER_SHUTDOWN_TIMEOUT:?} after stop",
                    self.pid
                );
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
}

// Safety net for any exit path that skips `finish` (an assertion failure or
// panic anywhere in a cohort while `worker` is still a live local). Without
// this, `Child::drop` leaves the spawned process running: it does not send a
// kill signal, so a failed cycle would leak a live Runtime worker for every
// subsequent cohort in the run.
impl Drop for RuntimeWorker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn parse_worker_output(line: &str) -> WorkerOutput {
    let mut fields = line.split('\t');
    let kind = fields.next().expect("worker output kind");
    let output = match kind {
        "READY" => WorkerOutput::Ready {
            pid: fields
                .next()
                .expect("worker pid")
                .parse()
                .expect("worker pid"),
            socket_path: PathBuf::from(fields.next().expect("worker socket path")),
            runtime_id: fields
                .next()
                .expect("RuntimeId")
                .parse()
                .expect("RuntimeId u128"),
            execution_id: fields
                .next()
                .expect("ExecutionId")
                .parse()
                .expect("ExecutionId u128"),
        },
        "CLEANUP" => WorkerOutput::Cleanup(
            fields
                .next()
                .expect("cleanup ns")
                .parse()
                .expect("cleanup integer"),
        ),
        "LIFECYCLE" => WorkerOutput::Lifecycle(LifecycleMetrics {
            attachment_count: fields
                .next()
                .expect("worker attachments")
                .parse()
                .expect("worker attachments"),
            has_controller: fields
                .next()
                .expect("worker controller")
                .parse()
                .expect("worker controller"),
            local_connection_count: fields
                .next()
                .expect("worker connections")
                .parse()
                .expect("worker connections"),
            pending_resync_count: fields
                .next()
                .expect("worker pending resync")
                .parse()
                .expect("worker pending resync"),
            pending_resync_set_count: fields
                .next()
                .expect("worker pending resync set")
                .parse()
                .expect("worker pending resync set"),
            listener_backoff_active: fields
                .next()
                .expect("worker listener backoff")
                .parse()
                .expect("worker listener backoff"),
        }),
        other => panic!("unknown Runtime worker output: {other}"),
    };
    assert!(
        fields.next().is_none(),
        "extra Runtime worker output fields"
    );
    output
}

pub(crate) fn run_runtime_worker(geometry: Geometry) {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let mut config = RuntimeConfig::m001().expect("M001 Runtime config");
    config.singleton_path = env::temp_dir().join(format!("s9w-{}-{nonce:x}.lock", process::id()));
    let runtime_dir = env::temp_dir().join(format!("s9wd-{}-{nonce:x}", process::id()));
    config.local_ipc = LocalIpcMode::Enabled {
        runtime_dir_override: Some(runtime_dir),
    };
    config.graceful_termination = Duration::from_millis(50);
    config.forced_reap = Duration::from_millis(250);
    config.final_drain = Duration::from_millis(100);

    let mut runtime = Runtime::new(config).expect("Runtime");
    let execution_id = runtime
        .create_execution(
            CommandSpec::new("/bin/cat"),
            WindowSize::cells(geometry.columns, geometry.rows).expect("benchmark geometry"),
        )
        .expect("execution");
    let socket_path = runtime
        .local_ipc_socket_path()
        .expect("Runtime socket path")
        .to_path_buf();
    println!(
        "READY\t{}\t{}\t{}\t{}",
        process::id(),
        socket_path.display(),
        u128::from_le_bytes(runtime.id().to_bytes()),
        u128::from_le_bytes(execution_id.to_bytes())
    );
    stdout().flush().expect("READY flush");

    let (command_tx, command_rx) = mpsc::sync_channel::<WorkerCommand>(1);
    let command_thread = thread::spawn(move || {
        let stdin = stdin();
        let mut stdin = stdin.lock();
        let mut command_buffer = String::with_capacity(16);
        loop {
            command_buffer.clear();
            match stdin.read_line(&mut command_buffer) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            let command = match command_buffer.trim_end() {
                "lifecycle" => WorkerCommand::Lifecycle,
                "stop" => WorkerCommand::Stop,
                other => panic!("unknown Runtime worker command: {other}"),
            };
            if command_tx.send(command).is_err() || matches!(command, WorkerCommand::Stop) {
                break;
            }
        }
    });

    let mut stop = false;
    while !stop {
        runtime
            .poll_once(Some(Duration::from_millis(2)))
            .expect("Runtime poll");

        // SPEC-009 Section 16 forbids a synchronous logging dependency on the
        // Runtime's disconnect/detach hot path. `poll_once` above only records
        // the elapsed cleanup time into a plain in-memory field (see
        // `Runtime::record_pass9_cleanup_sample` in
        // crates/seyal-runtime/src/runtime.rs); this harness reads it back out
        // and performs the actual CLEANUP line I/O here, entirely outside the
        // production event-dispatch call.
        if let Some(cleanup_ns) = runtime.take_benchmark_pass9_cleanup_sample() {
            println!("CLEANUP\t{cleanup_ns}");
            stdout().flush().expect("cleanup flush");
        }

        loop {
            match command_rx.try_recv() {
                Ok(command) => match command {
                    WorkerCommand::Lifecycle => {
                        let lifecycle = runtime
                            .benchmark_pass9_lifecycle_diagnostics(execution_id)
                            .expect("Runtime local-IPC lifecycle diagnostics");
                        println!(
                            "LIFECYCLE\t{}\t{}\t{}\t{}\t{}\t{}",
                            lifecycle.attachment_count,
                            lifecycle.has_controller,
                            lifecycle.local_connection_count,
                            lifecycle.pending_resync_count,
                            lifecycle.pending_resync_set_count,
                            lifecycle.listener_backoff_active
                        );
                        stdout().flush().expect("lifecycle flush");
                    }
                    WorkerCommand::Stop => {
                        stop = true;
                        break;
                    }
                },
                Err(mpsc::TryRecvError::Empty) => break,
                // The command channel disconnects only when the reader
                // thread's reusable-buffer read loop ends, which happens on
                // stdin EOF — i.e. the controlling cohort
                // process is gone and can never send an explicit "stop".
                // Treating this the same as "stop" is the only way this
                // worker terminates instead of running forever as an
                // undetectable orphan: verified by direct reproduction —
                // spawning a worker and closing all three of its stdio pipes
                // (matching what the kernel does when a parent dies) left it
                // running indefinitely before this fix.
                Err(mpsc::TryRecvError::Disconnected) => {
                    stop = true;
                    break;
                }
            }
        }
    }

    runtime.begin_shutdown().expect("begin Runtime shutdown");
    runtime
        .run_until_empty(Instant::now() + Duration::from_secs(3))
        .expect("Runtime shutdown");
    drop(runtime);
    command_thread.join().expect("worker command thread");
}
