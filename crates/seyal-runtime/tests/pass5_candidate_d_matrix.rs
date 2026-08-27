#![cfg(target_os = "macos")]

//! Production-path Pass-5 matrix tests. These use a real Runtime, real PTY,
//! Seyal VT mutation, Candidate-D binary UDS delivery and a disposable client
//! display cache. They deliberately do not use the legacy shared projection.

use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    time::{Duration, Instant},
};

use seyal_exec::{CommandSpec, WindowSize};
use seyal_runtime::{
    ExecutionId, LocalIpcMode, Runtime, RuntimeConfig,
    display::{DecodedDisplayChunk, DisplayCache, decode_chunk, empty_cache},
    local_ipc::framing::{
        Attach, Attached, ClientHello, FrameHeader, HEADER_LEN, InputRef, MessageType, Role,
        ServerHello, encode_frame,
    },
};

fn config() -> RuntimeConfig {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let suffix = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut config = RuntimeConfig::m001().expect("M001 runtime config");
    config.singleton_path = std::env::temp_dir().join(format!("s5m-{suffix:x}.lock"));
    config.local_ipc = LocalIpcMode::Enabled {
        runtime_dir_override: Some(std::env::temp_dir().join(format!("s5md-{suffix:x}"))),
    };
    config.graceful_termination = Duration::from_millis(50);
    config.forced_reap = Duration::from_millis(250);
    config.final_drain = Duration::from_millis(100);
    config
}

struct Client {
    stream: UnixStream,
    buffered: Vec<u8>,
}

struct Harness {
    runtime: Runtime,
}

impl Harness {
    fn new() -> Self {
        Self {
            runtime: Runtime::new(config()).expect("Runtime"),
        }
    }

    fn pump(&mut self) {
        self.runtime
            .poll_once(Some(Duration::from_millis(5)))
            .expect("Runtime poll");
    }

    fn spawn(&mut self, command: CommandSpec) -> ExecutionId {
        self.runtime
            .create_execution(
                command,
                WindowSize::new(80, 24, 0, 0).expect("valid geometry"),
            )
            .expect("execution")
    }

