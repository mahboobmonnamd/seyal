#![cfg(target_os = "macos")]

use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    time::{Duration, Instant},
};

use seyal_exec::{CommandSpec, WindowSize};
use seyal_runtime::{
    AttachmentId, LocalIpcMode, Runtime, RuntimeConfig,
    display::{DecodedDisplayChunk, DisplayCache, decode_chunk, empty_cache},
    local_ipc::framing::{
        Attach, Attached, CAP_CORRELATED_RESIZE, CAP_SEMANTIC_TERMINAL_KEY, ClientHello,
        FrameHeader, HEADER_LEN, MessageType, ResizeRequest, ResizeResult, ResizeResultCode, Role,
        ServerHello, TerminalKey, TerminalKeyKind, TerminalKeyModifiers, encode_frame,
    },
};

fn config() -> RuntimeConfig {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let suffix = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut config = RuntimeConfig::m001().expect("config");
    config.singleton_path = std::env::temp_dir().join(format!("s7-{suffix:x}.lock"));
    config.local_ipc = LocalIpcMode::Enabled {
        runtime_dir_override: Some(std::env::temp_dir().join(format!("s7d-{suffix:x}"))),
    };
    config
}

struct Harness {
    runtime: Runtime,
    stream: UnixStream,
    buffered: Vec<u8>,
}

impl Harness {
    fn new(command: CommandSpec) -> (Self, seyal_runtime::ExecutionId) {
        let mut runtime = Runtime::new(config()).expect("Runtime");
        let execution_id = runtime
            .create_execution(command, WindowSize::cells(80, 24).expect("size"))
            .expect("execution");
        let socket = runtime
            .local_ipc_socket_path()
            .expect("socket")
            .to_path_buf();
        let deadline = Instant::now() + Duration::from_secs(2);
        let stream = loop {
            match UnixStream::connect(&socket) {
                Ok(stream) => break stream,
                Err(_) => {
                    assert!(Instant::now() < deadline, "connect timeout");
                    runtime.poll_once(Some(Duration::from_millis(5))).unwrap();
                }
            }
        };
        stream.set_nonblocking(true).unwrap();
        runtime.poll_once(Some(Duration::from_millis(5))).unwrap();
        (
            Self {
                runtime,
                stream,
                buffered: Vec::new(),
            },
            execution_id,
        )
    }

    fn pump(&mut self) {
        self.runtime
            .poll_once(Some(Duration::from_millis(5)))
            .expect("poll");
    }

    fn send(&mut self, kind: MessageType, payload: &[u8]) {
        let bytes = encode_frame(kind, payload);
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut sent = 0;
        while sent < bytes.len() {
            match self.stream.write(&bytes[sent..]) {
                Ok(0) => panic!("zero write"),
                Ok(count) => sent += count,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => self.pump(),
                Err(error) => panic!("write: {error}"),
            }
            assert!(Instant::now() < deadline, "send timeout");
        }
        self.pump();
    }

    fn frame(&mut self) -> (u16, Vec<u8>) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if self.buffered.len() >= HEADER_LEN {
                let header = FrameHeader::decode(&self.buffered[..HEADER_LEN]).unwrap();
                let total = HEADER_LEN + header.payload_len as usize;
                if self.buffered.len() >= total {
                    let bytes = self.buffered.drain(..total).collect::<Vec<_>>();
                    return (header.message_type, bytes[HEADER_LEN..].to_vec());
                }
            }
            let mut buf = [0u8; 16 * 1024];
            match self.stream.read(&mut buf) {
                Ok(0) => panic!("closed"),
                Ok(count) => self.buffered.extend_from_slice(&buf[..count]),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => self.pump(),
                Err(error) => panic!("read: {error}"),
            }
            assert!(Instant::now() < deadline, "frame timeout");
        }
    }

    fn hello(&mut self) -> ServerHello {
        self.send(
            MessageType::ClientHello,
            &ClientHello {
                client_capabilities: 0,
            }
            .encode(),
        );
        let (kind, payload) = self.frame();
        assert_eq!(kind, MessageType::ServerHello as u16);
        ServerHello::decode(&payload).unwrap()
    }

    fn attach(
        &mut self,
        execution_id: seyal_runtime::ExecutionId,
        role: Role,
    ) -> (Attached, DisplayCache) {
        self.send(
            MessageType::Attach,
            &Attach {
                execution_id,
                requested_role: role,
            }
            .encode(),
        );
        let (kind, payload) = self.frame();
        assert_eq!(kind, MessageType::Attached as u16);
        let attached = Attached::decode(&payload).unwrap();
        let chunks = self.display_batch(MessageType::DisplaySnapshot);
        let mut cache = empty_cache();
        cache.apply_chunks(&chunks).unwrap();
        (attached, cache)
    }

    fn display_batch(&mut self, expected: MessageType) -> Vec<DecodedDisplayChunk> {
        let (kind, payload) = self.frame();
        assert_eq!(kind, expected as u16);
        let first = decode_chunk(&encode_frame(expected, &payload)).unwrap();
        let count = first.chunk_count;
        let mut chunks = vec![first];
        for _ in 1..count {
            let (kind, payload) = self.frame();
            assert_eq!(kind, expected as u16);
            chunks.push(decode_chunk(&encode_frame(expected, &payload)).unwrap());
        }
        chunks
    }

    fn next_display(&mut self, cache: &mut DisplayCache) {
        let (kind, payload) = self.frame();
        let kind = MessageType::from_u16(kind).expect("display message");
        assert!(matches!(
            kind,
            MessageType::DisplaySnapshot | MessageType::DisplayDelta
        ));
        let first = decode_chunk(&encode_frame(kind, &payload)).unwrap();
        let count = first.chunk_count;
        let mut chunks = vec![first];
        for _ in 1..count {
            let (next_kind, next_payload) = self.frame();
            assert_eq!(next_kind, kind as u16);
            chunks.push(decode_chunk(&encode_frame(kind, &next_payload)).unwrap());
        }
        cache.apply_chunks(&chunks).unwrap();
    }
}

