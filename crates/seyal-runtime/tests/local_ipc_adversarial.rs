#![cfg(target_os = "macos")]

//! Attachment IDs are identities, not bearer capabilities. A second connection
//! cannot use another connection's controller ID for input/resize/resync/detach.

use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    sync::Barrier,
    time::{Duration, Instant},
};

use seyal_exec::{CommandSpec, WindowSize};
use seyal_runtime::{
    ExecutionId, LocalIpcMode, Runtime, RuntimeConfig, RuntimeError,
    display::{DisplayCache, decode_chunk, empty_cache},
    local_ipc::framing::{
        Attach, Attached, ClientHello, Detach, ErrorCode, ErrorMessage, FrameHeader, HEADER_LEN,
        InputRef, MessageType, Resize, Resync, Role, ServerHello, encode_frame,
    },
};

fn config() -> RuntimeConfig {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let suffix = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut config = RuntimeConfig::m001().unwrap();
    config.singleton_path = std::env::temp_dir().join(format!("s5a-{suffix:x}.lock"));
    config.local_ipc = LocalIpcMode::Enabled {
        runtime_dir_override: Some(std::env::temp_dir().join(format!("s5ad-{suffix:x}"))),
    };
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
            runtime: Runtime::new(config()).unwrap(),
        }
    }
    fn pump(&mut self) {
        self.runtime
            .poll_once(Some(Duration::from_millis(5)))
            .unwrap();
    }
    fn spawn_cat(&mut self) -> ExecutionId {
        self.runtime
            .create_execution(
                CommandSpec::new("/bin/cat"),
                WindowSize::new(80, 24, 0, 0).unwrap(),
            )
            .unwrap()
    }
    fn connect(&mut self) -> Client {
        let path = self.runtime.local_ipc_socket_path().unwrap().to_path_buf();
        let stream = UnixStream::connect(path).unwrap();
        stream.set_nonblocking(true).unwrap();
        self.pump();
        Client {
            stream,
            buffered: Vec::new(),
        }
    }
    fn send(&mut self, client: &mut Client, kind: MessageType, payload: &[u8]) {
        client
            .stream
            .write_all(&encode_frame(kind, payload))
            .unwrap();
        self.pump();
    }
    fn frame(&mut self, client: &mut Client) -> (MessageType, Vec<u8>) {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if client.buffered.len() >= HEADER_LEN {
                let header = FrameHeader::decode(&client.buffered[..HEADER_LEN]).unwrap();
                let total = HEADER_LEN + header.payload_len as usize;
                if client.buffered.len() >= total {
                    let raw = client.buffered.drain(..total).collect::<Vec<_>>();
                    return (
                        MessageType::from_u16(header.message_type).unwrap(),
                        raw[HEADER_LEN..].to_vec(),
                    );
                }
            }
            let mut chunk = [0u8; 8192];
            match client.stream.read(&mut chunk) {
                Ok(0) => panic!("connection closed"),
                Ok(n) => client.buffered.extend_from_slice(&chunk[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => self.pump(),
                Err(e) => panic!("read failed: {e}"),
            }
            assert!(Instant::now() < deadline, "frame timed out");
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
        let (kind, payload) = self.frame(client);
        assert_eq!(kind, MessageType::ServerHello);
        ServerHello::decode(&payload).unwrap();
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
        let (kind, payload) = self.frame(client);
        assert_eq!(kind, MessageType::Attached);
        let attached = Attached::decode(&payload).unwrap();
        let mut cache = empty_cache();
        self.apply_display(client, &mut cache);
        (attached, cache)
    }
    fn apply_display(&mut self, client: &mut Client, cache: &mut DisplayCache) {
        let (kind, payload) = self.frame(client);
        assert!(matches!(
            kind,
            MessageType::DisplaySnapshot | MessageType::DisplayDelta
        ));
        let first = decode_chunk(&encode_frame(kind, &payload)).unwrap();
        let expected = first.chunk_count as usize;
        let mut chunks = vec![first];
        while chunks.len() < expected {
            let (next_kind, next_payload) = self.frame(client);
            assert_eq!(next_kind, kind);
            chunks.push(decode_chunk(&encode_frame(kind, &next_payload)).unwrap());
        }
        cache.apply_chunks(&chunks).unwrap();
    }
    fn stale(&mut self, client: &mut Client, kind: MessageType, payload: &[u8]) {
        self.send(client, kind, payload);
        let (response, payload) = self.frame(client);
        assert_eq!(response, MessageType::Error);
        let error = ErrorMessage::decode(&payload).unwrap();
        assert_eq!(error.error_code, ErrorCode::StaleIdentity as u16);
        assert_eq!(error.offending_message_type, kind as u16);
    }
}

#[test]
fn another_connection_cannot_reuse_controller_attachment_identity() {
    let mut harness = Harness::new();
    let execution_id = harness.spawn_cat();

    let mut owner = harness.connect();
    harness.hello(&mut owner);
    let (controller, mut owner_cache) = harness.attach(&mut owner, execution_id, Role::Controller);

    let mut attacker = harness.connect();
    harness.hello(&mut attacker);
    let (_observer, _cache) = harness.attach(&mut attacker, execution_id, Role::Observer);

    harness.stale(
        &mut attacker,
        MessageType::Input,
        &InputRef {
            attachment_id: controller.attachment_id,
            bytes: b"EVIL",
        }
        .encode(),
    );
    harness.stale(
        &mut attacker,
        MessageType::Resize,
        &Resize {
            attachment_id: controller.attachment_id,
            rows: 30,
            columns: 100,
        }
        .encode(),
    );
    harness.stale(
        &mut attacker,
        MessageType::Resync,
        &Resync {
            attachment_id: controller.attachment_id,
        }
        .encode(),
    );
    harness.stale(
        &mut attacker,
        MessageType::Detach,
        &Detach {
            attachment_id: controller.attachment_id,
        }
        .encode(),
    );

    harness.send(
        &mut owner,
        MessageType::Input,
        &InputRef {
            attachment_id: controller.attachment_id,
            bytes: b"OK",
        }
        .encode(),
    );
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let row: String = owner_cache
            .cells
            .iter()
            .take(owner_cache.columns as usize)
            .map(|c| c.scalar)
            .collect();
        if row.starts_with("OK") {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "legitimate controller stopped making progress"
        );
        harness.apply_display(&mut owner, &mut owner_cache);
    }

    drop(attacker);
    drop(owner);
    harness.pump();
    harness.runtime.begin_shutdown().unwrap();
    harness
        .runtime
        .run_until_empty(Instant::now() + Duration::from_secs(3))
        .unwrap();
}