    fn connect(&mut self) -> Client {
        let path = self
            .runtime
            .local_ipc_socket_path()
            .expect("local IPC socket")
            .to_path_buf();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match UnixStream::connect(&path) {
                Ok(stream) => {
                    stream.set_nonblocking(true).expect("nonblocking client");
                    self.pump();
                    return Client {
                        stream,
                        buffered: Vec::new(),
                    };
                }
                Err(_) => {
                    assert!(Instant::now() < deadline, "connect timed out");
                    self.pump();
                }
            }
        }
    }

    fn send(&mut self, client: &mut Client, kind: MessageType, payload: &[u8]) {
        let frame = encode_frame(kind, payload);
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut sent = 0usize;
        while sent < frame.len() {
            match client.stream.write(&frame[sent..]) {
                Ok(0) => panic!("client write returned zero"),
                Ok(count) => sent += count,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => self.pump(),
                Err(error) => panic!("client write failed: {error}"),
            }
            assert!(Instant::now() < deadline, "client write timed out");
        }
        self.pump();
    }

    fn frame(&mut self, client: &mut Client, deadline: Instant) -> (u16, Vec<u8>) {
        loop {
            if client.buffered.len() >= HEADER_LEN {
                let header =
                    FrameHeader::decode(&client.buffered[..HEADER_LEN]).expect("valid header");
                let total = HEADER_LEN + header.payload_len as usize;
                if client.buffered.len() >= total {
                    let frame = client.buffered.drain(..total).collect::<Vec<_>>();
                    return (header.message_type, frame[HEADER_LEN..].to_vec());
                }
            }

            let mut chunk = [0u8; 16 * 1024];
            match client.stream.read(&mut chunk) {
                Ok(0) => panic!("connection closed while awaiting frame"),
                Ok(count) => client.buffered.extend_from_slice(&chunk[..count]),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => self.pump(),
                Err(error) => panic!("client read failed: {error}"),
            }
            assert!(Instant::now() < deadline, "timed out awaiting frame");
        }
    }

    fn hello(&mut self, client: &mut Client) {
        self.send(
            client,
            MessageType::ClientHello,
            &ClientHello {
                client_capabilities: 0,
            }
            .encode(),
        );
        let (kind, payload) = self.frame(client, Instant::now() + Duration::from_secs(2));
        assert_eq!(kind, MessageType::ServerHello as u16);
        let hello = ServerHello::decode(&payload).expect("ServerHello");
        assert_ne!(
            hello.server_capabilities & seyal_runtime::local_ipc::framing::CAP_BINARY_DISPLAY,
            0
        );
    }

    fn display_batch(
        &mut self,
        client: &mut Client,
        expected: MessageType,
        deadline: Instant,
    ) -> Vec<DecodedDisplayChunk> {
        let (kind, payload) = self.frame(client, deadline);
        assert_eq!(kind, expected as u16);
        let first = decode_chunk(&encode_frame(expected, &payload)).expect("display chunk");
        let count = first.chunk_count as usize;
        let mut chunks = vec![first];
        for _ in 1..count {
            let (kind, payload) = self.frame(client, deadline);
            assert_eq!(kind, expected as u16);
            chunks.push(
                decode_chunk(&encode_frame(expected, &payload)).expect("display continuation"),
            );
        }
        chunks
    }

    fn attach(
        &mut self,
        client: &mut Client,
        execution_id: ExecutionId,
        role: Role,
    ) -> (Attached, DisplayCache) {
        self.send(
            client,
            MessageType::Attach,
            &Attach {
                execution_id,
                requested_role: role,
            }
            .encode(),
        );
        let deadline = Instant::now() + Duration::from_secs(3);
        let (kind, payload) = self.frame(client, deadline);
        assert_eq!(kind, MessageType::Attached as u16);
        let attached = Attached::decode(&payload).expect("Attached");
        let mut cache = empty_cache();
        cache
            .apply_chunks(&self.display_batch(client, MessageType::DisplaySnapshot, deadline))
            .expect("initial snapshot");
        assert_eq!(cache.generation, attached.current_generation);
        (attached, cache)
    }

    fn apply_next(&mut self, client: &mut Client, cache: &mut DisplayCache) -> MessageType {
        let deadline = Instant::now() + Duration::from_secs(5);
        let (kind, payload) = self.frame(client, deadline);
        let message_type = MessageType::from_u16(kind).expect("known message type");
        assert!(matches!(
            message_type,
            MessageType::DisplaySnapshot | MessageType::DisplayDelta
        ));
        let first = decode_chunk(&encode_frame(message_type, &payload)).expect("display chunk");
        let count = first.chunk_count as usize;
        let mut chunks = vec![first];
        for _ in 1..count {
            let (next_kind, payload) = self.frame(client, deadline);
            assert_eq!(next_kind, kind);
            chunks.push(
                decode_chunk(&encode_frame(message_type, &payload)).expect("display continuation"),
            );
        }
        cache.apply_chunks(&chunks).expect("display apply");
        message_type
    }
}

fn first_row(cache: &DisplayCache) -> String {
    if cache.rows == 0 || cache.columns == 0 {
        return String::new();
    }
    cache.cells[..cache.columns as usize]
        .iter()
        .map(|cell| cell.scalar)
        .collect()
}

fn contains(cache: &DisplayCache, needle: &str) -> bool {
    if cache.columns == 0 {
        return false;
    }
    cache.cells.chunks(cache.columns as usize).any(|row| {
        row.iter()
            .map(|cell| cell.scalar)
            .collect::<String>()
            .contains(needle)
    })
}

#[test]
fn same_execution_fanout_is_consistent_at_4_8_and_16_viewers() {
    for viewer_count in [4usize, 8, 16] {
        let mut harness = Harness::new();
        let execution_id = harness.spawn(CommandSpec::new("/bin/cat"));
        let mut controller = harness.connect();
        harness.hello(&mut controller);
        let (controller_attachment, controller_cache) =
            harness.attach(&mut controller, execution_id, Role::Controller);
        let mut viewers = vec![(controller, controller_cache)];

        for _ in 1..viewer_count {
            let mut observer = harness.connect();
            harness.hello(&mut observer);
            let (_attached, cache) = harness.attach(&mut observer, execution_id, Role::Observer);
            viewers.push((observer, cache));
        }

        harness.send(
            &mut viewers[0].0,
            MessageType::Input,
            &InputRef {
                attachment_id: controller_attachment.attachment_id,
                bytes: b"FANOUT",
            }
            .encode(),
        );

        for (client, cache) in &mut viewers {
            let deadline = Instant::now() + Duration::from_secs(5);
            while !first_row(cache).starts_with("FANOUT") {
                assert!(Instant::now() < deadline, "viewer fanout timed out");
                harness.apply_next(client, cache);
            }
        }

        let generation = viewers[0].1.generation;
        let cells = viewers[0].1.cells.clone();
        assert!(
            viewers
                .iter()
                .all(|(_, cache)| cache.generation == generation),
            "all {viewer_count} viewers must observe one canonical generation"
        );
        assert!(
            viewers.iter().all(|(_, cache)| cache.cells == cells),
            "all {viewer_count} viewers must derive identical visible state"
        );
    }
}

