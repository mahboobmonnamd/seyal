#![cfg(target_os = "macos")]

//! Regression for SPEC-004 final display ordering. The tail marker must reach
//! the disposable display cache before Runtime emits `Lifecycle::Finalized`.

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
        Attach, Attached, ClientHello, FrameHeader, HEADER_LEN, Lifecycle, LifecycleMessage,
        MessageType, Role, ServerHello, encode_frame,
    },
};

fn config() -> RuntimeConfig {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let suffix = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut config = RuntimeConfig::m001().expect("bundled capability profile");
    config.singleton_path = std::env::temp_dir().join(format!("s5f-{suffix:x}.lock"));
    config.local_ipc = LocalIpcMode::Enabled {
        runtime_dir_override: Some(std::env::temp_dir().join(format!("s5fd-{suffix:x}"))),
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

    fn frame(&mut self, client: &mut Client, deadline: Instant) -> (MessageType, Vec<u8>) {
        loop {
            if client.buffered.len() >= HEADER_LEN {
                let header = FrameHeader::decode(&client.buffered[..HEADER_LEN]).unwrap();
                let total = HEADER_LEN + header.payload_len as usize;
                if client.buffered.len() >= total {
                    let raw = client.buffered.drain(..total).collect::<Vec<_>>();
                    return (
                        MessageType::from_u16(header.message_type).expect("known frame"),
                        raw[HEADER_LEN..].to_vec(),
                    );
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
        first_kind: MessageType,
        first_payload: Vec<u8>,
        deadline: Instant,
    ) -> Vec<DecodedDisplayChunk> {
        let first = decode_chunk(&encode_frame(first_kind, &first_payload)).unwrap();
        let mut chunks = vec![first];
        let expected = chunks[0].chunk_count as usize;
        while chunks.len() < expected {
            let (kind, payload) = self.frame(client, deadline);
            assert_eq!(kind, first_kind);
            chunks.push(decode_chunk(&encode_frame(kind, &payload)).unwrap());
        }
        chunks
    }

    fn attach(&mut self, client: &mut Client, execution_id: ExecutionId) -> DisplayCache {
        self.send(
            client,
            MessageType::ClientHello,
            &ClientHello {
                client_capabilities: 0,
            }
            .encode(),
        );
        let deadline = Instant::now() + Duration::from_secs(2);
        let (kind, payload) = self.frame(client, deadline);
        assert_eq!(kind, MessageType::ServerHello);
        ServerHello::decode(&payload).unwrap();

        self.send(
            client,
            MessageType::Attach,
            &Attach {
                execution_id,
                requested_role: Role::Observer,
            }
            .encode(),
        );
        let (kind, payload) = self.frame(client, deadline);
        assert_eq!(kind, MessageType::Attached);
        let attached = Attached::decode(&payload).unwrap();
        let (kind, payload) = self.frame(client, deadline);
        assert_eq!(kind, MessageType::DisplaySnapshot);
        let chunks = self.display_batch(client, kind, payload, deadline);
        let mut cache = empty_cache();
        cache.apply_chunks(&chunks).unwrap();
        assert_eq!(cache.generation, attached.current_generation);
        cache
    }
}

#[test]
fn final_tail_bytes_are_committed_before_finalized_lifecycle() {
    let mut harness = Harness::new();
    let execution_id = harness.spawn();
    let mut client = harness.connect();
    let mut cache = harness.attach(&mut client, execution_id);
    let deadline = Instant::now() + Duration::from_secs(8);

    loop {
        let (kind, payload) = harness.frame(&mut client, deadline);
        match kind {
            MessageType::DisplaySnapshot | MessageType::DisplayDelta => {
                let chunks = harness.display_batch(&mut client, kind, payload, deadline);
                cache.apply_chunks(&chunks).expect("display apply");
            }
            MessageType::Lifecycle => {
                let lifecycle = LifecycleMessage::decode(&payload).unwrap();
                assert_eq!(lifecycle.execution_id, execution_id);
                assert_eq!(lifecycle.lifecycle, Lifecycle::Finalized);
                break;
            }
            other => panic!("unexpected frame before finalization: {other:?}"),
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
}
