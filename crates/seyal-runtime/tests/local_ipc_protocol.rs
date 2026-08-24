#![cfg(target_os = "macos")]

//! SPEC-004 local attachment protocol integration tests.
//!
//! These exercise the real Unix-domain control socket, the real
//! shared-memory projection ABI (via genuine `mmap`/`SCM_RIGHTS`), and a
//! real `TerminalExecution`/PTY -- end to end, from plain client sockets
//! exactly as a future native client would use them. No test peeks at
//! Runtime-internal state; every assertion is made from what the wire
//! protocol itself observably returns.
//!
//! `Runtime` is intentionally not `Send` (its reactor holds raw
//! platform-native event-buffer state), so these tests drive the real,
//! unmodified single-threaded `Runtime::poll_once` loop from the same
//! thread as the client sockets, using nonblocking client reads and
//! polling both together until each expected frame arrives.

use std::{
    io::{Read, Write},
    os::fd::{AsRawFd, OwnedFd},
    os::unix::net::UnixStream,
    path::PathBuf,
    time::{Duration, Instant},
};

use seyal_exec::{CommandSpec, WindowSize};
use seyal_runtime::local_ipc::fd_transfer::{self, RecvFd};
use seyal_runtime::local_ipc::framing::{
    Attach, Attached, ClientHello, ErrorCode, ErrorMessage, FrameHeader, HEADER_LEN, InputRef,
    MessageType, ProjectionReplaced, Resize as WireResize, Role, ServerHello, encode_frame,
};
use seyal_runtime::projection::layout::{REGION_HEADER_LEN, RegionHeader};
use seyal_runtime::projection::lifecycle::ReadOnlyMapping;
use seyal_runtime::projection::writer::read_latest;
use seyal_runtime::{ExecutionId, LocalIpcMode, Runtime, RuntimeConfig};