#[test]
fn server_advertises_both_pass7_capabilities() {
    let (mut harness, _execution_id) = Harness::new(CommandSpec::new("/bin/cat"));
    let hello = harness.hello();
    assert_ne!(hello.server_capabilities & CAP_SEMANTIC_TERMINAL_KEY, 0);
    assert_ne!(hello.server_capabilities & CAP_CORRELATED_RESIZE, 0);
}

#[test]
fn controller_terminal_key_is_encoded_by_runtime_and_reaches_pty() {
    let command = CommandSpec::new("/bin/sh").args([
        "-c",
        "stty raw -echo; od -An -tu1 -N3 | tr -s ' ' | sed 's/^ //'",
    ]);
    let (mut harness, execution_id) = Harness::new(command);
    harness.hello();
    let (attached, mut cache) = harness.attach(execution_id, Role::Controller);
    harness.send(
        MessageType::TerminalKey,
        &TerminalKey {
            attachment_id: attached.attachment_id,
            kind: TerminalKeyKind::ArrowUp,
            modifiers: TerminalKeyModifiers::NONE,
            scalar: 0,
        }
        .encode(),
    );

    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let text: String = cache.cells.iter().map(|cell| cell.scalar).collect();
        if text.contains("27 91 65") {
            break;
        }
        assert!(Instant::now() < deadline, "semantic key bytes not observed");
        harness.next_display(&mut cache);
    }
}

#[test]
fn correlated_resize_returns_applied_generation_before_projection() {
    let (mut harness, execution_id) = Harness::new(CommandSpec::new("/bin/cat"));
    harness.hello();
    let (attached, mut cache) = harness.attach(execution_id, Role::Controller);
    harness.send(
        MessageType::ResizeRequest,
        &ResizeRequest {
            attachment_id: attached.attachment_id,
            request_id: 1,
            rows: 40,
            columns: 120,
        }
        .encode(),
    );

    let (kind, payload) = harness.frame();
    assert_eq!(kind, MessageType::ResizeResult as u16);
    let result = ResizeResult::decode(&payload).unwrap();
    assert_eq!(result.request_id, 1);
    assert_eq!(result.result_code, ResizeResultCode::Applied);
    assert!(result.applied_generation > cache.generation);

    while cache.generation < result.applied_generation {
        harness.next_display(&mut cache);
    }
    assert_eq!((cache.rows, cache.columns), (40, 120));
}

#[test]
fn duplicate_resize_request_id_is_correlated_malformed_failure() {
    let (mut harness, execution_id) = Harness::new(CommandSpec::new("/bin/cat"));
    harness.hello();
    let (attached, _cache) = harness.attach(execution_id, Role::Controller);
    let request = ResizeRequest {
        attachment_id: attached.attachment_id,
        request_id: 7,
        rows: 30,
        columns: 100,
    };
    harness.send(MessageType::ResizeRequest, &request.encode());
    let (kind, payload) = harness.frame();
    assert_eq!(kind, MessageType::ResizeResult as u16);
    assert_eq!(
        ResizeResult::decode(&payload).unwrap().result_code,
        ResizeResultCode::Applied
    );

    // Consume the projection from the first successful resize before testing
    // the duplicate result, so mandatory-vs-presentation ordering is explicit.
    let _ = harness.display_batch(MessageType::DisplaySnapshot);
    harness.send(MessageType::ResizeRequest, &request.encode());
    let (kind, payload) = harness.frame();
    assert_eq!(kind, MessageType::ResizeResult as u16);
    assert_eq!(
        ResizeResult::decode(&payload).unwrap().result_code,
        ResizeResultCode::Error(seyal_runtime::local_ipc::framing::ErrorCode::MalformedPayload)
    );
}

#[test]
fn unauthorized_resize_cannot_poison_request_id_sequence() {
    let (mut harness, execution_id) = Harness::new(CommandSpec::new("/bin/cat"));
    harness.hello();
    let (attached, _cache) = harness.attach(execution_id, Role::Controller);

    // This frame is structurally valid but carries a stale attachment. Its
    // large request ID must not advance the live connection's ordering state.
    harness.send(
        MessageType::ResizeRequest,
        &ResizeRequest {
            attachment_id: AttachmentId::from_bytes(999u128.to_le_bytes()),
            request_id: 99,
            rows: 30,
            columns: 100,
        }
        .encode(),
    );
    let (kind, payload) = harness.frame();
    assert_eq!(kind, MessageType::ResizeResult as u16);
    assert_eq!(
        ResizeResult::decode(&payload).unwrap().result_code,
        ResizeResultCode::Error(seyal_runtime::local_ipc::framing::ErrorCode::StaleIdentity)
    );

    // The first valid request ID remains usable after the rejected request.
    harness.send(
        MessageType::ResizeRequest,
        &ResizeRequest {
            attachment_id: attached.attachment_id,
            request_id: 1,
            rows: 30,
            columns: 100,
        }
        .encode(),
    );
    let (kind, payload) = harness.frame();
    assert_eq!(kind, MessageType::ResizeResult as u16);
    assert_eq!(
        ResizeResult::decode(&payload).unwrap().result_code,
        ResizeResultCode::Applied
    );
}
