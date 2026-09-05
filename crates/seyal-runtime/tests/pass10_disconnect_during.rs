#![cfg(target_os = "macos")]
#![allow(unsafe_code)]

//! Pass 10 §6.8 disconnect-during adversarial matrix cells.
//!
//! Covers abrupt client loss while Runtime is in:
//! - input backpressure
//! - outstanding correlated resize
//! - multi-chunk snapshot delivery
//! - Block finalization
//!
//! Also covers listener re-arm when capacity frees after accept backoff.

use std::{
    io::{Read, Write},
    os::fd::AsRawFd,
    os::unix::net::UnixStream,
    time::{Duration, Instant},
};

use seyal_exec::{CommandSpec, WindowSize};
use seyal_protocol::pass8::{
    BLOCK_STATE_MESSAGE_TYPE, BlockLifecycle, BlockState, CAP_BLOCK_METADATA,
};
use seyal_runtime::{
    ExecutionId, ExecutionLifecycle, LocalIpcMode, Runtime, RuntimeConfig,
    display::{decode_chunk, empty_cache},
    local_ipc::{
        connection::MAX_CONNECTIONS,
        framing::{
            Attach, Attached, ClientHello, ErrorCode, ErrorMessage, FrameHeader, HEADER_LEN,
            InputRef, MessageType, ResizeRequest, ResizeResult, ResizeResultCode, Role,
            ServerHello, encode_frame,
        },
    },
};

static TEST_SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
fn serialized() -> std::sync::MutexGuard<'static, ()> {
    TEST_SERIAL
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}

fn fd_count() -> usize {
    (0..1024)
        .filter(|fd| {
            // SAFETY: F_GETFD only inspects the integer descriptor; invalid
            // descriptors are reported with EBADF and are not modified.
            (unsafe { libc::fcntl(*fd, libc::F_GETFD) }) >= 0
        })
        .count()
}

