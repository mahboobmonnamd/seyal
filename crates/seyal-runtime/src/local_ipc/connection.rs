//! Candidate-D local Unix-domain transport.
//! Control is ordered/bounded; display state is replaceable and never blocks PTY progress.

use std::{
    collections::{HashMap, VecDeque},
    io,
    os::fd::{AsRawFd, RawFd},
    os::unix::net::{UnixListener, UnixStream},
    path::Path,
    sync::Arc,
};

use crate::{
    display::{DisplayKind, EncodedDisplayBatch},
    local_ipc::{
        auth,
        fd_transfer::{self, RecvFd},
        framing::{FrameHeader, HEADER_LEN, MAX_FRAME_PAYLOAD, MessageType},
    },
};
#[cfg(feature = "test-fault-injection")]
use crate::test_fault::{self, FaultPoint};

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeltaEnqueueResult {
    Queued,
    Skipped,
    NeedSnapshot,
}

struct OutboundItem {
    bytes: Vec<u8>,
    sent: usize,
}

impl OutboundItem {
    fn new(bytes: Vec<u8>) -> Self {
        Self { bytes, sent: 0 }
    }

    fn remaining_len(&self) -> usize {
        self.bytes.len().saturating_sub(self.sent)
    }
}

struct DisplayItem {
    batch: EncodedDisplayBatch,
    frame_index: usize,
    sent: usize,
}

impl DisplayItem {
    fn new(batch: EncodedDisplayBatch) -> Self {
        Self {
            batch,
            frame_index: 0,
            sent: 0,
        }
    }
}

struct Connection {
    stream: UnixStream,
    state: ConnectionState,
    read_buf: Vec<u8>,
    mandatory: VecDeque<OutboundItem>,
    after_display: VecDeque<OutboundItem>,
    queued_control_bytes: usize,
    display_inflight: Option<DisplayItem>,
    pending_display: Option<EncodedDisplayBatch>,
    display_generation: u64,
}

impl Connection {
    fn queue_snapshot(&mut self, snapshot: EncodedDisplayBatch) {
        self.display_generation = snapshot.generation;
        self.pending_display = Some(snapshot);
    }

    fn try_queue_delta(&mut self, delta: EncodedDisplayBatch) -> DeltaEnqueueResult {
        if delta.generation <= self.display_generation {
            return DeltaEnqueueResult::Skipped;
        }
        if self.display_generation != delta.base_generation || self.pending_display.is_some() {
            return DeltaEnqueueResult::NeedSnapshot;
        }
        self.display_generation = delta.generation;
        self.pending_display = Some(delta);
        DeltaEnqueueResult::Queued
    }

    fn has_snapshot_delivery(&self) -> bool {
        self.display_inflight
            .as_ref()
            .is_some_and(|item| item.batch.kind == DisplayKind::Snapshot)
            || self
                .pending_display
                .as_ref()
                .is_some_and(|batch| batch.kind == DisplayKind::Snapshot)
    }
}

#[derive(Debug)]
pub enum ServerEvent {
    Connected {
        token: u64,
    },
    Frame {
        token: u64,
        message_type: u16,
        payload: Vec<u8>,
    },
    FramingError {
        token: u64,
    },
    Disconnected {
        token: u64,
    },
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
        set_close_on_exec(listener.as_raw_fd())?;
        listener.set_nonblocking(true)?;
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

    pub fn state_of(&self, token: u64) -> Option<ConnectionState> {
        self.connections.get(&token).map(|connection| connection.state)
    }

    pub fn presentation_generation(&self, token: u64) -> Option<u64> {
        self.connections
            .get(&token)
            .map(|connection| connection.display_generation)
    }

    pub fn has_snapshot_delivery(&self, token: u64) -> bool {
        self.connections
            .get(&token)
            .is_some_and(Connection::has_snapshot_delivery)
    }

    pub fn set_state(&mut self, token: u64, state: ConnectionState) {
        if let Some(connection) = self.connections.get_mut(&token) {
            connection.state = state;
        }
    }

