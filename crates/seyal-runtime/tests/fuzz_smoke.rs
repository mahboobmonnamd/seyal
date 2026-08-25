#![cfg(target_os = "macos")]
#![allow(unsafe_code)]

use std::{
    env, fs,
    io::Write,
    os::fd::{AsRawFd, OwnedFd},
    os::unix::net::UnixStream,
    path::PathBuf,
    time::{Duration, Instant},
};

use seyal_exec::{CommandSpec, WindowSize};
use seyal_runtime::local_ipc::fd_transfer::{self, RecvFd};
use seyal_runtime::local_ipc::framing::{
    Attach, Attached, ClientHello, Detach, FrameHeader, HEADER_LEN, MessageType, Resync, Role,
    ServerHello, decode_message, encode_frame,
};
use seyal_runtime::projection::layout::{
    CELL_LEN, CellRecord, DAMAGE_LEN, DamageRecord, MAX_REGION_BYTES, REGION_HEADER_LEN,
    SLOT_HEADER_LEN, SlotHeader,
};
use seyal_runtime::projection::writer::{RegionMemory, read_latest, read_region_header};
use seyal_runtime::{AttachmentId, ExecutionId, LocalIpcMode, Runtime, RuntimeConfig};

fn input() -> Vec<u8> {
    let path =
        PathBuf::from(env::var_os("SEYAL_FUZZ_INPUT").expect("SEYAL_FUZZ_INPUT is required"));
    fs::read(path).expect("read retained fuzz seed")
}

#[test]
#[ignore = "executed by fuzz/targets/local-binary-protocol-decode with a retained seed"]
fn local_binary_protocol_decode_seed() {
    let bytes = input();
    if bytes.len() < HEADER_LEN {
        return;
    }
    let Ok(header) = FrameHeader::decode(&bytes[..HEADER_LEN]) else {
        return;
    };
    let payload_end = HEADER_LEN.saturating_add(header.payload_len as usize);
    let Some(payload) = bytes.get(HEADER_LEN..payload_end.min(bytes.len())) else {
        return;
    };
    if payload.len() != header.payload_len as usize {
        return;
    }
    let _ = decode_message(&header, payload);
}

#[test]
#[ignore = "executed by fuzz/targets/shared-projection-validation with a retained seed"]
fn shared_projection_validation_seed() {
    let bytes = input();
    let bounded_len = bytes.len().min(MAX_REGION_BYTES as usize);
    let storage_bytes = bounded_len.max(REGION_HEADER_LEN).div_ceil(8) * 8;
    let mut storage = vec![0u64; storage_bytes / 8].into_boxed_slice();
    for (index, chunk) in bytes[..bounded_len].chunks(8).enumerate() {
        let mut word = [0u8; 8];
        word[..chunk.len()].copy_from_slice(chunk);
        storage[index] = u64::from_le_bytes(word);
    }

    // SAFETY: boxed `u64` storage is 8-byte aligned, remains alive for the
    // entire reader exercise, and `storage_bytes` exactly describes its span.
    let memory = unsafe { RegionMemory::new(storage.as_mut_ptr().cast(), storage_bytes) };
    if let Ok(region) = read_region_header(&memory) {
        let _ = read_latest(&memory, &region);
    }

    // Retain direct fixed-record decoder coverage as a supplement to the
    // production mapped-reader path above.
    if bytes.len() >= SLOT_HEADER_LEN {
        let _ = SlotHeader::decode(&bytes[..SLOT_HEADER_LEN], 256, 512);
    }
    let (cell_chunks, _) = bytes.as_chunks::<CELL_LEN>();
    for chunk in cell_chunks {
        let _ = CellRecord::decode(chunk);
    }
    let (damage_chunks, _) = bytes.as_chunks::<DAMAGE_LEN>();
    for chunk in damage_chunks {
        let _ = DamageRecord::decode(chunk, 256);
    }
}

struct RuntimeFuzzHarness {
    runtime: Runtime,
    execution_id: ExecutionId,
}

impl RuntimeFuzzHarness {
    fn new() -> Self {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let suffix = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut config = RuntimeConfig::m001().expect("fuzz Runtime config");
        config.singleton_path = env::temp_dir().join(format!("fz-{suffix:x}.lock"));
        config.local_ipc = LocalIpcMode::Enabled {
            runtime_dir_override: Some(env::temp_dir().join(format!("fzd-{suffix:x}"))),
        };
        config.graceful_termination = Duration::from_millis(20);
        config.forced_reap = Duration::from_millis(100);
        config.final_drain = Duration::from_millis(20);
        let mut runtime = Runtime::new(config).expect("fuzz Runtime");
        let execution_id = runtime
            .create_execution(
                CommandSpec::new("/bin/cat"),
                WindowSize::new(8, 2, 0, 0).expect("fuzz geometry"),
            )
            .expect("fuzz execution");
        Self {
            runtime,
            execution_id,
        }
    }

    fn pump(&mut self) {
        let _ = self.runtime.poll_once(Some(Duration::from_millis(1)));
    }

    fn connect(&mut self) -> UnixStream {
        let path = self
            .runtime
            .local_ipc_socket_path()
            .expect("fuzz local IPC path")
            .to_path_buf();
        let mut stream = UnixStream::connect(path).expect("fuzz connect");
        stream.set_nonblocking(true).expect("fuzz nonblocking");
        self.send(
            &mut stream,
            MessageType::ClientHello,
            &ClientHello {
                client_capabilities: 0,
            }
            .encode(),
        );
        let (kind, payload, fd) = self
            .recv_frame(&stream, Duration::from_millis(250))
            .expect("fuzz ServerHello");
        drop(fd);
        assert_eq!(kind, MessageType::ServerHello as u16);
        ServerHello::decode(&payload).expect("fuzz valid ServerHello");
        stream
    }

