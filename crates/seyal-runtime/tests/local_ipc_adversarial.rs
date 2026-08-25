#![cfg(target_os = "macos")]

//! Attachment IDs are identities, not bearer capabilities. A second connection
//! cannot use another connection's controller ID for input/resize/resync/detach.

use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    time::{Duration, Instant},
};

use seyal_exec::{CommandSpec, WindowSize};
use seyal_runtime::{
    ExecutionId, LocalIpcMode, Runtime, RuntimeConfig,
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
