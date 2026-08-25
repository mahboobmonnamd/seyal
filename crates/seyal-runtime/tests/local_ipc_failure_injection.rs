#![cfg(all(target_os = "macos", feature = "test-fault-injection"))]

use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    time::{Duration, Instant},
};

use seyal_exec::{CommandSpec, WindowSize};
use seyal_runtime::{
    ExecutionId, LocalIpcMode, Runtime, RuntimeConfig,
    local_ipc::framing::{Attach, Attached, ClientHello, FrameHeader, HEADER_LEN, MessageType, Role, ServerHello, encode_frame},
    test_fault::{self, FaultPoint},
};

fn config() -> RuntimeConfig {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let suffix = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut config = RuntimeConfig::m001().unwrap();
    config.singleton_path = std::env::temp_dir().join(format!("s5fi-{suffix:x}.lock"));
    config.local_ipc = LocalIpcMode::Enabled { runtime_dir_override: Some(std::env::temp_dir().join(format!("s5fid-{suffix:x}"))) };
    config
}
fn pump(runtime: &mut Runtime) { runtime.poll_once(Some(Duration::from_millis(5))).unwrap(); }
fn connect(runtime: &mut Runtime) -> UnixStream { let stream = UnixStream::connect(runtime.local_ipc_socket_path().unwrap()).unwrap(); stream.set_nonblocking(true).unwrap(); pump(runtime); stream }
fn send(runtime: &mut Runtime, stream: &mut UnixStream, kind: MessageType, payload: &[u8]) { stream.write_all(&encode_frame(kind, payload)).unwrap(); pump(runtime); }
fn frame(runtime: &mut Runtime, stream: &mut UnixStream) -> Option<(MessageType, Vec<u8>)> {
    let deadline = Instant::now() + Duration::from_secs(2); let mut buffer = Vec::new();
    loop { let mut chunk = [0u8; 8192]; match stream.read(&mut chunk) { Ok(0) => return None, Ok(n) => buffer.extend_from_slice(&chunk[..n]), Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => pump(runtime), Err(_) => return None };
        if buffer.len() >= HEADER_LEN { let h = FrameHeader::decode(&buffer[..HEADER_LEN]).unwrap(); let total = HEADER_LEN + h.payload_len as usize; if buffer.len() >= total { return Some((MessageType::from_u16(h.message_type).unwrap(), buffer[HEADER_LEN..total].to_vec())); } }
        assert!(Instant::now() < deadline, "frame timed out");
    }
}
fn hello(runtime: &mut Runtime, stream: &mut UnixStream) { send(runtime, stream, MessageType::ClientHello, &ClientHello { client_capabilities: 0 }.encode()); let (kind, payload) = frame(runtime, stream).unwrap(); assert_eq!(kind, MessageType::ServerHello); ServerHello::decode(&payload).unwrap(); }
fn attach(runtime: &mut Runtime, stream: &mut UnixStream, execution_id: ExecutionId) { send(runtime, stream, MessageType::Attach, &Attach { execution_id, requested_role: Role::Controller }.encode()); }
fn no_authority(runtime: &Runtime, execution_id: ExecutionId) { assert_eq!(runtime.lookup(execution_id).unwrap().attachment_count, 0); }

#[test]
fn attach_admission_failure_publishes_no_authority_and_fresh_controller_recovers() {
    let mut runtime = Runtime::new(config()).unwrap();
    let execution_id = runtime.create_execution(CommandSpec::new("/bin/cat"), WindowSize::new(80, 24, 0, 0).unwrap()).unwrap();
    let mut failed = connect(&mut runtime); hello(&mut runtime, &mut failed);
    test_fault::fail_next(FaultPoint::AttachAdmission); attach(&mut runtime, &mut failed, execution_id);
    for _ in 0..4 { pump(&mut runtime); } no_authority(&runtime, execution_id);

    let mut fresh = connect(&mut runtime); hello(&mut runtime, &mut fresh); attach(&mut runtime, &mut fresh, execution_id);
    let (kind, payload) = frame(&mut runtime, &mut fresh).expect("Attached"); assert_eq!(kind, MessageType::Attached); assert_eq!(Attached::decode(&payload).unwrap().execution_id, execution_id);
    assert_eq!(runtime.lookup(execution_id).unwrap().attachment_count, 1);
    drop(fresh); drop(failed); for _ in 0..8 { pump(&mut runtime); } no_authority(&runtime, execution_id);
    runtime.begin_shutdown().unwrap(); runtime.run_until_empty(Instant::now() + Duration::from_secs(3)).unwrap();
}

#[test]
fn writable_flush_failure_reclaims_published_authority() {
    let mut runtime = Runtime::new(config()).unwrap();
    let execution_id = runtime.create_execution(CommandSpec::new("/bin/cat"), WindowSize::new(80, 24, 0, 0).unwrap()).unwrap();
    let mut client = connect(&mut runtime); hello(&mut runtime, &mut client);
    test_fault::fail_next(FaultPoint::AttachFlush); attach(&mut runtime, &mut client, execution_id);
    for _ in 0..16 { pump(&mut runtime); }
    no_authority(&runtime, execution_id);
}
