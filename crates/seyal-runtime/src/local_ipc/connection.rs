//! SPEC-004 section 5.3/7 connection state machine, nonblocking socket
//! integration and bounded per-client queues (macOS).
//!
//! This module owns transport plumbing only: accepting connections,
//! reading/framing bytes, and writing queued frames (optionally with one
//! transferred descriptor). It has no opinion on what `Attach`/`Input`
//! mean; that semantic handling lives in the Runtime wiring layer that
//! consumes [`ServerEvent`]s from [`LocalIpcServer::poll`].

use std::{
    collections::{HashMap, VecDeque},
    io::{self, Read},
    os::fd::{AsRawFd, OwnedFd},
    os::unix::net::{UnixListener, UnixStream},
    path::Path,
    time::Duration,
};

use crate::local_ipc::{
    auth,
    fd_transfer,
    framing::{FrameHeader, HEADER_LEN, MAX_FRAME_PAYLOAD, MessageType},
    kq::{Kqueue, Readiness},
};

/// M001 hard maximum concurrent local control connections (SPEC-004
/// section 5.1).
pub const MAX_CONNECTIONS: usize = 16;
/// M001 hard maximum mandatory queued outbound control bytes per client
/// (SPEC-004 section 5.1/7).
pub const MAX_OUTBOUND_QUEUE_BYTES: usize = 262_144;

const LISTENER_TOKEN: u64 = 0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionState {
    AwaitHello,
    Ready,
    Attached,
    Closing,
}