    pub fn accept_ready(&mut self) -> io::Result<Vec<ServerEvent>> {
        let mut events = Vec::new();
        loop {
            match self.listener.accept() {
                Ok((stream, _)) => {
                    if set_close_on_exec(stream.as_raw_fd()).is_err() {
                        continue;
                    }
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
                            after_display: VecDeque::new(),
                            queued_control_bytes: 0,
                            display_inflight: None,
                            pending_display: None,
                            display_generation: 0,
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

    pub fn service_read(&mut self, token: u64, hangup: bool) -> Vec<ServerEvent> {
        let mut events = Vec::new();
        let Some(connection) = self.connections.get_mut(&token) else {
            return events;
        };
        let mut chunk = [0u8; READ_CHUNK_BYTES];
        while events.len() < MAX_FRAMES_PER_READINESS {
            let remaining = MAX_RECEIVE_BUFFER_BYTES.saturating_sub(connection.read_buf.len());
            if remaining == 0 {
                events.push(ServerEvent::FramingError { token });
                self.close_with_event(token, &mut events);
                return events;
            }
            match fd_transfer::recv_with_fd(
                connection.stream.as_raw_fd(),
                &mut chunk[..READ_CHUNK_BYTES.min(remaining)],
            ) {
                Ok((0, RecvFd::None)) => {
                    self.close_with_event(token, &mut events);
                    return events;
                }
                Ok((count, RecvFd::None)) => {
                    connection.read_buf.extend_from_slice(&chunk[..count]);
                    if drain_frames(connection, token, &mut events).is_err() {
                        self.close_with_event(token, &mut events);
                        return events;
                    }
                }
                Ok((_count, RecvFd::One(fd))) => {
                    drop(fd);
                    events.push(ServerEvent::FramingError { token });
                    self.close_with_event(token, &mut events);
                    return events;
                }
                Ok((_count, RecvFd::Malformed)) => {
                    events.push(ServerEvent::FramingError { token });
                    self.close_with_event(token, &mut events);
                    return events;
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(_) => {
                    self.close_with_event(token, &mut events);
                    return events;
                }
            }
        }
        if hangup {
            if self
                .connections
                .get(&token)
                .is_some_and(|connection| !connection.read_buf.is_empty())
            {
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
        #[cfg(feature = "test-fault-injection")]
        if test_fault::take(FaultPoint::AttachFlush) {
            self.close_with_event(token, &mut events);
            return events;
        }
        if flush_outbound(connection).is_err() {
            self.close_with_event(token, &mut events);
        }
        events
    }

    pub fn enqueue_mandatory(&mut self, token: u64, bytes: Vec<u8>) -> io::Result<()> {
        self.enqueue_control(token, bytes, false)
    }

    pub fn enqueue_after_display(&mut self, token: u64, bytes: Vec<u8>) -> io::Result<()> {
        self.enqueue_control(token, bytes, true)
    }

    pub fn enqueue_attach_transaction(
        &mut self,
        token: u64,
        attached: Vec<u8>,
        snapshot: EncodedDisplayBatch,
    ) -> io::Result<()> {
        #[cfg(feature = "test-fault-injection")]
        if test_fault::take(FaultPoint::AttachAdmission) {
            return Err(io::Error::other("injected attach admission failure"));
        }
        let Some(connection) = self.connections.get_mut(&token) else {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "connection is closed",
            ));
        };
        if connection.display_inflight.is_some() || connection.pending_display.is_some() {
            return Err(io::Error::other(
                "attach transaction requires empty display queue",
            ));
        }
        let new_total = connection
            .queued_control_bytes
            .checked_add(attached.len())
            .ok_or_else(|| io::Error::other("control queue length overflow"))?;
        if new_total > MAX_OUTBOUND_QUEUE_BYTES {
            return Err(io::Error::other(
                "outbound control queue capacity exceeded",
            ));
        }
        connection.queued_control_bytes = new_total;
        connection.mandatory.push_back(OutboundItem::new(attached));
        connection.queue_snapshot(snapshot);
        Ok(())
    }

    fn enqueue_control(
        &mut self,
        token: u64,
        bytes: Vec<u8>,
        after_display: bool,
    ) -> io::Result<()> {
        let Some(connection) = self.connections.get_mut(&token) else {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "connection is closed",
            ));
        };
        let new_total = connection
            .queued_control_bytes
            .checked_add(bytes.len())
            .ok_or_else(|| io::Error::other("control queue length overflow"))?;
        if new_total > MAX_OUTBOUND_QUEUE_BYTES {
            return Err(io::Error::other(
                "outbound control queue capacity exceeded",
            ));
        }
        connection.queued_control_bytes = new_total;
        if after_display {
            connection.after_display.push_back(OutboundItem::new(bytes));
        } else {
            connection.mandatory.push_back(OutboundItem::new(bytes));
        }
        Ok(())
    }

    pub fn enqueue_snapshot(
        &mut self,
        token: u64,
        snapshot: EncodedDisplayBatch,
    ) -> io::Result<()> {
        let Some(connection) = self.connections.get_mut(&token) else {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "connection is closed",
            ));
        };
        connection.queue_snapshot(snapshot);
        Ok(())
    }

    pub fn try_enqueue_delta(
        &mut self,
        token: u64,
        delta: EncodedDisplayBatch,
    ) -> io::Result<DeltaEnqueueResult> {
        let Some(connection) = self.connections.get_mut(&token) else {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "connection is closed",
            ));
        };
        Ok(connection.try_queue_delta(delta))
    }

    pub fn wants_write(&self, token: u64) -> bool {
        self.connections.get(&token).is_some_and(|connection| {
            !connection.mandatory.is_empty()
                || connection.display_inflight.is_some()
                || connection.pending_display.is_some()
                || !connection.after_display.is_empty()
        })
    }

    pub fn close(&mut self, token: u64) -> bool {
        self.connections.remove(&token).is_some()
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
        let total = HEADER_LEN
            .checked_add(header.payload_len as usize)
            .ok_or(FramingCutError)?;
        if connection.read_buf.len() < total {
            return Ok(());
        }
        let payload = connection.read_buf[HEADER_LEN..total].to_vec();
        connection.read_buf.drain(..total);
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
        match flush_bytes(connection.stream.as_raw_fd(), &item.bytes, &mut item.sent)? {
            FlushProgress::WouldBlock => return Ok(()),
            FlushProgress::Progress => {
                let after = item.remaining_len();
                connection.queued_control_bytes = connection
                    .queued_control_bytes
                    .saturating_sub(before.saturating_sub(after));
                if after == 0 {
                    connection.mandatory.pop_front();
                }
            }
        }
    }

    loop {
        if connection.display_inflight.is_none() {
            let Some(batch) = connection.pending_display.take() else {
                break;
            };
            connection.display_inflight = Some(DisplayItem::new(batch));
        }
        let Some(item) = connection.display_inflight.as_mut() else {
            break;
        };
        if item.frame_index >= item.batch.frames.len() {
            connection.display_inflight = None;
            continue;
        }
        let frame = Arc::clone(&item.batch.frames[item.frame_index]);
        match flush_bytes(connection.stream.as_raw_fd(), &frame, &mut item.sent)? {
            FlushProgress::WouldBlock => return Ok(()),
            FlushProgress::Progress => {
                if item.sent == frame.len() {
                    item.frame_index += 1;
                    item.sent = 0;
                    if item.frame_index >= item.batch.frames.len() {
                        connection.display_inflight = None;
                        continue;
                    }
                }
                return Ok(());
            }
        }
    }

    while let Some(item) = connection.after_display.front_mut() {
        let before = item.remaining_len();
        match flush_bytes(connection.stream.as_raw_fd(), &item.bytes, &mut item.sent)? {
            FlushProgress::WouldBlock => return Ok(()),
            FlushProgress::Progress => {
                let after = item.remaining_len();
                connection.queued_control_bytes = connection
                    .queued_control_bytes
                    .saturating_sub(before.saturating_sub(after));
                if after == 0 {
                    connection.after_display.pop_front();
                }
            }
        }
    }
    Ok(())
}