fn config(tag: &str) -> RuntimeConfig {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let suffix = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut config = RuntimeConfig::m001().expect("M001 config");
    config.singleton_path = std::env::temp_dir().join(format!("s10d-{tag}-{suffix:x}.lock"));
    config.local_ipc = LocalIpcMode::Enabled {
        runtime_dir_override: Some(std::env::temp_dir().join(format!("s10dd-{tag}-{suffix:x}"))),
    };
    config.final_drain = Duration::from_millis(100);
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
    fn new(tag: &str) -> Self {
        Self {
            runtime: Runtime::new(config(tag)).expect("Runtime"),
        }
    }

    fn with_config(config: RuntimeConfig) -> Self {
        Self {
            runtime: Runtime::new(config).expect("Runtime"),
        }
    }

    fn pump(&mut self) {
        self.runtime
            .poll_once(Some(Duration::from_millis(5)))
            .expect("poll");
    }

    fn connect(&mut self) -> Client {
        let path = self
            .runtime
            .local_ipc_socket_path()
            .expect("socket")
            .to_path_buf();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match UnixStream::connect(&path) {
                Ok(stream) => {
                    stream.set_nonblocking(true).unwrap();
                    self.pump();
                    return Client {
                        stream,
                        buffered: Vec::new(),
                    };
                }
                Err(_) => {
                    assert!(Instant::now() < deadline, "connect timed out");
                    self.pump();
                }
            }
        }
    }

    fn send(&mut self, client: &mut Client, kind: MessageType, payload: &[u8]) {
        client
            .stream
            .write_all(&encode_frame(kind, payload))
            .expect("send");
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
            let mut chunk = [0u8; 16 * 1024];
            match client.stream.read(&mut chunk) {
                Ok(0) => panic!("connection closed"),
                Ok(n) => client.buffered.extend_from_slice(&chunk[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => self.pump(),
                Err(e) => panic!("read failed: {e}"),
            }
            assert!(Instant::now() < deadline, "frame timed out");
        }
    }

    fn raw_frame(&mut self, client: &mut Client) -> (u16, Vec<u8>) {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if client.buffered.len() >= HEADER_LEN {
                let header = FrameHeader::decode(&client.buffered[..HEADER_LEN]).unwrap();
                let total = HEADER_LEN + header.payload_len as usize;
                if client.buffered.len() >= total {
                    let raw = client.buffered.drain(..total).collect::<Vec<_>>();
                    return (header.message_type, raw[HEADER_LEN..].to_vec());
                }
            }
            let mut chunk = [0u8; 16 * 1024];
            match client.stream.read(&mut chunk) {
                Ok(0) => panic!("connection closed"),
                Ok(n) => client.buffered.extend_from_slice(&chunk[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => self.pump(),
                Err(e) => panic!("read failed: {e}"),
            }
            assert!(Instant::now() < deadline, "frame timed out");
        }
    }

    fn hello(&mut self, client: &mut Client, capabilities: u32) {
        self.send(
            client,
            MessageType::ClientHello,
            &ClientHello {
                client_capabilities: capabilities,
            }
            .encode(),
        );
        let (kind, payload) = self.frame(client);
        assert_eq!(kind, MessageType::ServerHello);
        let _ = ServerHello::decode(&payload).unwrap();
    }

    fn attach(&mut self, client: &mut Client, execution_id: ExecutionId, role: Role) -> Attached {
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
        Attached::decode(&payload).unwrap()
    }

    fn drain_display_batch(&mut self, client: &mut Client) {
        let (kind, payload) = self.frame(client);
        assert!(matches!(
            kind,
            MessageType::DisplaySnapshot | MessageType::DisplayDelta
        ));
        let first = decode_chunk(&encode_frame(kind, &payload)).unwrap();
        let expected = first.chunk_count as usize;
        let mut seen = 1usize;
        while seen < expected {
            let (next_kind, next_payload) = self.frame(client);
            assert_eq!(next_kind, kind);
            let _ = decode_chunk(&encode_frame(kind, &next_payload)).unwrap();
            seen += 1;
        }
    }

    fn wait_attachment_released(&mut self, execution_id: ExecutionId) {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            self.pump();
            if self
                .runtime
                .lookup(execution_id)
                .is_some_and(|summary| summary.attachment_count == 0)
            {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "attachment never released after disconnect"
            );
        }
    }

    fn shutdown(mut self) {
        self.runtime.begin_shutdown().unwrap();
        self.runtime
            .run_until_empty(Instant::now() + Duration::from_secs(3))
            .unwrap();
        assert_eq!(self.runtime.execution_count(), 0);
    }
}

#[test]
fn disconnect_during_input_backpressure_releases_authority_without_wedging_execution() {
    let _guard = serialized();
    let baseline_fds = fd_count();
    {
        let mut config = config("bp");
        config.per_execution_input_bytes = 64;
        config.aggregate_input_bytes = 64;
        let mut harness = Harness::with_config(config);
        // Child that does not drain stdin so accepted input stays pending.
        let execution_id = harness
            .runtime
            .create_execution(
                CommandSpec::new("/bin/sh").args(["-c", "sleep 30"]),
                WindowSize::new(80, 24, 0, 0).unwrap(),
            )
            .unwrap();

        let mut client = harness.connect();
        harness.hello(&mut client, 0);
        let attached = harness.attach(&mut client, execution_id, Role::Controller);
        harness.drain_display_batch(&mut client);

        // Fill the accepted-but-unwritten budget.
        harness.send(
            &mut client,
            MessageType::Input,
            &InputRef {
                attachment_id: attached.attachment_id,
                bytes: &[b'x'; 64],
            }
            .encode(),
        );
        assert!(
            harness.runtime.aggregate_accepted_but_unwritten_bytes() > 0,
            "pending input must occupy the budget before backpressure"
        );

        // Next admission must be explicit Backpressure while pending work remains.
        harness.send(
            &mut client,
            MessageType::Input,
            &InputRef {
                attachment_id: attached.attachment_id,
                bytes: b"y",
            }
            .encode(),
        );
        let (kind, payload) = harness.frame(&mut client);
        assert_eq!(kind, MessageType::Error);
        let error = ErrorMessage::decode(&payload).unwrap();
        assert_eq!(error.error_code, ErrorCode::Backpressure as u16);
        assert_eq!(error.offending_message_type, MessageType::Input as u16);

        // Disconnect while still backpressured with outstanding pending input.
        drop(client);
        harness.wait_attachment_released(execution_id);
        assert_eq!(
            harness.runtime.lookup(execution_id).unwrap().lifecycle,
            ExecutionLifecycle::Running
        );

        let mut successor = harness.connect();
        harness.hello(&mut successor, 0);
        let _ = harness.attach(&mut successor, execution_id, Role::Controller);
        assert_eq!(
            harness
                .runtime
                .lookup(execution_id)
                .unwrap()
                .attachment_count,
            1
        );

        drop(successor);
        harness.wait_attachment_released(execution_id);
        harness.shutdown();
    }
    assert_eq!(
        fd_count(),
        baseline_fds,
        "disconnect during input backpressure leaked descriptors"
    );
}

