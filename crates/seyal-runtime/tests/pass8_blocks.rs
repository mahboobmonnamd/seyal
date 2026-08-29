#![cfg(target_os = "macos")]

//! M001 Pass 8 production-path Block metadata acceptance coverage.

use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    time::{Duration, Instant},
};

use seyal_exec::{CommandSpec, WindowSize};
use seyal_protocol::pass8::{
    BLOCK_STATE_MESSAGE_TYPE, BlockLifecycle as WireBlockLifecycle, BlockState, CAP_BLOCK_METADATA,
};
use seyal_runtime::{
    BlockLifecycle, ExecutionId, LocalIpcMode, Runtime, RuntimeConfig,
    display::{DecodedDisplayChunk, DisplayCache, decode_chunk, empty_cache},
    local_ipc::framing::{
        Attach, Attached, ClientHello, FrameHeader, HEADER_LEN, Lifecycle, LifecycleMessage,
        MessageType, Role, ServerHello, encode_frame,
    },
};

fn config() -> RuntimeConfig {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let suffix = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut config = RuntimeConfig::m001().expect("bundled capability profile");
    config.singleton_path = std::env::temp_dir().join(format!("s8b-{suffix:x}.lock"));
    config.local_ipc = LocalIpcMode::Enabled {
        runtime_dir_override: Some(std::env::temp_dir().join(format!("s8bd-{suffix:x}"))),
    };
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

    fn spawn(&mut self) -> ExecutionId {
        self.runtime
            .create_execution(
                CommandSpec::new("/bin/sh").args([
                    "-c",
                    "/usr/bin/yes A | /usr/bin/head -c 70000; printf FINAL",
                ]),
                WindowSize::new(16, 4, 0, 0).expect("valid geometry"),
            )
            .expect("execution")
    }

    fn connect(&mut self) -> Client {
        let path = self
            .runtime
            .local_ipc_socket_path()
            .expect("local IPC path")
            .to_path_buf();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match UnixStream::connect(&path) {
                Ok(stream) => {
                    stream.set_nonblocking(true).unwrap();
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
        client
            .stream
            .write_all(&encode_frame(kind, payload))
            .expect("send frame");
        self.pump();
    }

    fn raw_frame(&mut self, client: &mut Client, deadline: Instant) -> (u16, Vec<u8>) {
        loop {
            if client.buffered.len() >= HEADER_LEN {
                let header = FrameHeader::decode(&client.buffered[..HEADER_LEN]).unwrap();
                let total = HEADER_LEN + header.payload_len as usize;
                if client.buffered.len() >= total {
                    let raw = client.buffered.drain(..total).collect::<Vec<_>>();
                    return (header.message_type, raw[HEADER_LEN..].to_vec());
                }
            }
            let mut chunk = [0u8; 16 * 1024];
            match client.stream.read(&mut chunk) {
                Ok(0) => panic!("connection closed"),
                Ok(count) => client.buffered.extend_from_slice(&chunk[..count]),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => self.pump(),
                Err(error) => panic!("read failed: {error}"),
            }
            assert!(Instant::now() < deadline, "frame timed out");
        }
    }

    fn display_batch(
        &mut self,
        client: &mut Client,
        first_kind: u16,
        first_payload: Vec<u8>,
        deadline: Instant,
    ) -> Vec<DecodedDisplayChunk> {
        let kind = MessageType::from_u16(first_kind).expect("display message type");
        let first = decode_chunk(&encode_frame(kind, &first_payload)).unwrap();
        let mut chunks = vec![first];
        let expected = chunks[0].chunk_count as usize;
        while chunks.len() < expected {
            let (next_kind, payload) = self.raw_frame(client, deadline);
            assert_eq!(next_kind, first_kind);
            chunks.push(
                decode_chunk(&encode_frame(kind, &payload)).expect("continued display chunk"),
            );
        }
        chunks
    }

    fn attach(
        &mut self,
        client: &mut Client,
        execution_id: ExecutionId,
    ) -> (DisplayCache, BlockState) {
        self.send(
            client,
            MessageType::ClientHello,
            &ClientHello {
                client_capabilities: CAP_BLOCK_METADATA,
            }
            .encode(),
        );
        let deadline = Instant::now() + Duration::from_secs(2);
        let (kind, payload) = self.raw_frame(client, deadline);
        assert_eq!(kind, MessageType::ServerHello as u16);
        let hello = ServerHello::decode(&payload).unwrap();
        assert_ne!(hello.server_capabilities & CAP_BLOCK_METADATA, 0);

        self.send(
            client,
            MessageType::Attach,
            &Attach {
                execution_id,
                requested_role: Role::Observer,
            }
            .encode(),
        );
        let (kind, payload) = self.raw_frame(client, deadline);
        assert_eq!(kind, MessageType::Attached as u16);
        let attached = Attached::decode(&payload).unwrap();
        let (kind, payload) = self.raw_frame(client, deadline);
        assert_eq!(kind, MessageType::DisplaySnapshot as u16);
        let chunks = self.display_batch(client, kind, payload, deadline);
        let mut cache = empty_cache();
        cache.apply_chunks(&chunks).unwrap();
        assert_eq!(cache.generation, attached.current_generation);

        let (kind, payload) = self.raw_frame(client, deadline);
        assert_eq!(kind, BLOCK_STATE_MESSAGE_TYPE);
        let current = BlockState::decode(&payload).expect("current BlockState");
        assert_eq!(current.execution_id, execution_id);
        assert_eq!(current.state, WireBlockLifecycle::Current);
        assert_eq!(current.revision, 1);
        (cache, current)
    }
}

#[test]
fn block_identity_anchor_completion_order_and_retirement_follow_spec007() {
    let mut harness = Harness::new();
    let execution_id = harness.spawn();

    let runtime_block = harness.runtime.block(execution_id).expect("Block admitted");
    assert_eq!(
        runtime_block.workspace_id,
        harness.runtime.default_workspace_id()
    );
    assert_eq!(runtime_block.execution_id, execution_id);
    assert_eq!(runtime_block.lifecycle, BlockLifecycle::Current);
    assert_eq!(runtime_block.revision, 1);
    assert_ne!(runtime_block.id.to_bytes(), [0; 16]);
    let initial_line = harness
        .runtime
        .execution(execution_id)
        .expect("execution")
        .initial_primary_line_id()
        .expect("initial primary line");
    assert_eq!(runtime_block.start_line_id, initial_line.0);

    let mut client = harness.connect();
    let (mut cache, current) = harness.attach(&mut client, execution_id);
    assert_eq!(current.block_id, runtime_block.id);
    assert_eq!(current.start_line_id, runtime_block.start_line_id);

    let deadline = Instant::now() + Duration::from_secs(8);
    let mut completed = None;
    loop {
        let (kind, payload) = harness.raw_frame(&mut client, deadline);
        if kind == MessageType::DisplaySnapshot as u16 || kind == MessageType::DisplayDelta as u16 {
            assert!(
                completed.is_none(),
                "display arrived after Completed BlockState"
            );
            let chunks = harness.display_batch(&mut client, kind, payload, deadline);
            cache.apply_chunks(&chunks).expect("display apply");
        } else if kind == BLOCK_STATE_MESSAGE_TYPE {
            assert!(completed.is_none(), "duplicate completion metadata");
            let value = BlockState::decode(&payload).expect("completed BlockState");
            assert_eq!(value.execution_id, execution_id);
            assert_eq!(value.block_id, current.block_id);
            assert_eq!(value.start_line_id, current.start_line_id);
            assert_eq!(value.state, WireBlockLifecycle::Completed);
            assert_eq!(value.revision, 2);
            completed = Some(value);
        } else if kind == MessageType::Lifecycle as u16 {
            let lifecycle = LifecycleMessage::decode(&payload).unwrap();
            assert_eq!(lifecycle.execution_id, execution_id);
            assert_eq!(lifecycle.lifecycle, Lifecycle::Finalized);
            assert!(
                completed.is_some(),
                "Finalized arrived before Completed BlockState"
            );
            break;
        } else {
            panic!("unexpected frame before finalization: {kind}");
        }
        assert!(Instant::now() < deadline, "finalization timed out");
    }

    let visible = cache
        .cells
        .iter()
        .map(|cell| cell.scalar)
        .collect::<String>();
    assert!(
        visible.contains("FINAL"),
        "final display state omitted terminal tail marker: {visible:?}"
    );
    assert_eq!(harness.runtime.execution_count(), 0);
    assert_eq!(harness.runtime.block_count(), 0);
    assert_eq!(harness.runtime.block(execution_id), None);
}