enum FlushProgress {
    Progress,
    WouldBlock,
}

fn flush_bytes(socket: RawFd, bytes: &[u8], sent: &mut usize) -> io::Result<FlushProgress> {
    if *sent >= bytes.len() {
        return Ok(FlushProgress::Progress);
    }
    match fd_transfer::send_with_fd(socket, &bytes[*sent..], None) {
        Ok(0) => Ok(FlushProgress::WouldBlock),
        Ok(count) => {
            if count > bytes.len() - *sent {
                return Err(io::Error::other("sendmsg reported impossible byte count"));
            }
            *sent += count;
            Ok(FlushProgress::Progress)
        }
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(FlushProgress::WouldBlock),
        Err(error) => Err(error),
    }
}

fn set_close_on_exec(fd: RawFd) -> io::Result<()> {
    // SAFETY: `fd` is a live descriptor borrowed from an owning socket.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }
    if flags & libc::FD_CLOEXEC == 0
        && unsafe { libc::fcntl(fd, libc::F_SETFD, flags | libc::FD_CLOEXEC) } < 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::{encode_delta, encode_snapshot};
    use seyal_exec::{
        ProjectionAttributes, ProjectionCell, ProjectionColor, ProjectionDamage,
        TerminalProjectionSnapshot, TerminalProjectionUpdate,
    };

    fn sample_cells(count: usize) -> Vec<ProjectionCell> {
        vec![
            ProjectionCell {
                scalar: 'x',
                foreground: ProjectionColor::Default,
                background: ProjectionColor::Default,
                attributes: ProjectionAttributes::default(),
            };
            count
        ]
    }

    fn snapshot(generation: u64) -> TerminalProjectionSnapshot {
        TerminalProjectionSnapshot {
            rows: 2,
            columns: 2,
            cursor_row: 0,
            cursor_col: 0,
            cursor_visible: true,
            alternate_screen: false,
            source_damage_generation: generation,
            cells: sample_cells(4),
        }
    }

    fn update(generation: u64) -> TerminalProjectionUpdate {
        TerminalProjectionUpdate {
            rows: 2,
            columns: 2,
            cursor_row: 0,
            cursor_col: 0,
            cursor_visible: true,
            alternate_screen: false,
            source_damage_generation: generation,
            damage: ProjectionDamage {
                full: false,
                first_row: 0,
                last_row: 0,
            },
            cells: sample_cells(2),
        }
    }

    fn connection() -> Connection {
        Connection {
            stream: UnixStream::pair().unwrap().0,
            state: ConnectionState::Attached,
            read_buf: Vec::new(),
            mandatory: VecDeque::new(),
            after_display: VecDeque::new(),
            queued_control_bytes: 0,
            display_inflight: None,
            pending_display: None,
            display_generation: 1,
        }
    }

    #[test]
    fn pending_slot_prevents_unbounded_display_history() {
        let mut connection = connection();
        assert_eq!(
            connection.try_queue_delta(encode_delta(&update(2), 1).unwrap()),
            DeltaEnqueueResult::Queued
        );
        assert_eq!(
            connection.try_queue_delta(encode_delta(&update(3), 2).unwrap()),
            DeltaEnqueueResult::NeedSnapshot
        );
        assert_eq!(connection.pending_display.as_ref().unwrap().generation, 2);
    }

    #[test]
    fn snapshot_supersedes_not_started_pending_state() {
        let mut connection = connection();
        connection.queue_snapshot(encode_snapshot(&snapshot(7)).unwrap());
        connection.queue_snapshot(encode_snapshot(&snapshot(9)).unwrap());
        assert_eq!(connection.display_generation, 9);
        assert!(connection.has_snapshot_delivery());
    }
}