#[test]
fn disconnect_during_outstanding_resize_keeps_applied_geometry_and_frees_lease() {
    let _guard = serialized();
    let baseline_fds = fd_count();
    {
        let mut harness = Harness::new("resize");
        let execution_id = harness
            .runtime
            .create_execution(
                CommandSpec::new("/bin/cat"),
                WindowSize::new(80, 24, 0, 0).unwrap(),
            )
            .unwrap();

        let mut client = harness.connect();
        harness.hello(&mut client, 0);
        let attached = harness.attach(&mut client, execution_id, Role::Controller);
        harness.drain_display_batch(&mut client);

        // Admit the resize, leave ResizeResult / projection unread, then vanish.
        harness.send(
            &mut client,
            MessageType::ResizeRequest,
            &ResizeRequest {
                attachment_id: attached.attachment_id,
                request_id: 1,
                rows: 40,
                columns: 120,
            }
            .encode(),
        );
        assert_eq!(
            harness
                .runtime
                .execution(execution_id)
                .unwrap()
                .terminal()
                .rows(),
            40
        );
        assert_eq!(
            harness
                .runtime
                .execution(execution_id)
                .unwrap()
                .terminal()
                .cols(),
            120
        );

        drop(client);
        harness.wait_attachment_released(execution_id);
        assert_eq!(
            harness
                .runtime
                .execution(execution_id)
                .unwrap()
                .terminal()
                .rows(),
            40
        );
        assert_eq!(
            harness.runtime.lookup(execution_id).unwrap().lifecycle,
            ExecutionLifecycle::Running
        );

        let mut successor = harness.connect();
        harness.hello(&mut successor, 0);
        let fresh = harness.attach(&mut successor, execution_id, Role::Controller);
        harness.drain_display_batch(&mut successor);
        // A new connection starts a fresh request-id sequence; first request
        // after reconnect must not be poisoned by the abandoned outstanding one.
        harness.send(
            &mut successor,
            MessageType::ResizeRequest,
            &ResizeRequest {
                attachment_id: fresh.attachment_id,
                request_id: 1,
                rows: 30,
                columns: 100,
            }
            .encode(),
        );
        let (kind, payload) = harness.frame(&mut successor);
        assert_eq!(kind, MessageType::ResizeResult);
        let result = ResizeResult::decode(&payload).unwrap();
        assert_eq!(result.result_code, ResizeResultCode::Applied);
        assert_eq!(
            harness
                .runtime
                .execution(execution_id)
                .unwrap()
                .terminal()
                .rows(),
            30
        );

        drop(successor);
        harness.wait_attachment_released(execution_id);
        harness.shutdown();
    }
    assert_eq!(
        fd_count(),
        baseline_fds,
        "disconnect during outstanding resize leaked descriptors"
    );
}

