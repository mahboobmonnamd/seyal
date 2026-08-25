#![cfg(all(target_os = "macos", feature = "test-fault-injection"))]

//! Deterministic Pass-5 failure/rollback matrix.
//!
//! These tests use the real Runtime, PTY, Unix-domain transport, projection
//! lifecycle and SCM_RIGHTS send path. Failure injection is a non-default
//! compile-time feature; normal Seyal builds contain no failpoint branches.

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
            Attach, Attached, ClientHello, ErrorCode, ErrorMessage, FrameHeader, HEADER_LEN,
            MessageType, Role, ServerHello, encode_frame,
        },
    },
    test_fault::{self, FaultPoint},
};

fn config() -> RuntimeConfig {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let suffix = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut config = RuntimeConfig::m001().expect("bundled capability policy");
    config.singleton_path = std::env::temp_dir().join(format!("s5fi-{suffix:x}.lock"));
    config.local_ipc = LocalIpcMode::Enabled {
        runtime_dir_override: Some(std::env::temp_dir().join(format!("s5fid-{suffix:x}"))),
    };
    config.graceful_termination = Duration::from_millis(50);
    config.forced_reap = Duration::from_millis(250);
    config.final_drain = Duration::from_millis(100);
    config
}

fn pump(runtime: &mut Runtime) {
    let _ = runtime.poll_once(Some(Duration::from_millis(5)));
}

fn connect(runtime: &mut Runtime) -> UnixStream {
    let path = runtime
        .local_ipc_socket_path()
        .expect("local IPC socket")
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
                pump(runtime);
            }
        }
    }
}

fn send_frame(
    runtime: &mut Runtime,
    stream: &mut UnixStream,
    message_type: MessageType,
    payload: &[u8],
) {
    stream
        .write_all(&encode_frame(message_type, payload))
        .expect("write protocol frame");
    pump(runtime);
}

fn expect_plain_frame(
    runtime: &mut Runtime,
    stream: &mut UnixStream,
    deadline: Instant,
) -> (u16, Vec<u8>) {
    let mut buffer = Vec::new();
    loop {
        let mut chunk = [0u8; 4096];
        match stream.read(&mut chunk) {
            Ok(0) => panic!("connection closed while awaiting frame"),
            Ok(count) => buffer.extend_from_slice(&chunk[..count]),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("client read failed: {error}"),
        }
        if buffer.len() >= HEADER_LEN {
            let header = FrameHeader::decode(&buffer[..HEADER_LEN]).expect("valid frame header");
            let total = HEADER_LEN + header.payload_len as usize;
            if buffer.len() >= total {
                return (header.message_type, buffer[HEADER_LEN..total].to_vec());
            }
        }
        assert!(Instant::now() < deadline, "frame wait timed out");
        pump(runtime);
    }
}

fn expect_fd_frame(
    runtime: &mut Runtime,
    stream: &UnixStream,
    deadline: Instant,
) -> (u16, Vec<u8>, OwnedFd) {
    let mut buffer = Vec::new();
    let mut received_fd = None;
    loop {
        if buffer.len() >= HEADER_LEN {
            let header = FrameHeader::decode(&buffer[..HEADER_LEN]).expect("valid frame header");
            let total = HEADER_LEN + header.payload_len as usize;
            if buffer.len() >= total {
                return (
                    header.message_type,
                    buffer[HEADER_LEN..total].to_vec(),
                    received_fd.expect("fd-bearing frame completed without descriptor"),
                );
            }
        }

        let mut chunk = [0u8; 4096];
        match fd_transfer::recv_with_fd(stream.as_raw_fd(), &mut chunk) {
            Ok((0, _)) => panic!("connection closed while awaiting fd frame"),
            Ok((count, RecvFd::One(fd))) => {
                assert!(received_fd.replace(fd).is_none(), "multiple descriptors");
                buffer.extend_from_slice(&chunk[..count]);
            }
            Ok((count, RecvFd::None)) => buffer.extend_from_slice(&chunk[..count]),
            Ok((_, RecvFd::Malformed)) => panic!("malformed descriptor transfer"),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("recvmsg failed: {error}"),
        }
        assert!(Instant::now() < deadline, "fd frame wait timed out");
        pump(runtime);
    }
}

fn hello(runtime: &mut Runtime, stream: &mut UnixStream) {
    send_frame(
        runtime,
        stream,
        MessageType::ClientHello,
        &ClientHello {
            client_capabilities: 0,
        }
        .encode(),
    );
    let (message_type, payload) =
        expect_plain_frame(runtime, stream, Instant::now() + Duration::from_secs(2));
    assert_eq!(message_type, MessageType::ServerHello as u16);
    ServerHello::decode(&payload).expect("valid ServerHello");
}

