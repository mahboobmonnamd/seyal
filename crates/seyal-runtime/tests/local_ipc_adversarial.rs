#![cfg(target_os = "macos")]

//! Cross-connection authority regression coverage for SPEC-004.
//!
//! Attachment identifiers are opaque identities, not bearer capabilities.
//! Knowing another connection's controller `AttachmentId` must not grant any
//! input, resize, resync, or detach authority.

use std::{
    io::{Read, Write},
    os::fd::{AsRawFd, OwnedFd},
    os::unix::net::UnixStream,
    time::{Duration, Instant},
};

use seyal_exec::{CommandSpec, WindowSize};
use seyal_runtime::{
    ExecutionId, LocalIpcMode, Runtime, RuntimeConfig,
    local_ipc::{
        fd_transfer::{self, RecvFd},
        framing::{
            Attach, Attached, ClientHello, Detach, ErrorCode, ErrorMessage, FrameHeader,
            HEADER_LEN, InputRef, MessageType, Resize, Resync, Role, ServerHello, encode_frame,
        },
    },
    projection::{
        layout::{REGION_HEADER_LEN, RegionHeader},
        lifecycle::ReadOnlyMapping,
        writer::read_latest,
    },
};

fn config() -> RuntimeConfig {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let suffix = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut config = RuntimeConfig::m001().expect("bundled capability profile");
    config.singleton_path = std::env::temp_dir().join(format!("s5a-{suffix:x}.lock"));
    config.local_ipc = LocalIpcMode::Enabled {
        runtime_dir_override: Some(std::env::temp_dir().join(format!("s5ad-{suffix:x}"))),
    };
    config.graceful_termination = Duration::from_millis(50);
    config.forced_reap = Duration::from_millis(250);
    config.final_drain = Duration::from_millis(100);
    config
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

    fn spawn_cat(&mut self) -> ExecutionId {
        self.runtime
            .create_execution(
                CommandSpec::new("/bin/cat"),
                WindowSize::new(8, 2, 0, 0).expect("valid geometry"),
            )
            .expect("execution")
    }

    fn pump(&mut self) {
        self.runtime
            .poll_once(Some(Duration::from_millis(5)))
            .expect("Runtime poll");
    }

    fn connect(&mut self) -> UnixStream {
        let path = self
            .runtime
            .local_ipc_socket_path()
            .expect("local IPC socket")
            .to_path_buf();
        let stream = UnixStream::connect(path).expect("connect local IPC");
        stream.set_nonblocking(true).expect("nonblocking client");
        stream
    }

    fn send(&mut self, stream: &mut UnixStream, message_type: MessageType, payload: &[u8]) {
        stream
            .write_all(&encode_frame(message_type, payload))
            .expect("write frame");
        self.pump();
    }

    fn frame(&mut self, stream: &mut UnixStream) -> (u16, Vec<u8>) {
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut buffer = Vec::new();
        loop {
            let mut chunk = [0u8; 4096];
            match stream.read(&mut chunk) {
                Ok(0) => panic!("connection closed"),
                Ok(count) => buffer.extend_from_slice(&chunk[..count]),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(error) => panic!("client read failed: {error}"),
            }
            if buffer.len() >= HEADER_LEN {
                let header = FrameHeader::decode(&buffer[..HEADER_LEN]).expect("valid header");
                let total = HEADER_LEN + header.payload_len as usize;
                if buffer.len() >= total {
                    return (header.message_type, buffer[HEADER_LEN..total].to_vec());
                }
            }
            assert!(Instant::now() < deadline, "timed out awaiting frame");
            self.pump();
        }
    }

    fn fd_frame(&mut self, stream: &UnixStream) -> (u16, Vec<u8>, OwnedFd) {
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut buffer = Vec::new();
        let mut fd = None;
        loop {
            if buffer.len() >= HEADER_LEN {
                let header = FrameHeader::decode(&buffer[..HEADER_LEN]).expect("valid header");
                let total = HEADER_LEN + header.payload_len as usize;
                if buffer.len() >= total {
                    return (
                        header.message_type,
                        buffer[HEADER_LEN..total].to_vec(),
                        fd.expect("fd-bearing frame without descriptor"),
                    );
                }
            }
            let mut chunk = [0u8; 4096];
            match fd_transfer::recv_with_fd(stream.as_raw_fd(), &mut chunk) {
                Ok((0, _)) => panic!("connection closed"),
                Ok((count, RecvFd::One(received))) => {
                    assert!(fd.replace(received).is_none(), "multiple descriptors");
                    buffer.extend_from_slice(&chunk[..count]);
                }
                Ok((count, RecvFd::None)) => buffer.extend_from_slice(&chunk[..count]),
                Ok((_, RecvFd::Malformed)) => panic!("malformed descriptor transfer"),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(error) => panic!("recvmsg failed: {error}"),
            }
            assert!(Instant::now() < deadline, "timed out awaiting fd frame");
            self.pump();
        }
    }

    fn hello(&mut self, stream: &mut UnixStream) {
        self.send(
            stream,
            MessageType::ClientHello,
            &ClientHello {
                client_capabilities: 0,
            }
            .encode(),
        );
        let (kind, payload) = self.frame(stream);
        assert_eq!(kind, MessageType::ServerHello as u16);
        ServerHello::decode(&payload).expect("valid ServerHello");
    }

    fn attach(
        &mut self,
        stream: &mut UnixStream,
        execution_id: ExecutionId,
        role: Role,
    ) -> (Attached, ReadOnlyMapping) {
        self.send(
            stream,
            MessageType::Attach,
            &Attach {
                execution_id,
                requested_role: role,
            }
            .encode(),
        );
        let (kind, payload, fd) = self.fd_frame(stream);
        assert_eq!(kind, MessageType::Attached as u16);
        let attached = Attached::decode(&payload).expect("valid Attached");
        let mapping = ReadOnlyMapping::new(fd, attached.region_bytes as usize).expect("projection");
        (attached, mapping)
    }

    fn expect_stale(&mut self, stream: &mut UnixStream, message_type: MessageType, payload: &[u8]) {
        self.send(stream, message_type, payload);
        let (kind, payload) = self.frame(stream);
        assert_eq!(kind, MessageType::Error as u16);
        let error = ErrorMessage::decode(&payload).expect("valid Error");
        assert_eq!(error.error_code, ErrorCode::StaleIdentity as u16);
        assert_eq!(error.offending_message_type, message_type as u16);
    }
}