    fn send(&mut self, stream: &mut UnixStream, kind: MessageType, payload: &[u8]) {
        let frame = encode_frame(kind, payload);
        let mut sent = 0usize;
        let deadline = Instant::now() + Duration::from_millis(250);
        while sent < frame.len() {
            match stream.write(&frame[sent..]) {
                Ok(0) => return,
                Ok(count) => sent += count,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => self.pump(),
                Err(_) => return,
            }
            if Instant::now() >= deadline {
                return;
            }
        }
        self.pump();
    }

    fn recv_frame(
        &mut self,
        stream: &UnixStream,
        timeout: Duration,
    ) -> Option<(u16, Vec<u8>, Option<OwnedFd>)> {
        let deadline = Instant::now() + timeout;
        let mut buffer = Vec::new();
        let mut descriptor = None;
        loop {
            if buffer.len() >= HEADER_LEN {
                let header = FrameHeader::decode(&buffer[..HEADER_LEN]).ok()?;
                let total = HEADER_LEN.checked_add(header.payload_len as usize)?;
                if buffer.len() >= total {
                    return Some((
                        header.message_type,
                        buffer[HEADER_LEN..total].to_vec(),
                        descriptor,
                    ));
                }
            }

            let mut chunk = [0u8; 4096];
            match fd_transfer::recv_with_fd(stream.as_raw_fd(), &mut chunk) {
                Ok((0, _)) => return None,
                Ok((count, RecvFd::One(fd))) => {
                    if descriptor.replace(fd).is_some() {
                        return None;
                    }
                    buffer.extend_from_slice(&chunk[..count]);
                }
                Ok((count, RecvFd::None)) => buffer.extend_from_slice(&chunk[..count]),
                Ok((_, RecvFd::Malformed)) => return None,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => self.pump(),
                Err(_) => return None,
            }
            if Instant::now() >= deadline {
                return None;
            }
        }
    }

    fn attach(&mut self, stream: &mut UnixStream, role: Role) -> Option<AttachmentId> {
        self.send(
            stream,
            MessageType::Attach,
            &Attach {
                execution_id: self.execution_id,
                requested_role: role,
            }
            .encode(),
        );
        let (kind, payload, fd) = self.recv_frame(stream, Duration::from_millis(250))?;
        if kind != MessageType::Attached as u16 {
            drop(fd);
            return None;
        }
        let attached = Attached::decode(&payload).ok()?;
        drop(fd);
        Some(attached.attachment_id)
    }

    fn drain_one(&mut self, stream: &UnixStream) {
        let _ = self.recv_frame(stream, Duration::from_millis(50));
    }

    fn shutdown(mut self) {
        let _ = self.runtime.begin_shutdown();
        let _ = self
            .runtime
            .run_until_empty(Instant::now() + Duration::from_secs(1));
    }
}

fn forged_attachment(seed: u8) -> AttachmentId {
    AttachmentId::from_bytes((0xfeed_0000u128 | seed as u128).to_le_bytes())
}

#[test]
#[ignore = "executed by fuzz/targets/reconnect-resync-state-machine with a retained seed"]
fn reconnect_resync_state_machine_seed() {
    let bytes = input();
    let mut harness = RuntimeFuzzHarness::new();
    let mut client = harness.connect();
    let mut attachment = None;

    // Cap state-machine work per seed. The retained/mutated bytes choose
    // operations and identities, but cannot create unbounded sockets/PTys or
    // queue work inside one fuzz invocation.
    for chunk in bytes.chunks(3).take(32) {
        if chunk.len() < 3 {
            break;
        }
        match chunk[0] % 6 {
            0 => {
                let role = if chunk[1] & 1 == 0 {
                    Role::Observer
                } else {
                    Role::Controller
                };
                if attachment.is_none() {
                    attachment = harness.attach(&mut client, role);
                }
            }
            1 => {
                let id = if chunk[1] & 1 == 0 {
                    attachment.unwrap_or_else(|| forged_attachment(chunk[2]))
                } else {
                    forged_attachment(chunk[2])
                };
                harness.send(
                    &mut client,
                    MessageType::Resync,
                    &Resync { attachment_id: id }.encode(),
                );
                harness.drain_one(&client);
            }
            2 => {
                let id = if chunk[1] & 1 == 0 {
                    attachment.unwrap_or_else(|| forged_attachment(chunk[2]))
                } else {
                    forged_attachment(chunk[2])
                };
                harness.send(
                    &mut client,
                    MessageType::Detach,
                    &Detach { attachment_id: id }.encode(),
                );
                harness.drain_one(&client);
                if attachment == Some(id) {
                    attachment = None;
                }
            }
            3 => {
                drop(client);
                for _ in 0..3 {
                    harness.pump();
                }
                client = harness.connect();
                attachment = None;
            }
            4 => {
                if let Some(id) = attachment {
                    let mut attacker = harness.connect();
                    harness.send(
                        &mut attacker,
                        MessageType::Resync,
                        &Resync { attachment_id: id }.encode(),
                    );
                    harness.drain_one(&attacker);
                }
            }
            _ => {
                // Repeated attach attempts exercise real one-attachment and
                // one-controller state transitions rather than a copied model.
                let role = if chunk[2] & 1 == 0 {
                    Role::Observer
                } else {
                    Role::Controller
                };
                let _ = harness.attach(&mut client, role);
            }
        }
    }

    drop(client);
    harness.shutdown();
}
