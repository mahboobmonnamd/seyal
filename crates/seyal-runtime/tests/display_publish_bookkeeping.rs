#![cfg(all(target_os = "macos", feature = "test-fault-injection"))]

//! Pass 10 / #759: display `published` bookkeeping must not advance when encode
//! fails (`DisplayUnavailable`). Otherwise later deltas use a base no viewer
//! received, breaking multi-viewer continuity.

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
        Attach, Attached, ClientHello, ErrorCode, ErrorMessage, FrameHeader, HEADER_LEN, InputRef,
        MessageType, Role, ServerHello, encode_frame,
    },
    test_fault::{self, FaultPoint},
};

fn config() -> RuntimeConfig {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let suffix = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut config = RuntimeConfig::m001().expect("bundled capability profile");
    config.singleton_path = std::env::temp_dir().join(format!("s759-{suffix:x}.lock"));
    config.local_ipc = LocalIpcMode::Enabled {
        runtime_dir_override: Some(std::env::temp_dir().join(format!("s759d-{suffix:x}"))),
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
            runtime: Runtime::new(config()).expect("Runtime"),
        }
    }

    fn pump(&mut self) {
        self.runtime
            .poll_once(Some(Duration::from_millis(5)))
            .expect("Runtime poll");
    }

    fn spawn(&mut self) -> ExecutionId {
        self.runtime
            .create_execution(
                CommandSpec::new("/bin/cat"),
                WindowSize::new(80, 24, 0, 0).expect("size"),
            )
            .expect("execution")
    }

    fn connect(&mut self) -> Client {
        let path = self
            .runtime
            .local_ipc_socket_path()
            .expect("local IPC bound")
            .to_path_buf();
        let stream = UnixStream::connect(path).expect("connect");
        stream.set_nonblocking(true).expect("nonblocking");
        self.pump();
        Client {
            stream,
            buffered: Vec::new(),
        }
    }

    fn send(&mut self, client: &mut Client, kind: MessageType, payload: &[u8]) {
        let frame = encode_frame(kind, payload);
        client.stream.write_all(&frame).expect("write");
        self.pump();
    }

    fn expect_frame(&mut self, client: &mut Client, deadline: Instant) -> (u16, Vec<u8>) {
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
        let (kind, payload) = self.expect_frame(client, Instant::now() + Duration::from_secs(2));
        assert_eq!(kind, MessageType::ServerHello as u16);
        ServerHello::decode(&payload).expect("ServerHello");
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
        let (kind, payload) = self.expect_frame(client, deadline);
        assert_eq!(kind, MessageType::Attached as u16);
        let attached = Attached::decode(&payload).expect("Attached");
        let chunks = self.expect_display_batch(client, MessageType::DisplaySnapshot, deadline);
        let mut cache = empty_cache();
        cache.apply_chunks(&chunks).expect("initial snapshot apply");
        assert_eq!(cache.generation, attached.current_generation);
        (attached, cache)
    }

    fn expect_display_batch(
        &mut self,
        client: &mut Client,
        expected_kind: MessageType,
        deadline: Instant,
    ) -> Vec<DecodedDisplayChunk> {
        let (kind, payload) = self.expect_frame(client, deadline);
        assert_eq!(kind, expected_kind as u16);
        let first_frame = encode_frame(expected_kind, &payload);
        let first = decode_chunk(&first_frame).expect("display chunk");
        let count = first.chunk_count as usize;
        let mut chunks = vec![first];
        for _ in 1..count {
            let (kind, payload) = self.expect_frame(client, deadline);
            assert_eq!(kind, expected_kind as u16);
            chunks
                .push(decode_chunk(&encode_frame(expected_kind, &payload)).expect("display chunk"));
        }
        chunks
    }

    fn expect_error(&mut self, client: &mut Client, offending: MessageType) {
        let (kind, payload) = self.expect_frame(client, Instant::now() + Duration::from_secs(3));
        assert_eq!(kind, MessageType::Error as u16);
        let error = ErrorMessage::decode(&payload).expect("ErrorMessage");
        assert_eq!(error.error_code, ErrorCode::DisplayUnavailable as u16);
        assert_eq!(error.offending_message_type, offending as u16);
    }

    fn apply_recovery_or_display(
        &mut self,
        client: &mut Client,
        cache: &mut DisplayCache,
    ) -> MessageType {
        let deadline = Instant::now() + Duration::from_secs(5);
        let (kind, payload) = self.expect_frame(client, deadline);
        let message_type = MessageType::from_u16(kind).expect("known message type");
        assert!(
            matches!(
                message_type,
                MessageType::DisplaySnapshot | MessageType::DisplayDelta
            ),
            "expected display recovery, got {message_type:?}"
        );
        let first = decode_chunk(&encode_frame(message_type, &payload)).expect("display chunk");
        let count = first.chunk_count as usize;
        let mut chunks = vec![first];
        for _ in 1..count {
            let (next_kind, next_payload) = self.expect_frame(client, deadline);
            assert_eq!(next_kind, kind);
            chunks.push(
                decode_chunk(&encode_frame(message_type, &next_payload)).expect("display chunk"),
            );
        }
        cache.apply_chunks(&chunks).expect("display apply");
        message_type
    }
}