#[test]
fn disconnect_during_snapshot_chunking_cleans_up_and_allows_full_reattach() {
    let _guard = serialized();
    let baseline_fds = fd_count();
    {
        let mut harness = Harness::new("chunk");
        // Large geometry forces multi-chunk DisplaySnapshot under MAX_FRAME_PAYLOAD.
        let execution_id = harness
            .runtime
            .create_execution(
                CommandSpec::new("/bin/cat"),
                WindowSize::new(200, 120, 0, 0).unwrap(),
            )
            .unwrap();

        let mut client = harness.connect();
        harness.hello(&mut client, 0);
        let _attached = harness.attach(&mut client, execution_id, Role::Controller);

        let (kind, payload) = harness.frame(&mut client);
        assert_eq!(kind, MessageType::DisplaySnapshot);
        let first = decode_chunk(&encode_frame(kind, &payload)).unwrap();
        assert!(
            first.chunk_count > 1,
            "fixture must deliver a multi-chunk snapshot; got {}",
            first.chunk_count
        );

        // Read nothing further — disconnect mid-batch while remaining chunks
        // are still queued / in flight on the connection.
        drop(client);
        harness.wait_attachment_released(execution_id);
        assert_eq!(
            harness.runtime.lookup(execution_id).unwrap().lifecycle,
            ExecutionLifecycle::Running
        );

        let mut successor = harness.connect();
        harness.hello(&mut successor, 0);
        let _ = harness.attach(&mut successor, execution_id, Role::Controller);
        let mut cache = empty_cache();
        let (kind, payload) = harness.frame(&mut successor);
        assert_eq!(kind, MessageType::DisplaySnapshot);
        let first = decode_chunk(&encode_frame(kind, &payload)).unwrap();
        let expected = first.chunk_count as usize;
        let mut chunks = vec![first];
        while chunks.len() < expected {
            let (next_kind, next_payload) = harness.frame(&mut successor);
            assert_eq!(next_kind, kind);
            chunks.push(decode_chunk(&encode_frame(kind, &next_payload)).unwrap());
        }
        cache
            .apply_chunks(&chunks)
            .expect("complete snapshot after reattach");
        assert_eq!((cache.rows, cache.columns), (120, 200));

        drop(successor);
        harness.wait_attachment_released(execution_id);
        harness.shutdown();
    }
    assert_eq!(
        fd_count(),
        baseline_fds,
        "disconnect during snapshot chunking leaked descriptors"
    );
}

#[test]
fn disconnect_during_block_finalization_retires_execution_without_leak() {
    let _guard = serialized();
    let baseline_fds = fd_count();
    {
        let mut harness = Harness::new("final");
        let execution_id = harness
            .runtime
            .create_execution(
                CommandSpec::new("/bin/sh").args([
                    "-c",
                    "sleep 0.15; /usr/bin/yes X | /usr/bin/head -c 65536; printf FINAL",
                ]),
                WindowSize::new(16, 4, 0, 0).unwrap(),
            )
            .unwrap();

        let mut client = harness.connect();
        harness.hello(&mut client, CAP_BLOCK_METADATA);
        harness.send(
            &mut client,
            MessageType::Attach,
            &Attach {
                execution_id,
                requested_role: Role::Observer,
            }
            .encode(),
        );

        // Reach Current BlockState, then stop reading so finalization frames
        // remain unread on the live connection.
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let (kind, payload) = harness.raw_frame(&mut client);
            if kind == BLOCK_STATE_MESSAGE_TYPE {
                let current = BlockState::decode(&payload).expect("BlockState");
                assert_eq!(current.execution_id, execution_id);
                assert_eq!(current.state, BlockLifecycle::Current);
                break;
            }
            assert!(Instant::now() < deadline, "Current BlockState timed out");
        }

        // SPEC-007 §10 retires Block metadata in the same bounded turn that
        // admits finalization frames, so Block cannot remain observable after
        // finalization completes. The adversarial cell is therefore: Runtime
        // finishes finalization while a stalled client still holds unread
        // finalization bytes, then the peer vanishes before consuming them.
        let retire_deadline = Instant::now() + Duration::from_secs(10);
        while harness.runtime.execution_count() != 0 && Instant::now() < retire_deadline {
            harness.pump();
        }
        assert_eq!(
            harness.runtime.execution_count(),
            0,
            "stalled client must not retain the execution across finalization"
        );
        assert_eq!(
            harness.runtime.block_count(),
            0,
            "stalled client must not retain Block metadata across finalization"
        );
        assert_eq!(harness.runtime.block(execution_id), None);

        // Prove finalization delivery is still owed before disconnect.
        let mut peek = [0u8; 1];
        // SAFETY: MSG_PEEK only inspects the socket receive buffer.
        let peeked = unsafe {
            libc::recv(
                client.stream.as_raw_fd(),
                peek.as_mut_ptr().cast(),
                peek.len(),
                libc::MSG_PEEK | libc::MSG_DONTWAIT,
            )
        };
        assert!(
            peeked > 0,
            "expected unread finalization bytes on the stalled connection before disconnect"
        );

        drop(client);

        let cleanup_deadline = Instant::now() + Duration::from_secs(5);
        while (harness.runtime.execution_count() != 0 || harness.runtime.block_count() != 0)
            && Instant::now() < cleanup_deadline
        {
            harness.pump();
        }
        assert_eq!(harness.runtime.execution_count(), 0);
        assert_eq!(harness.runtime.block_count(), 0);
    }
    assert_eq!(
        fd_count(),
        baseline_fds,
        "disconnect during Block finalization leaked descriptors"
    );
}

