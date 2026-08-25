#![cfg(target_os = "macos")]

//! Regression for SPEC-004 final display publication ordering.
//!
//! The command emits more than Runtime's 64 KiB PTY read quantum and places a
//! unique marker in the tail. An attached client must still observe that tail
//! from its already-mapped projection after the execution has finalized.

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
            Attach, Attached, ClientHello, FrameHeader, HEADER_LEN, MessageType, Role, ServerHello,
            encode_frame,
        },
    },
    projection::{
        lifecycle::ReadOnlyMapping,
        writer::{read_latest, read_region_header},
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

    fn connect(&mut self) -> UnixStream {
        let path = self
            .runtime
            .local_ipc_socket_path()
            .expect("local IPC path")
            .to_path_buf();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match UnixStream::connect(&path) {
                Ok(stream) => {
                    stream.set_nonblocking(true).expect("nonblocking client");
                    return stream;
                }
                Err(_) => {
                    assert!(Instant::now() < deadline, "connect timed out");
                    self.pump();
                }
            }
        }
    }

    fn send(&mut self, stream: &mut UnixStream, kind: MessageType, payload: &[u8]) {
        stream
            .write_all(&encode_frame(kind, payload))
            .expect("send protocol frame");
        self.pump();
    }

    fn expect_frame(&mut self, stream: &mut UnixStream) -> (u16, Vec<u8>) {
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut buffer = Vec::new();
        loop {
            let mut chunk = [0u8; 4096];
            match stream.read(&mut chunk) {
                Ok(0) => panic!("connection closed while waiting for frame"),
                Ok(count) => buffer.extend_from_slice(&chunk[..count]),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(error) => panic!("client read failed: {error}"),
            }
            if buffer.len() >= HEADER_LEN {
                let header = FrameHeader::decode(&buffer[..HEADER_LEN]).expect("frame header");
                let total = HEADER_LEN + header.payload_len as usize;
                if buffer.len() >= total {
                    return (header.message_type, buffer[HEADER_LEN..total].to_vec());
                }
            }
            assert!(Instant::now() < deadline, "frame timed out");
            self.pump();
        }
    }

    fn expect_frame_with_fd(&mut self, stream: &UnixStream) -> (u16, Vec<u8>, OwnedFd) {
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut buffer = Vec::new();
        let mut descriptor = None;
        loop {
            if buffer.len() >= HEADER_LEN {
                let header = FrameHeader::decode(&buffer[..HEADER_LEN]).expect("frame header");
                let total = HEADER_LEN + header.payload_len as usize;
                if buffer.len() >= total {
                    return (
                        header.message_type,
                        buffer[HEADER_LEN..total].to_vec(),
                        descriptor.expect("fd-bearing frame omitted descriptor"),
                    );
                }
            }

            let mut chunk = [0u8; 4096];
            match fd_transfer::recv_with_fd(stream.as_raw_fd(), &mut chunk) {
                Ok((0, _)) => panic!("connection closed while waiting for descriptor"),
                Ok((count, RecvFd::One(fd))) => {
                    assert!(descriptor.replace(fd).is_none(), "multiple descriptors");
                    buffer.extend_from_slice(&chunk[..count]);
                }
                Ok((count, RecvFd::None)) => buffer.extend_from_slice(&chunk[..count]),
                Ok((_, RecvFd::Malformed)) => panic!("malformed descriptor transfer"),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(error) => panic!("recvmsg failed: {error}"),
            }
            assert!(Instant::now() < deadline, "fd-bearing frame timed out");
            self.pump();
        }
    }

    fn attach(&mut self, stream: &mut UnixStream, execution_id: ExecutionId) -> ReadOnlyMapping {
        self.send(
            stream,
            MessageType::ClientHello,
            &ClientHello {
                client_capabilities: 0,
            }
            .encode(),
        );
        let (message_type, payload) = self.expect_frame(stream);
        assert_eq!(message_type, MessageType::ServerHello as u16);
        ServerHello::decode(&payload).expect("ServerHello");

        self.send(
            stream,
            MessageType::Attach,
            &Attach {
                execution_id,
                requested_role: Role::Observer,
            }
            .encode(),
        );
        let (message_type, payload, fd) = self.expect_frame_with_fd(stream);
        assert_eq!(message_type, MessageType::Attached as u16);
        let attached = Attached::decode(&payload).expect("Attached");
        ReadOnlyMapping::new(fd, attached.region_bytes as usize).expect("projection mapping")
    }
}

#[test]
fn final_tail_bytes_are_published_before_execution_teardown() {
    let mut harness = Harness::new();
    let execution_id = harness.spawn();
    let mut client = harness.connect();
    let mapping = harness.attach(&mut client, execution_id);
    let region = read_region_header(&mapping.memory()).expect("region header");

    let deadline = Instant::now() + Duration::from_secs(5);
    while harness.runtime.execution_count() != 0 {
        assert!(Instant::now() < deadline, "execution never finalized");
        harness.pump();
    }

    let snapshot = read_latest(&mapping.memory(), &region).expect("final readable projection");
    let visible = snapshot
        .cells
        .iter()
        .map(|cell| cell.scalar)
        .collect::<String>();
    assert!(
        visible.contains("FINAL"),
        "final projection omitted terminal tail marker: {visible:?}"
    );
}
