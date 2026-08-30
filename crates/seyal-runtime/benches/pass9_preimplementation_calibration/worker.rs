use std::{
    env,
    io::{BufRead, Write, stdin, stdout},
    path::PathBuf,
    process::{self, Child, ChildStdin, Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use seyal_exec::{CommandSpec, WindowSize};
use seyal_runtime::{ExecutionId, LocalIpcMode, Runtime, RuntimeConfig};

use super::config::{
    Geometry, QUIESCENT_SAMPLE_COUNT, QUIESCENT_SAMPLE_INTERVAL, WORKER_RESPONSE_TIMEOUT,
    WORKER_SHUTDOWN_TIMEOUT, WORKER_STARTUP_TIMEOUT,
};
use super::metrics::{ProcessMetrics, process_cpu_seconds, self_metrics};
use super::process_io::spawn_line_reader;

pub(crate) struct RuntimeWorker {
    child: Child,
    stdin: ChildStdin,
    output_rx: mpsc::Receiver<String>,
    pub(crate) pid: u32,
    pub(crate) socket_path: PathBuf,
    pub(crate) runtime_id: u128,
    pub(crate) execution_id: ExecutionId,
    pub(crate) expect_cleanup_transition: bool,
}

fn recv_worker_line(rx: &mpsc::Receiver<String>, timeout: Duration, context: &str) -> String {
    rx.recv_timeout(timeout).unwrap_or_else(|_| {
        panic!("Runtime worker produced no output within {timeout:?} while waiting for {context}")
    })
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
        let output_rx = spawn_line_reader(child.stdout.take().expect("worker stdout"));
        let stderr_rx = spawn_line_reader(child.stderr.take().expect("worker stderr"));
        thread::spawn(move || {
            while let Ok(line) = stderr_rx.recv() {
                eprintln!("[runtime-worker stderr] {line}");
            }
        });

        let line = recv_worker_line(&output_rx, WORKER_STARTUP_TIMEOUT, "READY");
        let fields = line.split('\t').collect::<Vec<_>>();
        assert_eq!(fields.first().copied(), Some("READY"));
        let pid: u32 = fields[1].parse().expect("worker pid");
        assert_eq!(pid, child.id());
        let socket_path = PathBuf::from(fields[2]);
        let runtime_id = fields[3].parse().expect("RuntimeId u128");
        let execution_raw: u128 = fields[4].parse().expect("ExecutionId u128");
        Self {
            child,
            stdin,
            output_rx,
            pid,
            socket_path,
            runtime_id,
            execution_id: ExecutionId::from_bytes(execution_raw.to_le_bytes()),
            expect_cleanup_transition: false,
        }
    }

    fn send_command(&mut self, command: &str) {
        writeln!(self.stdin, "{command}").expect("worker command write");
        self.stdin.flush().expect("worker command flush");
    }

    fn read_line_with_prefix(&mut self, prefix: &str) -> String {
        loop {
            let line = recv_worker_line(&self.output_rx, WORKER_RESPONSE_TIMEOUT, prefix);
            if line.starts_with(prefix) {
                return line;
            }
            assert!(
                line.starts_with("CLEANUP\t"),
                "unexpected Runtime worker output: {line}"
            );
            if self.expect_cleanup_transition {
                panic!("cleanup transition consumed before explicit read: {line}");
            }
        }
    }

    pub(crate) fn metrics(&mut self) -> ProcessMetrics {
        self.send_command("metrics");
        let line = self.read_line_with_prefix("METRICS\t");
        parse_worker_metrics(&line)
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
        }
    }

    pub(crate) fn read_cleanup_sample(&mut self) -> u64 {
        let line = self.read_line_with_prefix("CLEANUP\t");
        self.expect_cleanup_transition = false;
        line.split('\t')
            .nth(1)
            .expect("cleanup ns")
            .parse()
            .expect("cleanup integer")
    }

    pub(crate) fn measure_idle_cpu(&mut self) -> f64 {
        let started_cpu = process_cpu_seconds(self.pid);
        let started = Instant::now();
        thread::sleep(Duration::from_millis(250));
        let elapsed = started.elapsed().as_secs_f64();
        let cpu = (process_cpu_seconds(self.pid) - started_cpu).max(0.0);
        if elapsed == 0.0 {
            0.0
        } else {
            cpu / elapsed * 100.0
        }
    }

    pub(crate) fn finish(mut self) {
        self.send_command("stop");
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

fn parse_worker_metrics(line: &str) -> ProcessMetrics {
    let fields = line.split('\t').collect::<Vec<_>>();
    assert_eq!(fields[0], "METRICS");
    ProcessMetrics {
        rss_kib: fields[1].parse().expect("worker RSS"),
        threads: fields[2].parse().expect("worker threads"),
        fds: fields[3].parse().expect("worker fds"),
        attachment_count: fields[4].parse().expect("worker attachments"),
    }
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

    let (command_tx, command_rx) = mpsc::channel::<String>();
    let command_thread = thread::spawn(move || {
        let stdin = stdin();
        for line in stdin.lock().lines() {
            match line {
                Ok(line) => {
                    let stop = line == "stop";
                    if command_tx.send(line).is_err() || stop {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    let mut stop = false;
    while !stop {
        runtime
            .poll_once(Some(Duration::from_millis(2)))
            .expect("Runtime poll");

        loop {
            match command_rx.try_recv() {
                Ok(command) => match command.as_str() {
                    "metrics" => {
                        let metrics = self_metrics();
                        let attachment_count = runtime
                            .lookup(execution_id)
                            .expect("live execution")
                            .attachment_count;
                        println!(
                            "METRICS\t{}\t{}\t{}\t{}",
                            metrics.rss_kib, metrics.threads, metrics.fds, attachment_count
                        );
                        stdout().flush().expect("metrics flush");
                    }
                    "stop" => {
                        stop = true;
                        break;
                    }
                    other => panic!("unknown Runtime worker command: {other}"),
                },
                Err(mpsc::TryRecvError::Empty) => break,
                // The command channel disconnects only when the reader
                // thread's `for line in stdin.lock().lines()` loop ends,
                // which happens on stdin EOF — i.e. the controlling cohort
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