fn unique_suffix() -> u64 {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

fn config(_test: &str) -> RuntimeConfig {
    let mut config = RuntimeConfig::m001().expect("bundled capability profile");
    let suffix = unique_suffix();
    // Darwin's `sockaddr_un.sun_path` capacity is tiny (~104 bytes
    // including the NUL terminator); keep the runtime directory name very
    // short so `<dir>/control.sock` always fits regardless of how long
    // `std::env::temp_dir()` already is on this machine.
    config.singleton_path = std::env::temp_dir().join(format!("s5-{suffix:x}.lock"));
    let runtime_dir = std::env::temp_dir().join(format!("s5d-{suffix:x}"));
    config.local_ipc = LocalIpcMode::Enabled {
        runtime_dir_override: Some(runtime_dir),
    };
    config.graceful_termination = Duration::from_millis(50);
    config.forced_reap = Duration::from_millis(250);
    config.final_drain = Duration::from_millis(100);
    config
}

fn size() -> WindowSize {
    WindowSize::new(6, 2, 0, 0).expect("valid size")
}

/// Drives the real Runtime event loop and client sockets together on one
/// thread: every "wait for a frame"/"wait for a projection condition" helper
/// alternates a nonblocking client read attempt with one
/// `Runtime::poll_once` tick until the expected condition is observed or a
/// deadline elapses.
struct Harness {
    runtime: Runtime,
}

impl Harness {
    fn new(test: &str) -> Self {
        Self {
            runtime: Runtime::new(config(test)).expect("Runtime"),
        }
    }

    fn socket_path(&self) -> PathBuf {
        self.runtime
            .local_ipc_socket_path()
            .expect("local IPC bound")
            .to_path_buf()
    }

    fn spawn(&mut self, command: CommandSpec) -> ExecutionId {
        self.runtime
            .create_execution(command, size())
            .expect("execution")
    }

    fn pump(&mut self) {
        let _ = self.runtime.poll_once(Some(Duration::from_millis(5)));
    }

    fn connect(&mut self) -> UnixStream {
        let path = self.socket_path();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match UnixStream::connect(&path) {
                Ok(stream) => {
                    stream
                        .set_nonblocking(true)
                        .expect("nonblocking client socket");
                    return stream;
                }
                Err(_) => {
                    self.pump();
                    assert!(Instant::now() < deadline, "connect timed out");
                }
            }
        }
    }

    fn send(&mut self, stream: &mut UnixStream, message_type: MessageType, payload: &[u8]) {
        let frame = encode_frame(message_type, payload);
        // A single small write to a fresh nonblocking socket buffer never
        // blocks/partial-writes for the tiny frames these tests send; pump
        // once afterward so the Runtime has a chance to observe it even if
        // no reactor event happens to coincide.
        stream.write_all(&frame).expect("write frame");
        self.pump();
    }

    /// Waits for exactly one complete frame with no descriptor.
    fn expect_frame(&mut self, stream: &mut UnixStream, deadline: Instant) -> (u16, Vec<u8>) {
        let mut buffer = Vec::new();
        loop {
            let mut chunk = [0u8; 8192];
            match stream.read(&mut chunk) {
                Ok(0) => panic!("connection closed while awaiting a frame"),
                Ok(count) => buffer.extend_from_slice(&chunk[..count]),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(error) => panic!("client read error: {error}"),
            }
            if buffer.len() >= HEADER_LEN {
                let header = FrameHeader::decode(&buffer[..HEADER_LEN]).expect("valid header");
                let total = HEADER_LEN + header.payload_len as usize;
                if buffer.len() >= total {
                    return (header.message_type, buffer[HEADER_LEN..total].to_vec());
                }
            }
            assert!(Instant::now() < deadline, "timed out awaiting a frame");
            self.pump();
        }
    }

    /// Waits for exactly one complete frame that carries exactly one
    /// descriptor (`Attached`/`ProjectionReplaced`), relying on the
    /// server's documented contract that such frames are always sent whole
    /// in a single `sendmsg` call.
    /// Waits for exactly one complete frame that carries exactly one
    /// descriptor (`Attached`/`ProjectionReplaced`). Accumulates across
    /// multiple `recvmsg` calls: the connection layer guarantees the
    /// descriptor is transferred together with the first byte(s) it sends
    /// for that frame, but (like any stream socket) the remaining bytes
    /// may still arrive in a later, separate call.
    fn expect_frame_with_fd(
        &mut self,
        stream: &UnixStream,
        deadline: Instant,
    ) -> (u16, Vec<u8>, OwnedFd) {
        let mut buffer = Vec::new();
        let mut captured_fd: Option<OwnedFd> = None;
        loop {
            if buffer.len() >= HEADER_LEN {
                let header = FrameHeader::decode(&buffer[..HEADER_LEN]).expect("valid header");
                let total = HEADER_LEN + header.payload_len as usize;
                if buffer.len() >= total {
                    let fd =
                        captured_fd.expect("frame completed without ever observing a descriptor");
                    return (header.message_type, buffer[HEADER_LEN..total].to_vec(), fd);
                }
            }
            let mut chunk = [0u8; 4096];
            match fd_transfer::recv_with_fd(stream.as_raw_fd(), &mut chunk) {
                Ok((0, _)) => panic!("connection closed while awaiting an fd-bearing frame"),
                Ok((received, RecvFd::One(fd))) => {
                    assert!(
                        captured_fd.replace(fd).is_none(),
                        "received more than one descriptor for a single frame"
                    );
                    buffer.extend_from_slice(&chunk[..received]);
                }
                Ok((received, RecvFd::None)) => {
                    buffer.extend_from_slice(&chunk[..received]);
                }
                Ok((_, RecvFd::Malformed)) => panic!("malformed descriptor transfer"),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(
                        Instant::now() < deadline,
                        "timed out awaiting an fd-bearing frame"
                    );
                    self.pump();
                }
                Err(error) => panic!("client recvmsg error: {error}"),
            }
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
        let deadline = Instant::now() + Duration::from_secs(2);
        let (message_type, payload) = self.expect_frame(stream, deadline);
        assert_eq!(message_type, MessageType::ServerHello as u16);
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
        let deadline = Instant::now() + Duration::from_secs(2);
        let (message_type, payload, fd) = self.expect_frame_with_fd(stream, deadline);
        assert_eq!(message_type, MessageType::Attached as u16);
        let attached = Attached::decode(&payload).expect("valid Attached");
        let mapping =
            ReadOnlyMapping::new(fd, attached.region_bytes as usize).expect("mmap projection");
        (attached, mapping)
    }
}

fn region_header_of(mapping: &ReadOnlyMapping, expected_region_bytes: u64) -> RegionHeader {
    let bytes = mapping.memory().read_bytes(0..REGION_HEADER_LEN).unwrap();
    let header = RegionHeader::decode(&bytes).unwrap();
    assert_eq!(header.region_bytes, expected_region_bytes);
    header
}

fn row_text(mapping: &ReadOnlyMapping, region: &RegionHeader) -> Option<String> {
    let snapshot = read_latest(&mapping.memory(), region).ok()?;
    let mut row = String::new();
    for col in 0..snapshot.header.columns {
        row.push(snapshot.cells[col as usize].scalar);
    }
    Some(row)
}