#[test]
fn reconnected_controller_rejects_every_old_attachment_mutation() {
    let mut harness = Harness::new();
    let execution_id = harness.spawn_cat();
    let mut first = harness.connect();
    harness.hello(&mut first);
    let (old, _cache) = harness.attach(&mut first, execution_id, Role::Controller);
    drop(first);
    harness.pump();

    let mut replacement = harness.connect();
    harness.hello(&mut replacement);
    let (fresh, _cache) = harness.attach(&mut replacement, execution_id, Role::Controller);
    assert_ne!(old.attachment_id, fresh.attachment_id);

    harness.stale(
        &mut replacement,
        MessageType::Input,
        &InputRef {
            attachment_id: old.attachment_id,
            bytes: b"STALE",
        }
        .encode(),
    );
    harness.stale(
        &mut replacement,
        MessageType::Resize,
        &Resize {
            attachment_id: old.attachment_id,
            rows: 40,
            columns: 120,
        }
        .encode(),
    );
    harness.stale(
        &mut replacement,
        MessageType::Detach,
        &Detach {
            attachment_id: old.attachment_id,
        }
        .encode(),
    );

    harness.send(
        &mut replacement,
        MessageType::Input,
        &InputRef {
            attachment_id: fresh.attachment_id,
            bytes: b"FRESH",
        }
        .encode(),
    );
}

