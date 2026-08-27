#![cfg(target_os = "macos")]

use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

use seyal_client::{ClientError, GridGeometry, InputAdmissionFailure, LocalDisplayClient};
use seyal_exec::{CommandSpec, WindowSize};
use seyal_runtime::{ExecutionId, LocalIpcMode, Runtime, RuntimeConfig, local_ipc::framing::Role};

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct RuntimeHarness {
    socket_path: PathBuf,
    execution_id: ExecutionId,
    stop: mpsc::Sender<()>,
    join: thread::JoinHandle<()>,
}

impl RuntimeHarness {
    fn start(command: CommandSpec) -> Self {
        let suffix = COUNTER.fetch_add(1, Ordering::Relaxed);
        let (ready_tx, ready_rx) = mpsc::channel();
        let (stop_tx, stop_rx) = mpsc::channel();
        let join = thread::spawn(move || {
            let mut config = RuntimeConfig::m001().expect("M001 Runtime config");
            config.singleton_path = std::env::temp_dir().join(format!("s7c-{suffix:x}.lock"));
            config.local_ipc = LocalIpcMode::Enabled {
                runtime_dir_override: Some(std::env::temp_dir().join(format!("s7cd-{suffix:x}"))),
            };
            config.graceful_termination = Duration::from_millis(50);
            config.forced_reap = Duration::from_millis(250);
            config.final_drain = Duration::from_millis(100);

            let mut runtime = Runtime::new(config).expect("Runtime");
            let execution_id = runtime
                .create_execution(command, WindowSize::cells(80, 24).expect("geometry"))
                .expect("execution");
            let socket_path = runtime
                .local_ipc_socket_path()
                .expect("local IPC socket")
                .to_path_buf();
            ready_tx
                .send((socket_path, execution_id))
                .expect("test receiver");

            let deadline = Instant::now() + Duration::from_secs(10);
            while stop_rx.try_recv().is_err() && Instant::now() < deadline {
                runtime
                    .poll_once(Some(Duration::from_millis(2)))
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
        Self {
            socket_path,
            execution_id,
            stop: stop_tx,
            join,
        }
    }

    fn connect_controller(&self) -> LocalDisplayClient {
        LocalDisplayClient::connect_execution(&self.socket_path, self.execution_id, Role::Controller)
            .expect("controller attach")
    }

    fn connect_observer(&self) -> LocalDisplayClient {
        LocalDisplayClient::connect_execution(&self.socket_path, self.execution_id, Role::Observer)
            .expect("observer attach")
    }

    fn finish(self) {
        let _ = self.stop.send(());
        self.join.join().expect("Runtime thread");
    }
}

fn prepared_text(client: &LocalDisplayClient) -> String {
    client
        .prepared_surface()
        .prepared_cells()
        .iter()
        .filter_map(|cell| char::from_u32(cell.scalar))
        .collect()
}

fn pump_until(client: &mut LocalDisplayClient, predicate: impl Fn(&LocalDisplayClient) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if predicate(client) {
            return;
        }
        assert!(Instant::now() < deadline, "Pass 7 client condition timed out");
        match client.poll_prepare() {
            Ok(_) => {}
            Err(error) => panic!("Pass 7 client failed: {error:?}"),
        }
        if client.wants_write() {
            client.flush_control_write().expect("client flush");
        }
        thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn committed_text_is_one_controller_action_and_reaches_real_pty() {
    let runtime = RuntimeHarness::start(CommandSpec::new("/bin/cat"));
    let mut client = runtime.connect_controller();

    client
        .submit_committed_text("PASS7-TEXT")
        .expect("atomic committed input admission");
    pump_until(&mut client, |client| {
        prepared_text(client).contains("PASS7-TEXT")
    });

    assert_eq!(client.input_failure(), None);
    drop(client);
    runtime.finish();
}

#[test]
fn oversized_committed_text_is_rejected_whole_and_next_input_still_works() {
    let runtime = RuntimeHarness::start(CommandSpec::new("/bin/cat"));
    let mut client = runtime.connect_controller();
    let oversized = "x".repeat(65_537);

    assert_eq!(
        client.submit_committed_text(&oversized),
        Err(ClientError::CommitTooLarge)
    );
    assert_eq!(
        client.input_failure(),
        Some(InputAdmissionFailure::CommitTooLarge)
    );

    client
        .submit_committed_text("OK")
        .expect("subsequent atomic input");
    pump_until(&mut client, |client| prepared_text(client).contains("OK"));
    assert_eq!(client.input_failure(), None);

    drop(client);
    runtime.finish();
}

#[test]
fn observer_cannot_submit_terminal_input() {
    let runtime = RuntimeHarness::start(CommandSpec::new("/bin/cat"));
    let mut observer = runtime.connect_observer();

    assert_eq!(
        observer.submit_committed_text("DENIED"),
        Err(ClientError::LostController)
    );
    assert_eq!(
        observer.input_failure(),
        Some(InputAdmissionFailure::LostController)
    );

    drop(observer);
    runtime.finish();
}

#[test]
fn resize_away_then_return_converges_to_latest_desired_geometry() {
    let runtime = RuntimeHarness::start(CommandSpec::new("/bin/cat"));
    let mut client = runtime.connect_controller();
    assert_eq!((client.cache().rows, client.cache().columns), (24, 80));

    client
        .set_desired_geometry(GridGeometry {
            rows: 30,
            columns: 100,
        })
        .expect("resize away");
    client
        .set_desired_geometry(GridGeometry {
            rows: 24,
            columns: 80,
        })
        .expect("restore latest desired geometry");

    pump_until(&mut client, |client| {
        (client.cache().rows, client.cache().columns) == (24, 80)
            && !client.wants_write()
            && client.resize_failure().is_none()
    });

    // No further native resize event is required: the client has converged back
    // to the latest desired geometry even though it initially equalled the old
    // committed projection while a different resize was outstanding.
    for _ in 0..8 {
        let _ = client.poll_prepare();
        assert_eq!((client.cache().rows, client.cache().columns), (24, 80));
        thread::sleep(Duration::from_millis(1));
    }

    drop(client);
    runtime.finish();
}
