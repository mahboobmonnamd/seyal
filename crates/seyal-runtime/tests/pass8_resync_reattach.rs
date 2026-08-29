#![cfg(target_os = "macos")]

use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use seyal_exec::{CommandSpec, WindowSize};
use seyal_protocol::pass8::{BLOCK_STATE_MESSAGE_TYPE, BlockState, CAP_BLOCK_METADATA};
use seyal_runtime::{
    LocalIpcMode, Runtime, RuntimeConfig,
    local_ipc::framing::{
        Attach, Attached, ClientHello, Detach, Detached, FrameHeader, HEADER_LEN, MessageType,
        Resync, Role, encode_frame,
    },
};

fn config() -> RuntimeConfig {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let mut config = RuntimeConfig::m001().expect("M001 config");
    config.singleton_path = std::env::temp_dir().join(format!(
        "seyal-pass8-resync-{}-{nonce:x}.lock",
        std::process::id()
    ));
    config.local_ipc = LocalIpcMode::Enabled {
        runtime_dir_override: Some(std::env::temp_dir().join(format!(
            "s8rs-{:x}-{nonce:x}",
            std::process::id()
        ))),
    };
    config
}

fn pump(runtime: &mut Runtime) {
    runtime
        .poll_once(Some(Duration::from_millis(5)))
        .expect("Runtime poll");
}

fn send(runtime: &mut Runtime, stream: &mut UnixStream, kind: MessageType, payload: &[u8]) {
    stream
        .write_all(&encode_frame(kind, payload))
        .expect("send frame");
    pump(runtime);
}

fn raw_frame(
    runtime: &mut Runtime,
    stream: &mut UnixStream,
    buffered: &mut Vec<u8>,
) -> (u16, Vec<u8>) {
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
            Ok(0) => panic!("connection closed"),
            Ok(count) => buffered.extend_from_slice(&chunk[..count]),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => pump(runtime),
            Err(error) => panic!("read failed: {error}"),
        }
        assert!(Instant::now() < deadline, "frame timed out");
    }
}

fn read_until(
    runtime: &mut Runtime,
    stream: &mut UnixStream,
    buffered: &mut Vec<u8>,
    wanted: u16,
) -> Vec<u8> {
    loop {
        let (kind, payload) = raw_frame(runtime, stream, buffered);
        if kind == wanted {
            return payload;
        }
        assert!(
            kind == MessageType::DisplaySnapshot as u16
                || kind == MessageType::DisplayDelta as u16
                || kind == MessageType::ServerHello as u16
                || kind == MessageType::Attached as u16,
            "unexpected frame {kind} while waiting for {wanted}"
        );
    }
}

#[test]
fn resync_and_detach_reattach_preserve_execution_block_identity_and_anchor() {
    let mut runtime = Runtime::new(config()).expect("Runtime");
    let execution_id = runtime
        .create_execution(
            CommandSpec::new("/bin/sh").args([
                "-c",
                "printf '\033[?1049hTUI\033[?1049l'; i=0; while [ $i -lt 256 ]; do printf 'line-%s\\n' \"$i\"; i=$((i+1)); done; sleep 5",
            ]),
            WindowSize::new(16, 4, 0, 0).expect("size"),
        )
        .expect("execution");
    let original = runtime.block(execution_id).expect("Block admitted");

    let path = runtime
        .local_ipc_socket_path()
        .expect("socket")
        .to_path_buf();
    let mut stream = UnixStream::connect(path).expect("connect");
    stream.set_nonblocking(true).unwrap();
    pump(&mut runtime);
    let mut buffered = Vec::new();

    send(
        &mut runtime,
        &mut stream,
        MessageType::ClientHello,
        &ClientHello {
            client_capabilities: CAP_BLOCK_METADATA,
        }
        .encode(),
    );
    let _ = read_until(
        &mut runtime,
        &mut stream,
        &mut buffered,
        MessageType::ServerHello as u16,
    );

    send(
        &mut runtime,
        &mut stream,
        MessageType::Attach,
        &Attach {
            execution_id,
            requested_role: Role::Observer,
        }
        .encode(),
    );
    let attached_payload = read_until(
        &mut runtime,
        &mut stream,
        &mut buffered,
        MessageType::Attached as u16,
    );
    let attached = Attached::decode(&attached_payload).expect("Attached");
    let current_payload = read_until(
        &mut runtime,
        &mut stream,
        &mut buffered,
        BLOCK_STATE_MESSAGE_TYPE,
    );
    let current = BlockState::decode(&current_payload).expect("Current BlockState");
    assert_eq!(current.block_id, original.id);
    assert_eq!(current.start_line_id, original.start_line_id);

    send(
        &mut runtime,
        &mut stream,
        MessageType::Resync,
        &Resync {
            attachment_id: attached.attachment_id,
        }
        .encode(),
    );
    let _ = read_until(
        &mut runtime,
        &mut stream,
        &mut buffered,
        MessageType::DisplaySnapshot as u16,
    );
    let after_resync = runtime.block(execution_id).expect("Block after resync");
    assert_eq!(after_resync.id, original.id);
    assert_eq!(after_resync.start_line_id, original.start_line_id);

    send(
        &mut runtime,
        &mut stream,
        MessageType::Detach,
        &Detach {
            attachment_id: attached.attachment_id,
        }
        .encode(),
    );
    let detached_payload = read_until(
        &mut runtime,
        &mut stream,
        &mut buffered,
        MessageType::Detached as u16,
    );
    let detached = Detached::decode(&detached_payload).expect("Detached");
    assert_eq!(detached.attachment_id, attached.attachment_id);

    send(
        &mut runtime,
        &mut stream,
        MessageType::Attach,
        &Attach {
            execution_id,
            requested_role: Role::Observer,
        }
        .encode(),
    );
    let _ = read_until(
        &mut runtime,
        &mut stream,
        &mut buffered,
        MessageType::Attached as u16,
    );
    let reattached_payload = read_until(
        &mut runtime,
        &mut stream,
        &mut buffered,
        BLOCK_STATE_MESSAGE_TYPE,
    );
    let reattached = BlockState::decode(&reattached_payload).expect("reattached BlockState");
    assert_eq!(reattached.block_id, current.block_id);
    assert_eq!(reattached.start_line_id, current.start_line_id);
    assert_eq!(reattached.revision, current.revision);

    runtime.begin_shutdown().expect("shutdown");
    runtime
        .run_until_empty(Instant::now() + Duration::from_secs(10))
        .expect("drain");
    assert_eq!(runtime.block_count(), 0);
}
