#![cfg(target_os = "macos")]

use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use seyal_exec::{CommandSpec, WindowSize};
use seyal_protocol::pass8::{BLOCK_STATE_MESSAGE_TYPE, BlockLifecycle, BlockState, CAP_BLOCK_METADATA};
use seyal_runtime::{
    LocalIpcMode, Runtime, RuntimeConfig,
    local_ipc::framing::{
        Attach, ClientHello, FrameHeader, HEADER_LEN, MessageType, Role, encode_frame,
    },
};

fn config() -> RuntimeConfig {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let mut config = RuntimeConfig::m001().expect("M001 config");
    config.singleton_path = std::env::temp_dir().join(format!(
        "seyal-pass8-stall-{}-{nonce:x}.lock",
        std::process::id()
    ));
    config.local_ipc = LocalIpcMode::Enabled {
        runtime_dir_override: Some(std::env::temp_dir().join(format!(
            "s8st-{:x}-{nonce:x}",
            std::process::id()
        ))),
    };
    config.final_drain = Duration::from_millis(100);
    config
}

fn pump(runtime: &mut Runtime) {
    runtime
        .poll_once(Some(Duration::from_millis(5)))
        .expect("Runtime poll");
}

fn read_raw(runtime: &mut Runtime, stream: &mut UnixStream, buffered: &mut Vec<u8>) -> (u16, Vec<u8>) {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if buffered.len() >= HEADER_LEN {
            let header = FrameHeader::decode(&buffered[..HEADER_LEN]).expect("header");
            let total = HEADER_LEN + header.payload_len as usize;
            if buffered.len() >= total {
                let raw = buffered.drain(..total).collect::<Vec<_>>();
                return (header.message_type, raw[HEADER_LEN..].to_vec());
            }
        }
        let mut chunk = [0u8; 16 * 1024];
        match stream.read(&mut chunk) {
            Ok(0) => panic!("connection closed before Current BlockState"),
            Ok(count) => buffered.extend_from_slice(&chunk[..count]),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => pump(runtime),
            Err(error) => panic!("read failed: {error}"),
        }
        assert!(Instant::now() < deadline, "frame timed out");
    }
}

#[test]
fn stalled_block_capable_client_cannot_retain_completed_runtime_record() {
    let mut runtime = Runtime::new(config()).expect("Runtime");
    let execution_id = runtime
        .create_execution(
            CommandSpec::new("/bin/sh").args([
                "-c",
                "sleep 0.1; /usr/bin/yes X | /usr/bin/head -c 1048576; printf FINAL",
            ]),
            WindowSize::new(16, 4, 0, 0).expect("valid size"),
        )
        .expect("execution");

    let path = runtime
        .local_ipc_socket_path()
        .expect("socket")
        .to_path_buf();
    let mut stream = UnixStream::connect(path).expect("connect");
    stream.set_nonblocking(true).unwrap();
    pump(&mut runtime);

    stream
        .write_all(&encode_frame(
            MessageType::ClientHello,
            &ClientHello {
                client_capabilities: CAP_BLOCK_METADATA,
            }
            .encode(),
        ))
        .expect("hello");
    pump(&mut runtime);
    stream
        .write_all(&encode_frame(
            MessageType::Attach,
            &Attach {
                execution_id,
                requested_role: Role::Observer,
            }
            .encode(),
        ))
        .expect("attach");
    pump(&mut runtime);

    let mut buffered = Vec::new();
    loop {
        let (kind, payload) = read_raw(&mut runtime, &mut stream, &mut buffered);
        if kind == BLOCK_STATE_MESSAGE_TYPE {
            let current = BlockState::decode(&payload).expect("BlockState");
            assert_eq!(current.execution_id, execution_id);
            assert_eq!(current.state, BlockLifecycle::Current);
            break;
        }
    }

    // Deliberately stop reading from the attached socket. A blocked client may
    // lose completion/finalization delivery, but it must never retain Runtime
    // execution metadata or delay authoritative execution retirement.
    let deadline = Instant::now() + Duration::from_secs(10);
    while runtime.execution_count() != 0 && Instant::now() < deadline {
        pump(&mut runtime);
    }

    assert_eq!(runtime.execution_count(), 0);
    assert_eq!(runtime.block_count(), 0);
    assert_eq!(runtime.block(execution_id), None);
}