#[test]
fn listener_backoff_rearms_when_disconnect_frees_capacity() {
    let _guard = serialized();
    let baseline_fds = fd_count();
    {
        let mut harness = Harness::new("rearm");
        let mut held: Vec<Client> = Vec::new();
        for _ in 0..MAX_CONNECTIONS {
            let mut client = harness.connect();
            harness.hello(&mut client, 0);
            held.push(client);
        }

        // Escalate accept backoff via repeated capacity-full empty accept turns.
        // Each overflow connect is accepted then dropped without registration,
        // yielding an empty event list and exponential listener disarm.
        let path = harness
            .runtime
            .local_ipc_socket_path()
            .unwrap()
            .to_path_buf();
        for _ in 0..5 {
            let mut overflow = UnixStream::connect(&path).expect("overflow connect");
            overflow.set_nonblocking(true).unwrap();
            let close_deadline = Instant::now() + Duration::from_secs(2);
            loop {
                let mut probe = [0u8; 8];
                match overflow.read(&mut probe) {
                    Ok(0) => break,
                    Ok(_) => panic!("capacity reject must not service the peer"),
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        // Poll long enough to fire listener backoff deadlines
                        // so the next empty turn can escalate the delay.
                        harness
                            .runtime
                            .poll_once(Some(Duration::from_millis(50)))
                            .unwrap();
                    }
                    Err(e) => panic!("overflow read: {e}"),
                }
                assert!(
                    Instant::now() < close_deadline,
                    "capacity overflow peer was never closed"
                );
            }
            drop(overflow);
            // Allow the escalated backoff deadline to arm before the next turn.
            harness
                .runtime
                .poll_once(Some(Duration::from_millis(5)))
                .unwrap();
        }

        // Free one slot while the listener is still (or about to be) disarmed.
        held.pop();
        harness.pump();

        let started = Instant::now();
        let mut replacement = harness.connect();
        harness.hello(&mut replacement, 0);
        let admitted = started.elapsed();
        // Without re-arm on capacity free, admission can wait for the escalated
        // accept backoff (up to ACCEPT_BACKOFF_MAX = 250ms). With re-arm, a
        // short poll cycle is enough.
        assert!(
            admitted < Duration::from_millis(80),
            "listener did not re-arm promptly after capacity freed: {admitted:?}"
        );

        drop(replacement);
        held.clear();
        for _ in 0..8 {
            harness.pump();
        }
        harness.runtime.begin_shutdown().unwrap();
        harness
            .runtime
            .run_until_empty(Instant::now() + Duration::from_secs(3))
            .unwrap();
    }
    assert_eq!(
        fd_count(),
        baseline_fds,
        "listener re-arm path leaked descriptors"
    );
}
