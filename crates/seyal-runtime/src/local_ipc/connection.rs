//! SPEC-004 local Unix-domain control transport (macOS).
//!
//! This layer owns nonblocking listener/client sockets and bounded framing
//! buffers only. Readiness is supplied by `ExecutionReactor`, so Pass 5 uses
//! the same Runtime kqueue as PTY/process/control events rather than a second
//! polled event loop.

use std::{
    collections::{HashMap, VecDeque},
    io::{self, Read},
    os::fd::{AsRawFd, OwnedFd, RawFd},
    os::unix::net::{UnixListener, UnixStream},
    path::Path,
};

use crate::local_ipc::{
    auth, fd_transfer,
    framing::{FrameHeader, HEADER_LEN, MAX_FRAME_PAYLOAD, MessageType},
};

pub const MAX_CONNECTIONS: usize = 16;
pub const MAX_OUTBOUND_QUEUE_BYTES: usize = 262_144;
const MAX_RECEIVE_BUFFER_BYTES: usize = HEADER_LEN + MAX_FRAME_PAYLOAD as usize;
const READ_CHUNK_BYTES: usize = HEADER_LEN * 32;
const MAX_FRAMES_PER_READINESS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionState {
    AwaitHello,
    Ready,
    Attached,
    Closing,
}