#[test]
fn another_attached_connection_cannot_reuse_controller_attachment_identity() {
    let mut harness = Harness::new();
    let execution_id = harness.spawn_cat();

    let mut owner = harness.connect();
    harness.hello(&mut owner);
    let (controller, controller_mapping) =
        harness.attach(&mut owner, execution_id, Role::Controller);

    let mut attacker = harness.connect();
    harness.hello(&mut attacker);
    let (_observer, _observer_mapping) =
        harness.attach(&mut attacker, execution_id, Role::Observer);

    harness.expect_stale(
        &mut attacker,
        MessageType::Input,
        &InputRef {
            attachment_id: controller.attachment_id,
            bytes: b"EVIL",
        }
        .encode(),
    );
    harness.expect_stale(
        &mut attacker,
        MessageType::Resize,
        &Resize {
            attachment_id: controller.attachment_id,
            rows: 3,
            columns: 12,
        }
        .encode(),
    );
    harness.expect_stale(
        &mut attacker,
        MessageType::Resync,
        &Resync {
            attachment_id: controller.attachment_id,
        }
        .encode(),
    );
    harness.expect_stale(
        &mut attacker,
        MessageType::Detach,
        &Detach {
            attachment_id: controller.attachment_id,
        }
        .encode(),
    );

    // The failed cross-connection attempts must not revoke or weaken the
    // legitimate controller. Prove it still drives the real PTY and its
    // projection advances to the owner's input.
    harness.send(
        &mut owner,
        MessageType::Input,
        &InputRef {
            attachment_id: controller.attachment_id,
            bytes: b"OK",
        }
        .encode(),
    );
    let region_bytes = controller_mapping
        .memory()
        .read_bytes(0..REGION_HEADER_LEN)
        .expect("region header bytes");
    let region = RegionHeader::decode(&region_bytes).expect("region header");
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Ok(snapshot) = read_latest(&controller_mapping.memory(), &region) {
            let row: String = snapshot
                .cells
                .iter()
                .take(snapshot.header.columns as usize)
                .map(|cell| cell.scalar)
                .collect();
            if row.starts_with("OK") {
                break;
            }
        }
        assert!(
            Instant::now() < deadline,
            "owner projection did not advance"
        );
        harness.pump();
    }

    drop(attacker);
    drop(owner);
    harness.pump();
    harness.runtime.begin_shutdown().expect("begin shutdown");
    harness
        .runtime
        .run_until_empty(Instant::now() + Duration::from_secs(2))
        .expect("controlled shutdown");
}