#[test]
fn controller_cleanup_survives_100_graceful_and_100_abrupt_reconnect_cycles() {
    let mut harness = Harness::new();
    let execution_id = harness.spawn_cat();
    let runtime_id = harness.runtime.id();
    let mut attachment_ids = Vec::with_capacity(201);

    for cycle in 0..100 {
        let mut client = harness.connect();
        harness.hello(&mut client);
        let (attached, _cache) = harness.attach(&mut client, execution_id, Role::Controller);
        assert!(
            !attachment_ids.contains(&attached.attachment_id),
            "graceful cycle {cycle} reused an AttachmentId"
        );
        attachment_ids.push(attached.attachment_id);

        harness.send(
            &mut client,
            MessageType::Detach,
            &Detach {
                attachment_id: attached.attachment_id,
            }
            .encode(),
        );
        let (kind, _payload) = harness.frame(&mut client);
        assert_eq!(kind, MessageType::Detached);
        assert_eq!(
            harness
                .runtime
                .lookup(execution_id)
                .unwrap()
                .attachment_count,
            0,
            "graceful cycle {cycle} retained attachment/controller authority"
        );
        assert_eq!(harness.runtime.id(), runtime_id);
        assert!(harness.runtime.execution(execution_id).is_some());
        drop(client);
        harness.pump();
    }

    for cycle in 0..100 {
        let mut client = harness.connect();
        harness.hello(&mut client);
        let (attached, _cache) = harness.attach(&mut client, execution_id, Role::Controller);
        assert!(
            !attachment_ids.contains(&attached.attachment_id),
            "abrupt cycle {cycle} reused an AttachmentId"
        );
        attachment_ids.push(attached.attachment_id);
        drop(client);

        let deadline = Instant::now() + Duration::from_secs(1);
        while harness
            .runtime
            .lookup(execution_id)
            .unwrap()
            .attachment_count
            != 0
        {
            assert!(
                Instant::now() < deadline,
                "abrupt cycle {cycle} did not release Controller authority"
            );
            harness.pump();
        }
        assert_eq!(harness.runtime.id(), runtime_id);
        assert!(
            harness.runtime.execution(execution_id).is_some(),
            "abrupt cycle {cycle} terminated the live execution"
        );
    }

    let mut final_client = harness.connect();
    harness.hello(&mut final_client);
    let (final_attachment, mut final_cache) =
        harness.attach(&mut final_client, execution_id, Role::Controller);
    assert!(!attachment_ids.contains(&final_attachment.attachment_id));
    harness.send(
        &mut final_client,
        MessageType::Input,
        &InputRef {
            attachment_id: final_attachment.attachment_id,
            bytes: b"FINAL",
        }
        .encode(),
    );
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let visible: String = final_cache.cells.iter().map(|cell| cell.scalar).collect();
        if visible.contains("FINAL") {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Controller was not usable after 200 reconnect cycles"
        );
        harness.apply_display(&mut final_client, &mut final_cache);
    }

    drop(final_client);
    let deadline = Instant::now() + Duration::from_secs(1);
    while harness
        .runtime
        .lookup(execution_id)
        .unwrap()
        .attachment_count
        != 0
    {
        assert!(
            Instant::now() < deadline,
            "final attachment cleanup timed out"
        );
        harness.pump();
    }
    harness.runtime.begin_shutdown().unwrap();
    harness
        .runtime
        .run_until_empty(Instant::now() + Duration::from_secs(3))
        .unwrap();
}