fn first_row_text(cache: &DisplayCache) -> String {
    if cache.rows == 0 || cache.columns == 0 {
        return String::new();
    }
    cache.cells[..cache.columns as usize]
        .iter()
        .map(|cell| cell.scalar)
        .collect()
}

#[test]
fn encode_failure_does_not_advance_published_and_viewers_recover_together() {
    let mut harness = Harness::new();
    let execution_id = harness.spawn();

    let mut controller = harness.connect();
    harness.hello(&mut controller);
    let (attached, mut controller_cache) =
        harness.attach(&mut controller, execution_id, Role::Controller);
    let attach_generation = controller_cache.generation;

    let mut observer = harness.connect();
    harness.hello(&mut observer);
    let (_observer_attached, mut observer_cache) =
        harness.attach(&mut observer, execution_id, Role::Observer);
    assert_eq!(observer_cache.generation, attach_generation);

    // Fail the next steady-state encode exactly once. Attach already published
    // a base generation, so the following PTY damage takes the delta path.
    test_fault::fail_next(FaultPoint::DisplayEncode);
    harness.send(
        &mut controller,
        MessageType::Input,
        &InputRef {
            attachment_id: attached.attachment_id,
            bytes: b"AB",
        }
        .encode(),
    );

    harness.expect_error(&mut controller, MessageType::DisplayDelta);
    harness.expect_error(&mut observer, MessageType::DisplayDelta);

    // Viewers must still be on the attach generation: encode failure must not
    // advance Runtime `published`, or a later delta would use a phantom base.
    assert_eq!(controller_cache.generation, attach_generation);
    assert_eq!(observer_cache.generation, attach_generation);

    // Recovery: scheduled snapshot resync and/or a later successful fanout must
    // re-establish a shared authoritative generation both viewers can apply.
    let mut recovered = false;
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        for _ in 0..4 {
            harness.pump();
        }
        // Prefer draining any pending recovery snapshot without requiring more
        // PTY damage; if none arrives yet, emit another write to force fanout.
        if controller.buffered.is_empty() && observer.buffered.is_empty() {
            harness.send(
                &mut controller,
                MessageType::Input,
                &InputRef {
                    attachment_id: attached.attachment_id,
                    bytes: b"CD",
                }
                .encode(),
            );
        }

        let controller_kind =
            harness.apply_recovery_or_display(&mut controller, &mut controller_cache);
        let observer_kind = harness.apply_recovery_or_display(&mut observer, &mut observer_cache);
        assert_eq!(
            controller_cache.generation, observer_cache.generation,
            "multi-viewer generations diverged after encode-failure recovery"
        );
        // A phantom advanced `published` base would typically deliver a delta
        // whose base no client holds. Recovery must land both viewers on a
        // coherent generation, usually via snapshot.
        assert!(
            matches!(
                (controller_kind, observer_kind),
                (MessageType::DisplaySnapshot, MessageType::DisplaySnapshot)
                    | (MessageType::DisplayDelta, MessageType::DisplayDelta)
            ),
            "viewers received mismatched recovery kinds: {controller_kind:?}/{observer_kind:?}"
        );
        let row = first_row_text(&controller_cache);
        if row.contains('A') || row.contains('C') {
            assert_eq!(first_row_text(&observer_cache), row);
            recovered = true;
            break;
        }
    }
    assert!(
        recovered,
        "viewers never recovered coherent display after encode failure"
    );
    assert!(controller_cache.generation > attach_generation);
    assert_eq!(controller_cache.generation, observer_cache.generation);
}