#[test]
fn alternate_screen_state_is_delivered_over_candidate_d() {
    let mut harness = Harness::new();
    let execution_id = harness.spawn(
        CommandSpec::new("/bin/sh").args(["-c", "sleep 0.2; printf '\\033[?1049hALT'; sleep 2"]),
    );
    let mut client = harness.connect();
    harness.hello(&mut client);
    let (_attached, mut cache) = harness.attach(&mut client, execution_id, Role::Observer);

    let deadline = Instant::now() + Duration::from_secs(5);
    while !cache.alternate_screen || !contains(&cache, "ALT") {
        assert!(
            Instant::now() < deadline,
            "alternate-screen delivery timed out"
        );
        harness.apply_next(&mut client, &mut cache);
    }
    assert!(cache.alternate_screen);
    assert!(contains(&cache, "ALT"));
}

#[test]
fn initial_attach_while_alternate_screen_is_active_snapshots_alternate_state() {
    let mut harness = Harness::new();
    let execution_id = harness
        .spawn(CommandSpec::new("/bin/sh").args(["-c", "printf '\\033[?1049hALT_READY'; sleep 3"]));

    let active_deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let alternate_ready = harness
            .runtime
            .execution(execution_id)
            .map(|execution| execution.terminal())
            .is_some_and(|terminal| {
                terminal.modes().alternate_screen
                    && terminal
                        .row_text(0)
                        .is_some_and(|row| row.contains("ALT_READY"))
            });
        if alternate_ready {
            break;
        }
        assert!(
            Instant::now() < active_deadline,
            "canonical alternate screen did not become active before attach"
        );
        harness.pump();
    }

    let canonical_generation = harness
        .runtime
        .execution(execution_id)
        .expect("execution remains live")
        .terminal()
        .damage_generation();
    let mut client = harness.connect();
    harness.hello(&mut client);
    let (attached, cache) = harness.attach(&mut client, execution_id, Role::Observer);

    assert!(
        cache.alternate_screen,
        "initial snapshot must select active alternate screen"
    );
    assert!(contains(&cache, "ALT_READY"));
    assert_eq!(cache.generation, attached.current_generation);
    assert!(
        cache.generation >= canonical_generation,
        "attach snapshot must not regress behind the canonical state observed before attach"
    );
}

#[test]
fn stalled_viewer_is_superseded_and_recovers_from_current_snapshot() {
    let mut harness = Harness::new();
    let execution_id = harness.spawn(CommandSpec::new("/bin/sh").args([
        "-c",
        "sleep 0.2; yes LINE | head -n 50000; printf 'DONE\\r\\n'; sleep 3",
    ]));

    let mut slow = harness.connect();
    harness.hello(&mut slow);
    let (_slow_attachment, mut slow_cache) =
        harness.attach(&mut slow, execution_id, Role::Observer);

    let mut healthy = harness.connect();
    harness.hello(&mut healthy);
    let (_healthy_attachment, mut healthy_cache) =
        harness.attach(&mut healthy, execution_id, Role::Observer);

    let healthy_deadline = Instant::now() + Duration::from_secs(10);
    while !contains(&healthy_cache, "DONE") {
        assert!(
            Instant::now() < healthy_deadline,
            "healthy viewer stopped making progress while peer was stalled"
        );
        harness.apply_next(&mut healthy, &mut healthy_cache);
    }

    let recovery_deadline = Instant::now() + Duration::from_secs(5);
    let mut saw_snapshot = false;
    while !contains(&slow_cache, "DONE") {
        assert!(
            Instant::now() < recovery_deadline,
            "stalled viewer failed to converge to current state"
        );
        if harness.apply_next(&mut slow, &mut slow_cache) == MessageType::DisplaySnapshot {
            saw_snapshot = true;
        }
    }

    assert!(
        saw_snapshot,
        "superseded presentation history must recover through a current snapshot"
    );
    assert_eq!(slow_cache.generation, healthy_cache.generation);
    assert_eq!(slow_cache.cells, healthy_cache.cells);
}