impl ConnectionState {
    /// Validates whether `message_type` is a legal client-sent message in
    /// this state (SPEC-004 section 5.3). Server-to-client-only message
    /// types are always rejected here since a compliant client never sends
    /// them.
    pub fn validate_incoming(self, message_type: MessageType) -> Result<(), StateError> {
        use MessageType::*;
        let allowed = matches!(
            (self, message_type),
            (Self::AwaitHello, ClientHello)
                | (Self::Ready, ListExecutions | Attach | Goodbye)
                | (Self::Attached, Input | Resize | Resync | Detach | Goodbye)
        );
        if allowed {
            Ok(())
        } else {
            Err(StateError::InvalidState)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateError {
    InvalidState,
}

pub struct OutboundItem {
    bytes: Vec<u8>,
    sent: usize,
    fd: Option<OwnedFd>,
}

impl OutboundItem {
    pub fn new(bytes: Vec<u8>, fd: Option<OwnedFd>) -> Self {
        Self { bytes, sent: 0, fd }
    }
}

pub struct Connection {
    stream: UnixStream,
    pub state: ConnectionState,
    read_buf: Vec<u8>,
    outbox: VecDeque<OutboundItem>,
    outbox_bytes: usize,
    write_registered: bool,
}

#[derive(Debug)]
pub enum ServerEvent {
    Connected { token: u64 },
    /// A fully framed, protocol-version-valid frame. Message-type/semantic
    /// validation against `state` is the caller's responsibility (it may
    /// need to send a nonfatal `UnknownMessage`/`Error` reply rather than
    /// disconnect).
    Frame {
        token: u64,
        message_type: u16,
        payload: Vec<u8>,
    },
    /// A protocol-fatal framing error (SPEC-004 section 6.2/6.3): the
    /// connection is already closed by the time this is reported.
    FramingError { token: u64 },
    Disconnected { token: u64 },
    /// A peer whose UID did not match was rejected before any connection
    /// state was created (SPEC-004 section 4.3).
    PeerRejected,
}

pub struct LocalIpcServer {
    listener: UnixListener,
    kq: Kqueue,
    connections: HashMap<u64, Connection>,
    next_token: u64,
    max_connections: usize,
}

impl LocalIpcServer {
    pub fn bind(path: &Path, max_connections: usize) -> io::Result<Self> {
        let listener = UnixListener::bind(path)?;
        listener.set_nonblocking(true)?;
        let kq = Kqueue::create()?;
        kq.register_read(listener.as_raw_fd(), LISTENER_TOKEN)?;
        Ok(Self {
            listener,
            kq,
            connections: HashMap::new(),
            next_token: LISTENER_TOKEN + 1,
            max_connections,
        })
    }

    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }

    /// Waits for readiness, then services every ready descriptor exactly
    /// once, returning the resulting high-level events in occurrence order.
    pub fn poll(&mut self, timeout: Option<Duration>) -> io::Result<Vec<ServerEvent>> {
        let mut raw_events = [kq_placeholder(); 256];
        let count = self.kq.wait(timeout, &mut raw_events)?;
        let mut out = Vec::new();
        for event in &raw_events[..count] {
            if event.token == LISTENER_TOKEN {
                self.accept_ready_connections(&mut out)?;
                continue;
            }
            if event.readiness == Readiness::Readable {
                self.service_read(event.token, event.hangup, &mut out);
            }
            if event.readiness == Readiness::Writable {
                self.service_write(event.token, &mut out);
            }
        }
        Ok(out)
    }

    fn accept_ready_connections(&mut self, out: &mut Vec<ServerEvent>) -> io::Result<()> {
        loop {
            match self.listener.accept() {
                Ok((stream, _addr)) => {
                    if let Err(error) = auth::verify_same_user_peer(stream.as_raw_fd()) {
                        let _ = error;
                        out.push(ServerEvent::PeerRejected);
                        continue;
                    }
                    if self.connections.len() >= self.max_connections {
                        // Hard connection-capacity limit (SPEC-004 section
                        // 5.1): the socket is simply dropped without a
                        // connection being created.
                        continue;
                    }
                    stream.set_nonblocking(true)?;
                    let token = self.next_token;
                    self.next_token += 1;
                    self.kq.register_read(stream.as_raw_fd(), token)?;
                    self.connections.insert(
                        token,
                        Connection {
                            stream,
                            state: ConnectionState::AwaitHello,
                            read_buf: Vec::new(),
                            outbox: VecDeque::new(),
                            outbox_bytes: 0,
                            write_registered: false,
                        },
                    );
                    out.push(ServerEvent::Connected { token });
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(_) => break,
            }
        }
        Ok(())
    }

    fn service_read(&mut self, token: u64, hangup: bool, out: &mut Vec<ServerEvent>) {
        let Some(connection) = self.connections.get_mut(&token) else {
            return;
        };
        let mut chunk = [0u8; 64 * 1024];
        loop {
            match connection.stream.read(&mut chunk) {
                Ok(0) => {
                    self.close(token, out);
                    return;
                }
                Ok(count) => {
                    connection.read_buf.extend_from_slice(&chunk[..count]);
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(_) => {
                    self.close(token, out);
                    return;
                }
            }
        }
        if let Err(FramingCutError) = drain_frames(connection, token, out) {
            self.close(token, out);
            return;
        }
        // The only way execution reaches this point is via the `WouldBlock`
        // break above (the `Ok(0)`/`Err(_)` arms both return early), so the
        // socket's currently buffered bytes are fully drained/framed. If the
        // peer also half-closed its write side (`EV_EOF`) and nothing
        // remains buffered as an incomplete frame, there is nothing left to
        // read and this connection is done even though the last `read`
        // returned `WouldBlock` rather than `Ok(0)`.
        if hangup
            && self
                .connections
                .get(&token)
                .is_some_and(|connection| connection.read_buf.is_empty())
        {
            self.close(token, out);
        }
    }

    fn service_write(&mut self, token: u64, out: &mut Vec<ServerEvent>) {
        let close = {
            let Some(connection) = self.connections.get_mut(&token) else {
                return;
            };
            match flush_outbox(connection) {
                Ok(()) => false,
                Err(_) => true,
            }
        };
        if close {
            self.close(token, out);
            return;
        }
        if let Some(connection) = self.connections.get_mut(&token) {
            let idle = connection.outbox.is_empty();
            if idle && connection.write_registered {
                let _ = self.kq.deregister_write(connection.stream.as_raw_fd());
                connection.write_registered = false;
            }
        }
    }

    /// Queues `bytes` (a full header+payload frame) with an optional
    /// descriptor for exactly one transfer, then attempts an immediate
    /// flush. Enforces the bounded outbound queue (SPEC-004 section 7):
    /// exceeding it disconnects the slow client.
    pub fn enqueue(&mut self, token: u64, bytes: Vec<u8>, fd: Option<OwnedFd>) -> io::Result<()> {
        let Some(connection) = self.connections.get_mut(&token) else {
            return Ok(());
        };
        if connection.outbox_bytes + bytes.len() > MAX_OUTBOUND_QUEUE_BYTES {
            let mut dummy = Vec::new();
            self.close(token, &mut dummy);
            return Err(io::Error::other(
                "outbound queue capacity exceeded; connection closed",
            ));
        }
        connection.outbox_bytes += bytes.len();
        connection.outbox.push_back(OutboundItem::new(bytes, fd));
        if let Err(_error) = flush_outbox(connection) {
            let mut dummy = Vec::new();
            self.close(token, &mut dummy);
            return Ok(());
        }
        if !connection.outbox.is_empty() && !connection.write_registered {
            self.kq.register_write(connection.stream.as_raw_fd(), token)?;
            connection.write_registered = true;
        }
        Ok(())
    }

    pub fn close(&mut self, token: u64, out: &mut Vec<ServerEvent>) {
        if let Some(connection) = self.connections.remove(&token) {
            let _ = self.kq.deregister_all(connection.stream.as_raw_fd());
            out.push(ServerEvent::Disconnected { token });
        }
    }

    pub fn state_of(&self, token: u64) -> Option<ConnectionState> {
        self.connections.get(&token).map(|connection| connection.state)
    }

    pub fn set_state(&mut self, token: u64, state: ConnectionState) {
        if let Some(connection) = self.connections.get_mut(&token) {
            connection.state = state;
        }
    }
}

struct FramingCutError;

/// Parses as many complete frames as are available in `connection`'s read
/// buffer, emitting one [`ServerEvent::Frame`] per frame and leaving any
/// trailing partial frame buffered for the next read.
fn drain_frames(
    connection: &mut Connection,
    token: u64,
    out: &mut Vec<ServerEvent>,
) -> Result<(), FramingCutError> {
    loop {
        if connection.read_buf.len() < HEADER_LEN {
            return Ok(());
        }
        let header = match FrameHeader::decode(&connection.read_buf[..HEADER_LEN]) {
            Ok(header) => header,
            Err(error) => {
                out.push(ServerEvent::FramingError { token });
                let _ = error;
                return Err(FramingCutError);
            }
        };
        let total_len = HEADER_LEN + header.payload_len as usize;
        if header.payload_len > MAX_FRAME_PAYLOAD {
            out.push(ServerEvent::FramingError { token });
            return Err(FramingCutError);
        }
        if connection.read_buf.len() < total_len {
            return Ok(());
        }
        let payload = connection.read_buf[HEADER_LEN..total_len].to_vec();
        connection.read_buf.drain(0..total_len);
        out.push(ServerEvent::Frame {
            token,
            message_type: header.message_type,
            payload,
        });
    }
}

/// Sends as much of the queued outbound data as the socket currently
/// accepts. The descriptor on a frame that carries one (`Attached`,
/// `ProjectionReplaced`) is included in the very first `sendmsg` call for
/// that frame, so it is never duplicated or lost even if that call (or a
/// later continuation of the same item) only accepts part of the byte
/// range; the M001 frames that ever carry a descriptor are always well
/// under typical socket buffer capacity, so a split send is rare, but a
/// correct receiver must still be prepared to accumulate the remaining
/// plain bytes from a subsequent read rather than assuming one `recvmsg`
/// call always returns the whole frame.
fn flush_outbox(connection: &mut Connection) -> io::Result<()> {
    let fd = connection.stream.as_raw_fd();
    while let Some(item) = connection.outbox.front_mut() {
        let remaining = &item.bytes[item.sent..];
        let result = if item.fd.is_some() && item.sent == 0 {
            fd_transfer::send_with_fd(fd, remaining, item.fd.as_ref().map(AsRawFd::as_raw_fd))
        } else {
            fd_transfer::send_with_fd(fd, remaining, None)
        };
        match result {
            Ok(sent) => {
                if item.fd.is_some() {
                    // The descriptor (if any) has now been transferred
                    // regardless of whether every byte went out yet; never
                    // resend it on a later partial-completion call.
                    item.fd = None;
                }
                item.sent += sent;
                connection.outbox_bytes -= sent;
                if item.sent >= item.bytes.len() {
                    connection.outbox.pop_front();
                }
                if sent == 0 {
                    break;
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn kq_placeholder() -> crate::local_ipc::kq::Event {
    crate::local_ipc::kq::Event {
        token: 0,
        readiness: Readiness::Readable,
        hangup: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local_ipc::fd_transfer::RecvFd;
    use crate::local_ipc::framing::{Attach, Role, encode_frame};
    use std::io::Write;

    fn bind_test_server() -> (LocalIpcServer, std::path::PathBuf) {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let unique = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut path = std::env::temp_dir();
        path.push(format!(
            "syl-c{}-{}.sock",
            std::process::id() % 10_000,
            unique
        ));
        let _ = std::fs::remove_file(&path);
        let server = LocalIpcServer::bind(&path, MAX_CONNECTIONS).unwrap();
        (server, path)
    }

    #[test]
    fn accept_reports_connected_and_verifies_same_user_peer() {
        let (mut server, path) = bind_test_server();
        let _client = UnixStream::connect(&path).unwrap();
        let events = server.poll(Some(Duration::from_secs(1))).unwrap();
        assert!(matches!(events[0], ServerEvent::Connected { .. }));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn a_complete_frame_arriving_in_one_write_is_reported_once() {
        let (mut server, path) = bind_test_server();
        let mut client = UnixStream::connect(&path).unwrap();
        server.poll(Some(Duration::from_secs(1))).unwrap();

        let attach = Attach {
            execution_id: crate::ExecutionId::from_bytes(1u128.to_le_bytes()),
            requested_role: Role::Observer,
        };
        let frame = encode_frame(MessageType::Attach, &attach.encode());
        client.write_all(&frame).unwrap();

        let events = server.poll(Some(Duration::from_secs(1))).unwrap();
        let frames: Vec<_> = events
            .iter()
            .filter(|event| matches!(event, ServerEvent::Frame { .. }))
            .collect();
        assert_eq!(frames.len(), 1);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn a_frame_split_across_multiple_writes_is_reassembled() {
        let (mut server, path) = bind_test_server();
        let mut client = UnixStream::connect(&path).unwrap();
        server.poll(Some(Duration::from_secs(1))).unwrap();

        let attach = Attach {
            execution_id: crate::ExecutionId::from_bytes(2u128.to_le_bytes()),
            requested_role: Role::Controller,
        };
        let frame = encode_frame(MessageType::Attach, &attach.encode());
        client.write_all(&frame[..10]).unwrap();
        server.poll(Some(Duration::from_millis(50))).unwrap();
        client.write_all(&frame[10..]).unwrap();

        let events = server.poll(Some(Duration::from_secs(1))).unwrap();
        let frames: Vec<_> = events
            .into_iter()
            .filter(|event| matches!(event, ServerEvent::Frame { .. }))
            .collect();
        assert_eq!(frames.len(), 1);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn invalid_magic_is_reported_as_a_framing_error_and_closes_the_connection() {
        let (mut server, path) = bind_test_server();
        let mut client = UnixStream::connect(&path).unwrap();
        server.poll(Some(Duration::from_secs(1))).unwrap();

        let mut frame = encode_frame(MessageType::Goodbye, &[]);
        frame[0] = b'X';
        client.write_all(&frame).unwrap();

        let events = server.poll(Some(Duration::from_secs(1))).unwrap();
        assert!(events
            .iter()
            .any(|event| matches!(event, ServerEvent::FramingError { .. })));
        assert_eq!(server.connection_count(), 0);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn disconnect_mid_frame_is_reported_and_reclaims_the_connection() {
        let (mut server, path) = bind_test_server();
        let client = UnixStream::connect(&path).unwrap();
        server.poll(Some(Duration::from_secs(1))).unwrap();
        drop(client);

        let events = server.poll(Some(Duration::from_secs(1))).unwrap();
        assert!(events
            .iter()
            .any(|event| matches!(event, ServerEvent::Disconnected { .. })));
        assert_eq!(server.connection_count(), 0);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn enqueue_with_fd_delivers_bytes_and_the_descriptor_together() {
        let (mut server, path) = bind_test_server();
        let mut client = UnixStream::connect(&path).unwrap();
        let events = server.poll(Some(Duration::from_secs(1))).unwrap();
        let ServerEvent::Connected { token } = events[0] else {
            panic!("expected Connected");
        };

        let payload = b"hello".to_vec();
        let frame = encode_frame(MessageType::Attached, &payload);
        let file = std::fs::File::open("/dev/null").unwrap();
        server
            .enqueue(token, frame, Some(OwnedFd::from(file)))
            .unwrap();

        let mut buffer = [0u8; 128];
        let (received, fd) = fd_transfer::recv_with_fd(client.as_raw_fd(), &mut buffer).unwrap();
        assert!(received >= HEADER_LEN + payload.len());
        assert!(matches!(fd, RecvFd::One(_)));
        let _ = client.flush();
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn exceeding_the_outbound_queue_capacity_disconnects_the_slow_client() {
        let (mut server, path) = bind_test_server();
        let _client = UnixStream::connect(&path).unwrap();
        let events = server.poll(Some(Duration::from_secs(1))).unwrap();
        let ServerEvent::Connected { token } = events[0] else {
            panic!("expected Connected");
        };

        // Never read on the client side, so the server's socket send
        // buffer plus queue eventually saturates.
        let big_payload = vec![0u8; 4096];
        let mut disconnected = false;
        for _ in 0..200 {
            let frame = encode_frame(MessageType::Lifecycle, &big_payload);
            if server.enqueue(token, frame, None).is_err() {
                disconnected = true;
                break;
            }
        }
        assert!(disconnected, "expected the queue cap to disconnect the client");
        assert_eq!(server.connection_count(), 0);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn connection_state_rejects_input_before_attach() {
        assert_eq!(
            ConnectionState::Ready.validate_incoming(MessageType::Input),
            Err(StateError::InvalidState)
        );
        assert_eq!(
            ConnectionState::Attached.validate_incoming(MessageType::Input),
            Ok(())
        );
    }
}
