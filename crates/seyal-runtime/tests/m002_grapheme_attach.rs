#![cfg(target_os = "macos")]

//! M002 #817 regression: a grapheme-capable client must receive a v2 initial
//! snapshot during attach when the Runtime projection contains a multi-scalar
//! grapheme.

use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    time::{Duration, Instant},
};

use seyal_exec::{CommandSpec, WindowSize};
use seyal_runtime::{
    display::{decode_chunk, empty_cache},
    local_ipc::framing::{
        encode_frame, Attach, Attached, ClientHello, FrameHeader, MessageType, Role,
        CAP_COMMAND_BLOCKS, CAP_GRAPHEME_DISPLAY, HEADER_LEN,
    },
    LocalIpcMode, Runtime, RuntimeConfig,
};

fn config() -> RuntimeConfig {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let suffix = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut config = RuntimeConfig::m001().expect("runtime config");
    config.singleton_path = std::env::temp_dir().join(format!("m002-817-{suffix:x}.lock"));
    config.local_ipc = LocalIpcMode::Enabled {
        runtime_dir_override: Some(std::env::temp_dir().join(format!("m002-817-{suffix:x}"))),
    };
    config
}

struct Harness {
    runtime: Runtime,
    stream: UnixStream,
    buffered: Vec<u8>,
}

impl Harness {
    fn new() -> (Self, seyal_runtime::ExecutionId) {
        let mut runtime = Runtime::new(config()).expect("Runtime");
        let execution_id = runtime
            .create_execution(
                CommandSpec::new("/bin/sh").args(["-c", "printf 'e\\314\\201'; sleep 5"]),
                WindowSize::cells(80, 24).expect("geometry"),
            )
            .expect("execution");
        let socket = runtime
            .local_ipc_socket_path()
            .expect("local IPC socket")
            .to_path_buf();
        let deadline = Instant::now() + Duration::from_secs(2);
        let stream = loop {
            match UnixStream::connect(&socket) {
                Ok(stream) => break stream,
                Err(_) => {
                    assert!(Instant::now() < deadline, "connect timeout");
                    runtime
                        .poll_once(Some(Duration::from_millis(5)))
                        .expect("poll");
                }
            }
        };
        stream.set_nonblocking(true).expect("nonblocking");
        let mut harness = Self {
            runtime,
            stream,
            buffered: Vec::new(),
        };
        // Let the real PTY output reach TerminalState before attach. The
        // combining mark makes the scalar-only v1 encoder reject this snapshot.
        for _ in 0..80 {
            harness.pump();
        }
        (harness, execution_id)
    }

    fn pump(&mut self) {
        self.runtime
            .poll_once(Some(Duration::from_millis(5)))
            .expect("poll");
    }

    fn send(&mut self, kind: MessageType, payload: &[u8]) {
        let frame = encode_frame(kind, payload);
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut offset = 0;
        while offset < frame.len() {
            match self.stream.write(&frame[offset..]) {
                Ok(0) => panic!("zero write"),
                Ok(written) => offset += written,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => self.pump(),
                Err(error) => panic!("write: {error}"),
            }
            assert!(Instant::now() < deadline, "write timeout");
        }
        self.pump();
    }

    fn frame(&mut self) -> (MessageType, Vec<u8>) {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if self.buffered.len() >= HEADER_LEN {
                let header = FrameHeader::decode(&self.buffered[..HEADER_LEN]).expect("header");
                let total = HEADER_LEN + header.payload_len as usize;
                if self.buffered.len() >= total {
                    let frame = self.buffered.drain(..total).collect::<Vec<_>>();
                    return (
                        MessageType::from_u16(header.message_type).expect("message type"),
                        frame[HEADER_LEN..].to_vec(),
                    );
                }
            }
            let mut chunk = [0u8; 16 * 1024];
            match self.stream.read(&mut chunk) {
                Ok(0) => panic!("socket closed"),
                Ok(read) => self.buffered.extend_from_slice(&chunk[..read]),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => self.pump(),
                Err(error) => panic!("read: {error}"),
            }
            assert!(Instant::now() < deadline, "read timeout");
        }
    }
}

#[test]
fn grapheme_capable_attach_uses_v2_initial_snapshot() {
    let (mut harness, execution_id) = Harness::new();
    harness.send(
        MessageType::ClientHello,
        &ClientHello {
            client_capabilities: CAP_COMMAND_BLOCKS | CAP_GRAPHEME_DISPLAY,
        }
        .encode(),
    );
    let (kind, _) = harness.frame();
    assert_eq!(kind, MessageType::ServerHello);

    harness.send(
        MessageType::Attach,
        &Attach {
            execution_id,
            requested_role: Role::Controller,
        }
        .encode(),
    );
    let (kind, payload) = harness.frame();
    assert_eq!(kind, MessageType::Attached);
    let attached = Attached::decode(&payload).expect("Attached");

    let (kind, payload) = harness.frame();
    assert_eq!(kind, MessageType::DisplaySnapshotV2);
    let decoded = decode_chunk(&encode_frame(kind, &payload)).expect("v2 snapshot");
    let mut cache = empty_cache();
    cache.apply_chunks(&[decoded]).expect("apply v2 snapshot");
    assert_eq!(cache.generation, attached.current_generation);
}