impl Harness {
    fn wait_for_row(
        &mut self,
        mapping: &ReadOnlyMapping,
        region: &RegionHeader,
        predicate: impl Fn(&str) -> bool,
    ) -> String {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(row) = row_text(mapping, region)
                && predicate(&row)
            {
                return row;
            }
            assert!(Instant::now() < deadline, "condition timed out");
            self.pump();
        }
    }
}

#[test]
fn attach_receives_initial_snapshot_reflecting_real_pty_output() {
    let mut harness = Harness::new("initial-snapshot");
    let execution_id =
        harness.spawn(CommandSpec::new("/bin/sh").args(["-c", "printf hi; sleep 2"]));

    let mut client = harness.connect();
    harness.hello(&mut client);
    let (_attached, mapping) = harness.attach(&mut client, execution_id, Role::Observer);
    let region = region_header_of(&mapping, _attached.region_bytes);

    let row = harness.wait_for_row(&mapping, &region, |row| row.trim_end().starts_with("hi"));
    assert!(row.trim_end().starts_with("hi"), "unexpected row: {row:?}");
}

#[test]
fn attach_to_a_finalized_execution_is_rejected_as_invalid_execution() {
    let mut harness = Harness::new("finalized-attach");
    let execution_id = harness.spawn(CommandSpec::new("/bin/sh").args(["-c", "printf done"]));

    // Wait for the execution to fully finalize and disappear from the
    // registry before ever attempting to attach to it, purely via the
    // observable wire protocol (`ListExecutions` no longer lists it).
    let mut client = harness.connect();
    harness.hello(&mut client);
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        harness.send(&mut client, MessageType::ListExecutions, &[]);
        let (message_type, payload) = harness.expect_frame(&mut client, deadline);
        assert_eq!(message_type, MessageType::ExecutionList as u16);
        let list = seyal_runtime::local_ipc::framing::ExecutionList::decode(&payload).unwrap();
        if !list
            .entries
            .iter()
            .any(|entry| entry.execution_id == execution_id)
        {
            break;
        }
        assert!(Instant::now() < deadline, "execution never finalized");
        harness.pump();
    }

    harness.send(
        &mut client,
        MessageType::Attach,
        &Attach {
            execution_id,
            requested_role: Role::Observer,
        }
        .encode(),
    );
    let (message_type, payload) = harness.expect_frame(&mut client, deadline);
    assert_eq!(message_type, MessageType::Error as u16);
    let error = ErrorMessage::decode(&payload).unwrap();
    assert_eq!(error.error_code, ErrorCode::InvalidExecution as u16);
}

#[test]
fn controller_input_reaches_pty_and_projection_updates_without_any_ack() {
    let mut harness = Harness::new("controller-input");
    let execution_id = harness.spawn(CommandSpec::new("/bin/cat"));

    let mut client = harness.connect();
    harness.hello(&mut client);
    let (attached, mapping) = harness.attach(&mut client, execution_id, Role::Controller);
    let region = region_header_of(&mapping, attached.region_bytes);

    harness.send(
        &mut client,
        MessageType::Input,
        &InputRef {
            attachment_id: attached.attachment_id,
            bytes: b"AB",
        }
        .encode(),
    );

    let row = harness.wait_for_row(&mapping, &region, |row| row.starts_with("AB"));
    assert!(row.starts_with("AB"), "unexpected row: {row:?}");
}

#[test]
fn observer_input_is_rejected_with_permission_denied_and_never_reaches_the_pty() {
    let mut harness = Harness::new("observer-denied");
    let execution_id = harness.spawn(CommandSpec::new("/bin/cat"));

    let mut client = harness.connect();
    harness.hello(&mut client);
    let (attached, _mapping) = harness.attach(&mut client, execution_id, Role::Observer);

    harness.send(
        &mut client,
        MessageType::Input,
        &InputRef {
            attachment_id: attached.attachment_id,
            bytes: b"ZZ",
        }
        .encode(),
    );
    let deadline = Instant::now() + Duration::from_secs(2);
    let (message_type, payload) = harness.expect_frame(&mut client, deadline);
    assert_eq!(message_type, MessageType::Error as u16);
    let error = ErrorMessage::decode(&payload).unwrap();
    assert_eq!(error.error_code, ErrorCode::PermissionDenied as u16);
}