/// SPEC-009 8.1: "two simultaneous Runtime starters produce exactly one
/// canonical endpoint" and "a Runtime that loses singleton bind/startup
/// arbitration exits/fails startup and never selects an alternate competing
/// endpoint." `Runtime::new` never crosses a thread boundary here — the
/// winner performs its own client handshake from inside the thread that
/// created it — so this exercises the real flock-backed
/// `SingletonGuard::acquire` race without depending on `Runtime: Send`.
#[test]
fn two_simultaneous_runtime_starters_produce_exactly_one_canonical_endpoint() {
    const CONTENDERS: usize = 8;
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let suffix = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let singleton_path = std::env::temp_dir().join(format!("s5a-race-{suffix:x}.lock"));
    let runtime_dir = std::env::temp_dir().join(format!("s5ad-race-{suffix:x}"));

    let start = Barrier::new(CONTENDERS);
    let (result_tx, result_rx) = std::sync::mpsc::channel::<bool>();

    std::thread::scope(|scope| {
        for _ in 0..CONTENDERS {
            let start = &start;
            let result_tx = result_tx.clone();
            let mut config = RuntimeConfig::m001().unwrap();
            config.singleton_path = singleton_path.clone();
            config.local_ipc = LocalIpcMode::Enabled {
                runtime_dir_override: Some(runtime_dir.clone()),
            };
            scope.spawn(move || {
                start.wait();
                match Runtime::new(config) {
                    Ok(mut runtime) => {
                        // This thread won singleton arbitration. Prove the
                        // canonical endpoint it bound is genuinely live and
                        // reports this Runtime's own identity before any
                        // other contender could have raced in behind it.
                        let socket_path = runtime.local_ipc_socket_path().unwrap().to_path_buf();
                        let stream = UnixStream::connect(&socket_path).unwrap();
                        stream.set_nonblocking(true).unwrap();
                        let mut client = Client {
                            stream,
                            buffered: Vec::new(),
                        };
                        client
                            .stream
                            .write_all(&encode_frame(
                                MessageType::ClientHello,
                                &ClientHello {
                                    client_capabilities: 0,
                                }
                                .encode(),
                            ))
                            .unwrap();
                        let deadline = Instant::now() + Duration::from_secs(3);
                        let hello = loop {
                            runtime.poll_once(Some(Duration::from_millis(5))).unwrap();
                            if client.buffered.len() >= HEADER_LEN {
                                let header =
                                    FrameHeader::decode(&client.buffered[..HEADER_LEN]).unwrap();
                                let total = HEADER_LEN + header.payload_len as usize;
                                if client.buffered.len() >= total {
                                    let raw = client.buffered.drain(..total).collect::<Vec<_>>();
                                    assert_eq!(
                                        MessageType::from_u16(header.message_type).unwrap(),
                                        MessageType::ServerHello
                                    );
                                    break ServerHello::decode(&raw[HEADER_LEN..]).unwrap();
                                }
                            }
                            let mut chunk = [0u8; 8192];
                            match client.stream.read(&mut chunk) {
                                Ok(0) => panic!("winner connection closed during hello"),
                                Ok(n) => client.buffered.extend_from_slice(&chunk[..n]),
                                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                                Err(e) => panic!("winner hello read failed: {e}"),
                            }
                            assert!(Instant::now() < deadline, "winner hello timed out");
                        };
                        assert_eq!(
                            hello.runtime_id.to_le_bytes(),
                            runtime.id().to_bytes(),
                            "canonical endpoint answered with an identity other than its own binder"
                        );
                        drop(client);
                        runtime.begin_shutdown().unwrap();
                        runtime
                            .run_until_empty(Instant::now() + Duration::from_secs(3))
                            .unwrap();
                        result_tx.send(true).unwrap();
                    }
                    Err(RuntimeError::AlreadyRunning) => {
                        result_tx.send(false).unwrap();
                    }
                    Err(other) => panic!("unexpected startup failure: {other}"),
                }
            });
        }
    });
    drop(result_tx);

    let outcomes: Vec<bool> = result_rx.into_iter().collect();
    assert_eq!(outcomes.len(), CONTENDERS);
    assert_eq!(
        outcomes.iter().filter(|won| **won).count(),
        1,
        "exactly one simultaneous starter must bind the canonical endpoint, got {outcomes:?}"
    );
    assert_eq!(
        outcomes.iter().filter(|won| !**won).count(),
        CONTENDERS - 1,
        "every losing contender must fail startup with AlreadyRunning, not silently succeed"
    );
}