impl ConnectionState {
    pub fn validate_incoming(self, message_type: MessageType) -> Result<(), StateError> {
        use MessageType::*;
        let allowed = matches!(
            (self, message_type),
            (Self::AwaitHello, ClientHello)
                | (Self::Ready, ListExecutions | Attach | Goodbye)
                | (Self::Attached, Input | Resize | Resync | Detach | Goodbye)
        );
        allowed.then_some(()).ok_or(StateError::InvalidState)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateError {
    InvalidState,
}

struct OutboundItem {
    bytes: Vec<u8>,
    sent: usize,
    fd: Option<OwnedFd>,
}

impl OutboundItem {
    fn new(bytes: Vec<u8>, fd: Option<OwnedFd>) -> Self {
        Self { bytes, sent: 0, fd }
    }

    fn remaining_len(&self) -> usize {
        self.bytes.len().saturating_sub(self.sent)
    }
}

struct Connection {
    stream: UnixStream,
    state: ConnectionState,
    read_buf: Vec<u8>,
    mandatory: VecDeque<OutboundItem>,
    mandatory_bytes: usize,
    wake_inflight: Option<OutboundItem>,
    pending_wake: Option<Vec<u8>>,
}

#[derive(Debug)]
pub enum ServerEvent {
    Connected { token: u64 },
    Frame {
        token: u64,
        message_type: u16,
        payload: Vec<u8>,
    },
    FramingError { token: u64 },
    Disconnected { token: u64 },
    PeerRejected,
}

pub struct LocalIpcServer {
    listener: UnixListener,
    connections: HashMap<u64, Connection>,
    next_token: u64,
    max_connections: usize,
}

impl LocalIpcServer {
    pub fn bind(path: &Path, max_connections: usize) -> io::Result<Self> {
        let listener = UnixListener::bind(path)?;
        listener.set_nonblocking(true)?;
        // Socket path permissions are an authorization boundary in addition
        // to kernel peer-UID verification.
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        Ok(Self {
            listener,
            connections: HashMap::new(),
            next_token: 1,
            max_connections,
        })
    }

    pub fn listener_fd(&self) -> RawFd {
        self.listener.as_raw_fd()
    }

    pub fn connection_fd(&self, token: u64) -> Option<RawFd> {
        self.connections
            .get(&token)
            .map(|connection| connection.stream.as_raw_fd())
    }

    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }

    pub fn contains(&self, token: u64) -> bool {
        self.connections.contains_key(&token)
    }

    /// Accepts every currently pending client. Same-UID authentication is
    /// performed before a connection record is published.
    pub fn accept_ready(&mut self) -> io::Result<Vec<ServerEvent>> {
        let mut events = Vec::new();
        loop {
            match self.listener.accept() {
                Ok((stream, _)) => {
                    if auth::verify_same_user_peer(stream.as_raw_fd()).is_err() {
                        events.push(ServerEvent::PeerRejected);
                        continue;
                    }
                    if self.connections.len() >= self.max_connections {
                        continue;
                    }
                    stream.set_nonblocking(true)?;
                    let token = self.next_token;
                    self.next_token = self.next_token.wrapping_add(1).max(1);
                    self.connections.insert(
                        token,
                        Connection {
                            stream,
                            state: ConnectionState::AwaitHello,
                            read_buf: Vec::with_capacity(4096),
                            mandatory: VecDeque::new(),
                            mandatory_bytes: 0,
                            wake_inflight: None,
                            pending_wake: None,
                        },
                    );
                    events.push(ServerEvent::Connected { token });
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) => return Err(error),
            }
        }
        Ok(events)
    }

    /// Services one client read-readiness notification with bounded work.
    pub fn service_read(&mut self, token: u64, hangup: bool) -> Vec<ServerEvent> {
        let mut events = Vec::new();
        let Some(connection) = self.connections.get_mut(&token) else {
            return events;
        };
        let mut chunk = [0u8; READ_CHUNK_BYTES];

        while events.len() < MAX_FRAMES_PER_READINESS {
            match connection.stream.read(&mut chunk) {
                Ok(0) => {
                    self.close_with_event(token, &mut events);
                    return events;
                }
                Ok(count) => {
                    let Some(new_len) = connection.read_buf.len().checked_add(count) else {
                        events.push(ServerEvent::FramingError { token });
                        self.close_with_event(token, &mut events);
                        return events;
                    };
                    if new_len > MAX_RECEIVE_BUFFER_BYTES {
                        events.push(ServerEvent::FramingError { token });
                        self.close_with_event(token, &mut events);
                        return events;
                    }
                    connection.read_buf.extend_from_slice(&chunk[..count]);
                    if drain_frames(connection, token, &mut events).is_err() {
                        self.close_with_event(token, &mut events);
                        return events;
                    }
                    if events.len() >= MAX_FRAMES_PER_READINESS {
                        break;
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(_) => {
                    self.close_with_event(token, &mut events);
                    return events;
                }
            }
        }

        if hangup {
            let partial = self
                .connections
                .get(&token)
                .is_some_and(|connection| !connection.read_buf.is_empty());
            if partial {
                events.push(ServerEvent::FramingError { token });
            }
            self.close_with_event(token, &mut events);
        }
        events
    }

    pub fn service_write(&mut self, token: u64) -> Vec<ServerEvent> {
        let mut events = Vec::new();
        let Some(connection) = self.connections.get_mut(&token) else {
            return events;
        };
        if flush_outbound(connection).is_err() {
            self.close_with_event(token, &mut events);
        }
        events
    }

    /// Queues a non-coalescible control response. Queue overflow closes the
    /// slow client; terminal execution remains independent.
    pub fn enqueue_mandatory(
        &mut self,
        token: u64,
        bytes: Vec<u8>,
        fd: Option<OwnedFd>,
    ) -> io::Result<()> {
        let Some(connection) = self.connections.get_mut(&token) else {
            return Err(io::Error::new(io::ErrorKind::NotConnected, "connection is closed"));
        };
        let new_total = connection
            .mandatory_bytes
            .checked_add(bytes.len())
            .ok_or_else(|| io::Error::other("mandatory queue length overflow"))?;
        if new_total > MAX_OUTBOUND_QUEUE_BYTES {
            self.connections.remove(&token);
            return Err(io::Error::other("mandatory outbound queue capacity exceeded"));
        }
        connection.mandatory_bytes = new_total;
        connection.mandatory.push_back(OutboundItem::new(bytes, fd));
        if let Err(error) = flush_outbound(connection) {
            self.connections.remove(&token);
            return Err(error);
        }
        Ok(())
    }

    /// Keeps at most one not-yet-started advisory wake. If a wake frame is
    /// already partially in flight, one newer pending frame may replace any
    /// older pending frame; no generation history is accumulated.
    pub fn enqueue_wake(&mut self, token: u64, bytes: Vec<u8>) -> io::Result<()> {
        let Some(connection) = self.connections.get_mut(&token) else {
            return Err(io::Error::new(io::ErrorKind::NotConnected, "connection is closed"));
        };
        if connection.wake_inflight.is_none() {
            connection.pending_wake = Some(bytes);
        } else {
            connection.pending_wake = Some(bytes);
        }
        if let Err(error) = flush_outbound(connection) {
            self.connections.remove(&token);
            return Err(error);
        }
        Ok(())
    }

    pub fn wants_write(&self, token: u64) -> bool {
        self.connections.get(&token).is_some_and(|connection| {
            !connection.mandatory.is_empty()
                || connection.wake_inflight.is_some()
                || connection.pending_wake.is_some()
        })
    }

    pub fn close(&mut self, token: u64) -> bool {
        self.connections.remove(&token).is_some()
    }

    pub fn state_of(&self, token: u64) -> Option<ConnectionState> {
        self.connections.get(&token).map(|connection| connection.state)
    }

    pub fn set_state(&mut self, token: u64, state: ConnectionState) {
        if let Some(connection) = self.connections.get_mut(&token) {
            connection.state = state;
        }
    }

    fn close_with_event(&mut self, token: u64, events: &mut Vec<ServerEvent>) {
        if self.connections.remove(&token).is_some() {
            events.push(ServerEvent::Disconnected { token });
        }
    }
}

struct FramingCutError;

fn drain_frames(
    connection: &mut Connection,
    token: u64,
    events: &mut Vec<ServerEvent>,
) -> Result<(), FramingCutError> {
    while events.len() < MAX_FRAMES_PER_READINESS {
        if connection.read_buf.len() < HEADER_LEN {
            return Ok(());
        }
        let header = match FrameHeader::decode(&connection.read_buf[..HEADER_LEN]) {
            Ok(header) => header,
            Err(_) => {
                events.push(ServerEvent::FramingError { token });
                return Err(FramingCutError);
            }
        };
        if header.payload_len > MAX_FRAME_PAYLOAD {
            events.push(ServerEvent::FramingError { token });
            return Err(FramingCutError);
        }
        let total_len = HEADER_LEN
            .checked_add(header.payload_len as usize)
            .ok_or(FramingCutError)?;
        if connection.read_buf.len() < total_len {
            return Ok(());
        }
        let payload = connection.read_buf[HEADER_LEN..total_len].to_vec();
        connection.read_buf.drain(..total_len);
        events.push(ServerEvent::Frame {
            token,
            message_type: header.message_type,
            payload,
        });
    }
    Ok(())
}

fn flush_outbound(connection: &mut Connection) -> io::Result<()> {
    while let Some(item) = connection.mandatory.front_mut() {
        let before = item.remaining_len();
        match flush_item(connection.stream.as_raw_fd(), item)? {
            FlushProgress::WouldBlock => return Ok(()),
            FlushProgress::Progress => {
                let after = item.remaining_len();
                connection.mandatory_bytes = connection
                    .mandatory_bytes
                    .saturating_sub(before.saturating_sub(after));
                if after == 0 {
                    connection.mandatory.pop_front();
                }
            }
        }
    }

    loop {
        if connection.wake_inflight.is_none() {
            let Some(bytes) = connection.pending_wake.take() else {
                return Ok(());
            };
            connection.wake_inflight = Some(OutboundItem::new(bytes, None));
        }
        let Some(item) = connection.wake_inflight.as_mut() else {
            return Ok(());
        };
        match flush_item(connection.stream.as_raw_fd(), item)? {
            FlushProgress::WouldBlock => return Ok(()),
            FlushProgress::Progress if item.remaining_len() == 0 => {
                connection.wake_inflight = None;
                // If a newer wake arrived while this one was in flight,
                // immediately continue with that single coalesced successor.
                continue;
            }
            FlushProgress::Progress => return Ok(()),
        }
    }
}

enum FlushProgress {
    Progress,
    WouldBlock,
}

fn flush_item(socket: RawFd, item: &mut OutboundItem) -> io::Result<FlushProgress> {
    if item.sent >= item.bytes.len() {
        return Ok(FlushProgress::Progress);
    }
    let remaining = &item.bytes[item.sent..];
    let fd = if item.sent == 0 {
        item.fd.as_ref().map(AsRawFd::as_raw_fd)
    } else {
        None
    };
    match fd_transfer::send_with_fd(socket, remaining, fd) {
        Ok(0) => Ok(FlushProgress::WouldBlock),
        Ok(sent) => {
            if sent > remaining.len() {
                return Err(io::Error::other("sendmsg reported impossible byte count"));
            }
            // SCM_RIGHTS is consumed only after at least one byte from this
            // frame was accepted. EAGAIN/zero never loses the descriptor.
            if sent > 0 && item.sent == 0 {
                item.fd = None;
            }
            item.sent += sent;
            Ok(FlushProgress::Progress)
        }
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(FlushProgress::WouldBlock),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local_ipc::framing::{ClientHello, GenerationWake, encode_frame};
    use std::{io::Write, time::Duration};

    fn bind_test_server() -> (LocalIpcServer, std::path::PathBuf) {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let unique = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "syl-c{}-{unique}.sock",
            std::process::id() % 10_000
        ));
        let _ = std::fs::remove_file(&path);
        (LocalIpcServer::bind(&path, MAX_CONNECTIONS).unwrap(), path)
    }

    fn accept_one(server: &mut LocalIpcServer, path: &Path) -> (UnixStream, u64) {
        let client = UnixStream::connect(path).unwrap();
        let events = server.accept_ready().unwrap();
        let token = events
            .iter()
            .find_map(|event| match event {
                ServerEvent::Connected { token } => Some(*token),
                _ => None,
            })
            .unwrap();
        (client, token)
    }

    #[test]
    fn connection_state_machine_rejects_invalid_transitions() {
        assert!(
            ConnectionState::AwaitHello
                .validate_incoming(MessageType::Attach)
                .is_err()
        );
        assert!(
            ConnectionState::Ready
                .validate_incoming(MessageType::ClientHello)
                .is_err()
        );
        assert!(
            ConnectionState::Attached
                .validate_incoming(MessageType::Input)
                .is_ok()
        );
    }

    #[test]
    fn accept_reports_connected_and_verifies_same_user_peer() {
        let (mut server, path) = bind_test_server();
        let (_client, _token) = accept_one(&mut server, &path);
        assert_eq!(server.connection_count(), 1);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn complete_frame_is_reported_once() {
        let (mut server, path) = bind_test_server();
        let (mut client, token) = accept_one(&mut server, &path);
        let frame = encode_frame(
            MessageType::ClientHello,
            &ClientHello {
                client_capabilities: 0,
            }
            .encode(),
        );
        client.write_all(&frame).unwrap();
        let events = server.service_read(token, false);
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, ServerEvent::Frame { .. }))
                .count(),
            1
        );
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn fragmented_frame_is_buffered_within_the_hard_receive_cap() {
        let (mut server, path) = bind_test_server();
        let (mut client, token) = accept_one(&mut server, &path);
        let frame = encode_frame(
            MessageType::ClientHello,
            &ClientHello {
                client_capabilities: 0,
            }
            .encode(),
        );
        client.write_all(&frame[..10]).unwrap();
        assert!(server.service_read(token, false).is_empty());
        client.write_all(&frame[10..]).unwrap();
        assert!(server
            .service_read(token, false)
            .iter()
            .any(|event| matches!(event, ServerEvent::Frame { .. })));
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn malformed_header_closes_connection() {
        let (mut server, path) = bind_test_server();
        let (mut client, token) = accept_one(&mut server, &path);
        client.write_all(&[0xff; HEADER_LEN]).unwrap();
        let events = server.service_read(token, false);
        assert!(events
            .iter()
            .any(|event| matches!(event, ServerEvent::FramingError { .. })));
        assert!(!server.contains(token));
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn mandatory_queue_cap_disconnects_slow_client() {
        let (mut server, path) = bind_test_server();
        let (_client, token) = accept_one(&mut server, &path);
        let bytes = vec![0u8; MAX_OUTBOUND_QUEUE_BYTES + 1];
        assert!(server.enqueue_mandatory(token, bytes, None).is_err());
        assert!(!server.contains(token));
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn pending_generation_wakes_coalesce_to_latest_without_history_queue() {
        let (mut server, path) = bind_test_server();
        let (client, token) = accept_one(&mut server, &path);
        client
            .set_write_timeout(Some(Duration::from_millis(10)))
            .unwrap();
        // A live, writable socket may flush immediately. Force an in-memory
        // coalescing assertion by pre-installing an in-flight wake marker.
        let connection = server.connections.get_mut(&token).unwrap();
        connection.wake_inflight = Some(OutboundItem {
            bytes: vec![1; 64],
            sent: 1,
            fd: None,
        });
        for generation in [2u64, 3, 99] {
            let frame = encode_frame(
                MessageType::GenerationWake,
                &GenerationWake {
                    attachment_id: crate::AttachmentId::from_bytes(1u128.to_le_bytes()),
                    projection_id: crate::ProjectionId::from_bytes(2u128.to_le_bytes()),
                    committed_generation: generation,
                }
                .encode(),
            );
            server.enqueue_wake(token, frame).unwrap();
        }
        let connection = server.connections.get(&token).unwrap();
        assert!(connection.pending_wake.is_some());
        assert_eq!(connection.mandatory.len(), 0);
        std::fs::remove_file(path).ok();
    }
}