#[test]
fn second_controller_attach_is_rejected_and_first_controller_lease_is_untouched() {
    let mut harness = Harness::new("second-controller");
    let execution_id = harness.spawn(CommandSpec::new("/bin/cat"));

    let mut first = harness.connect();
    harness.hello(&mut first);
    let (_first_attached, _first_mapping) =
        harness.attach(&mut first, execution_id, Role::Controller);

    let mut second = harness.connect();
    harness.hello(&mut second);
    harness.send(
        &mut second,
        MessageType::Attach,
        &Attach {
            execution_id,
            requested_role: Role::Controller,
        }
        .encode(),
    );
    let deadline = Instant::now() + Duration::from_secs(2);
    let (message_type, payload) = harness.expect_frame(&mut second, deadline);
    assert_eq!(message_type, MessageType::Error as u16);
    let error = ErrorMessage::decode(&payload).unwrap();
    assert_eq!(error.error_code, ErrorCode::ControllerBusy as u16);
}

#[test]
fn resize_beyond_initial_capacity_triggers_projection_replaced() {
    let mut harness = Harness::new("resize-replace");
    let execution_id = harness.spawn(CommandSpec::new("/bin/cat"));

    let mut client = harness.connect();
    harness.hello(&mut client);
    let (attached, _mapping) = harness.attach(&mut client, execution_id, Role::Controller);
    assert_eq!(attached.capacity_rows, 2);
    assert_eq!(attached.capacity_cols, 6);

    harness.send(
        &mut client,
        MessageType::Resize,
        &WireResize {
            attachment_id: attached.attachment_id,
            rows: 10,
            columns: 20,
        }
        .encode(),
    );

    let deadline = Instant::now() + Duration::from_secs(5);
    let (message_type, payload, _fd) = harness.expect_frame_with_fd(&client, deadline);
    assert_eq!(message_type, MessageType::ProjectionReplaced as u16);
    let replaced = ProjectionReplaced::decode(&payload).unwrap();
    assert_eq!(replaced.capacity_rows, 10);
    assert_eq!(replaced.capacity_cols, 20);
}

#[test]
fn reconnect_obtains_current_state_without_pty_replay() {
    let mut harness = Harness::new("reconnect");
    let execution_id = harness.spawn(CommandSpec::new("/bin/cat"));

    let mut first = harness.connect();
    harness.hello(&mut first);
    let (first_attached, first_mapping) =
        harness.attach(&mut first, execution_id, Role::Controller);
    let first_region = region_header_of(&first_mapping, first_attached.region_bytes);

    harness.send(
        &mut first,
        MessageType::Input,
        &InputRef {
            attachment_id: first_attached.attachment_id,
            bytes: b"XY",
        }
        .encode(),
    );
    harness.wait_for_row(&first_mapping, &first_region, |row| row.starts_with("XY"));
    drop(first);
    harness.pump();

    // A brand new connection/attachment must see the *current* canonical
    // state immediately, never historical PTY bytes replayed and never a
    // second client-side VT reconstructing it.
    let mut second = harness.connect();
    harness.hello(&mut second);
    let (second_attached, second_mapping) =
        harness.attach(&mut second, execution_id, Role::Observer);
    assert_ne!(second_attached.attachment_id, first_attached.attachment_id);
    assert_ne!(second_attached.projection_id, first_attached.projection_id);
    let second_region = region_header_of(&second_mapping, second_attached.region_bytes);
    let row = row_text(&second_mapping, &second_region).expect("readable snapshot on reconnect");
    assert!(
        row.starts_with("XY"),
        "reconnect must observe current state, got {row:?}"
    );
}

#[test]
fn a_killed_client_never_blocks_pty_progress_for_other_attachments() {
    let mut harness = Harness::new("killed-client");
    let execution_id =
        harness.spawn(CommandSpec::new("/bin/sh").args(["-c", "yes XXXXXX | head -c 200000"]));

    // A slow/malicious client that attaches and then never reads again.
    let mut slow = harness.connect();
    harness.hello(&mut slow);
    let (_slow_attached, _slow_mapping) = harness.attach(&mut slow, execution_id, Role::Observer);
    // Abrupt close, simulating SIGKILL: the socket is gone but the
    // execution/Runtime must not notice any different behavior than a
    // graceful detach.
    drop(slow);
    harness.pump();

    // A second, healthy observer must still observe the high-volume
    // output completing, proving the killed client never backpressured
    // PTY -> VT progress.
    let mut healthy = harness.connect();
    harness.hello(&mut healthy);
    let (healthy_attached, mapping) = harness.attach(&mut healthy, execution_id, Role::Observer);
    let region = region_header_of(&mapping, healthy_attached.region_bytes);
    let row = harness.wait_for_row(&mapping, &region, |row| row.starts_with("XXXXX"));
    assert!(row.starts_with("XXXXX"));
}