fn send_controller_attach(
    runtime: &mut Runtime,
    stream: &mut UnixStream,
    execution_id: ExecutionId,
) {
    send_frame(
        runtime,
        stream,
        MessageType::Attach,
        &Attach {
            execution_id,
            requested_role: Role::Controller,
        }
        .encode(),
    );
}

fn assert_no_published_attachment(runtime: &Runtime, execution_id: ExecutionId) {
    let summary = runtime
        .lookup(execution_id)
        .expect("execution remains live");
    assert_eq!(
        summary.attachment_count, 0,
        "failed attach published attachment authority"
    );
}

fn expect_projection_unavailable(runtime: &mut Runtime, stream: &mut UnixStream) {
    let (message_type, payload) =
        expect_plain_frame(runtime, stream, Instant::now() + Duration::from_secs(2));
    assert_eq!(message_type, MessageType::Error as u16);
    let error = ErrorMessage::decode(&payload).expect("valid Error response");
    assert_eq!(error.error_code, ErrorCode::ProjectionUnavailable as u16);
}

fn expect_disconnect(runtime: &mut Runtime, stream: &mut UnixStream) {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let mut byte = [0u8; 1];
        match stream.read(&mut byte) {
            Ok(0) => return,
            Ok(_) => panic!("descriptor-send failure unexpectedly delivered response bytes"),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("client read failed: {error}"),
        }
        assert!(
            Instant::now() < deadline,
            "connection did not close after send failure"
        );
        pump(runtime);
    }
}

#[test]
fn failed_first_attach_rolls_back_resources_and_controller_authority() {
    let mut runtime = Runtime::new(config()).expect("Runtime");
    let execution_id = runtime
        .create_execution(
            CommandSpec::new("/bin/cat"),
            WindowSize::new(80, 24, 0, 0).expect("valid size"),
        )
        .expect("execution");

    // Projection-resource creation failures are a recoverable inability to
    // produce the requested display projection, so SPEC-004's specific wire
    // error is ProjectionUnavailable. They must still publish no attachment or
    // controller authority and leave later projection creation possible.
    for point in [
        FaultPoint::ShmOpenWriter,
        FaultPoint::Truncate,
        FaultPoint::MmapWriter,
        FaultPoint::ShmOpenReader,
        FaultPoint::ShmUnlink,
    ] {
        let mut client = connect(&mut runtime);
        hello(&mut runtime, &mut client);
        test_fault::fail_next(point);
        send_controller_attach(&mut runtime, &mut client, execution_id);
        expect_projection_unavailable(&mut runtime, &mut client);
        assert_no_published_attachment(&runtime, execution_id);
        drop(client);
        pump(&mut runtime);
    }

    // Descriptor delivery failure occurs after the private projection exists
    // and has a committed first generation, but before Runtime publishes the
    // attachment/controller/projection authority. Transport failure closes the
    // connection rather than publishing an attachment whose descriptor was not
    // accepted by the bounded outbound path.
    let mut failed_send_client = connect(&mut runtime);
    hello(&mut runtime, &mut failed_send_client);
    test_fault::fail_next(FaultPoint::SendAttachedDescriptor);
    send_controller_attach(&mut runtime, &mut failed_send_client, execution_id);
    expect_disconnect(&mut runtime, &mut failed_send_client);
    assert_no_published_attachment(&runtime, execution_id);

    // A fresh controller must still be able to attach. This proves the failed
    // attempts did not strand a controller lease or attachment registry entry;
    // successful projection creation also proves failed lifecycle stages did
    // not leave the Runtime unable to allocate a replacement projection.
    let mut final_client = connect(&mut runtime);
    hello(&mut runtime, &mut final_client);
    send_controller_attach(&mut runtime, &mut final_client, execution_id);
    let (message_type, payload, fd) = expect_fd_frame(
        &mut runtime,
        &final_client,
        Instant::now() + Duration::from_secs(2),
    );
    assert_eq!(message_type, MessageType::Attached as u16);
    let attached = Attached::decode(&payload).expect("valid Attached");
    assert_eq!(attached.execution_id, execution_id);
    drop(fd);
    assert_eq!(runtime.lookup(execution_id).unwrap().attachment_count, 1);

    drop(final_client);
    for _ in 0..8 {
        pump(&mut runtime);
    }
    runtime.begin_shutdown().expect("begin shutdown");
    runtime
        .run_until_empty(Instant::now() + Duration::from_secs(4))
        .expect("controlled shutdown");
}
