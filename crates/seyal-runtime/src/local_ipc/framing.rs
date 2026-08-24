//! SPEC-004 section 6 binary wire framing.
//!
//! This module owns the exact 24-byte frame header, the M001 message type
//! set and every payload contract. It never uses Rust struct memory layout
//! as the wire format: every field is explicitly read/written at its
//! specified byte offset with defined endianness, and every length is
//! validated before it is used to size an allocation.

use crate::{AttachmentId, ExecutionId, ProjectionId};

pub const MAGIC: [u8; 8] = *b"SEYALIPC";
pub const HEADER_LEN: usize = 24;
pub const MAJOR: u16 = 1;
pub const MINOR: u16 = 0;
pub const MAX_FRAME_PAYLOAD: u32 = 262_144;
pub const MAX_INPUT_BYTES: u32 = 65_536;
pub const MAX_EXECUTION_LIST_ENTRIES: u16 = 512;

/// SPEC-004 section 6.6 protocol error codes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum ErrorCode {
    UnsupportedVersion = 1,
    UnknownMessage = 2,
    InvalidState = 3,
    InvalidExecution = 4,
    InvalidAttachment = 5,
    StaleIdentity = 6,
    PermissionDenied = 7,
    ControllerBusy = 8,
    CapacityExceeded = 9,
    Backpressure = 10,
    InvalidGeometry = 11,
    ProjectionUnavailable = 12,
    MalformedPayload = 13,
    InternalFailure = 14,
}

impl ErrorCode {
    pub fn from_u16(value: u16) -> Option<Self> {
        Some(match value {
            1 => Self::UnsupportedVersion,
            2 => Self::UnknownMessage,
            3 => Self::InvalidState,
            4 => Self::InvalidExecution,
            5 => Self::InvalidAttachment,
            6 => Self::StaleIdentity,
            7 => Self::PermissionDenied,
            8 => Self::ControllerBusy,
            9 => Self::CapacityExceeded,
            10 => Self::Backpressure,
            11 => Self::InvalidGeometry,
            12 => Self::ProjectionUnavailable,
            13 => Self::MalformedPayload,
            14 => Self::InternalFailure,
            _ => return None,
        })
    }
}

/// Decode/validation failures. `Fatal` variants require closing the
/// connection per SPEC-004 section 6.3/6.6; `Semantic` variants map onto a
/// wire `Error` frame and leave the connection in its previous valid state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FramingError {
    TruncatedHeader,
    InvalidMagic,
    NonzeroReserved,
    OversizedPayload,
    LengthOverflow,
    UnsupportedMajorVersion,
    UnsupportedMinorVersion,
    UnknownMessageType,
    TruncatedPayload,
    ExactLengthMismatch,
    MalformedPayload,
}

impl FramingError {
    /// Whether this failure is protocol-fatal (connection must close) as
    /// opposed to a nonfatal semantic error that preserves framing
    /// continuity (SPEC-004 section 6.3).
    pub fn is_fatal(self) -> bool {
        !matches!(self, Self::UnknownMessageType)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameHeader {
    pub message_type: u16,
    pub flags: u16,
    pub payload_len: u32,
}

impl FrameHeader {
    pub fn new(message_type: u16, payload_len: u32) -> Self {
        Self {
            message_type,
            flags: 0,
            payload_len,
        }
    }

    pub fn encode(&self) -> [u8; HEADER_LEN] {
        let mut out = [0u8; HEADER_LEN];
        out[0..8].copy_from_slice(&MAGIC);
        out[8..10].copy_from_slice(&MAJOR.to_le_bytes());
        out[10..12].copy_from_slice(&MINOR.to_le_bytes());
        out[12..14].copy_from_slice(&self.message_type.to_le_bytes());
        out[14..16].copy_from_slice(&self.flags.to_le_bytes());
        out[16..20].copy_from_slice(&self.payload_len.to_le_bytes());
        out[20..24].copy_from_slice(&0u32.to_le_bytes());
        out
    }

    /// Validates the full header, including magic/version/reserved/length
    /// bounds, before any payload is allocated or read (SPEC-004 section
    /// 6.2). `bytes` must be exactly [`HEADER_LEN`] long.
    pub fn decode(bytes: &[u8]) -> Result<Self, FramingError> {
        if bytes.len() < HEADER_LEN {
            return Err(FramingError::TruncatedHeader);
        }
        if bytes[0..8] != MAGIC {
            return Err(FramingError::InvalidMagic);
        }
        let major = u16::from_le_bytes([bytes[8], bytes[9]]);
        let minor = u16::from_le_bytes([bytes[10], bytes[11]]);
        let message_type = u16::from_le_bytes([bytes[12], bytes[13]]);
        let flags = u16::from_le_bytes([bytes[14], bytes[15]]);
        let payload_len = u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
        let reserved = u32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);

        if reserved != 0 {
            return Err(FramingError::NonzeroReserved);
        }
        if major != MAJOR {
            return Err(FramingError::UnsupportedMajorVersion);
        }
        if minor != MINOR {
            return Err(FramingError::UnsupportedMinorVersion);
        }
        // flags must be zero in 1.0 unless a future minor specifies a bit.
        if flags != 0 {
            return Err(FramingError::MalformedPayload);
        }
        if payload_len > MAX_FRAME_PAYLOAD {
            return Err(FramingError::OversizedPayload);
        }
        // HEADER_LEN + payload_len is bounded by MAX_FRAME_PAYLOAD above, so
        // this never overflows usize on any supported target; the explicit
        // checked_add still documents/enforces the invariant defensively.
        HEADER_LEN
            .checked_add(payload_len as usize)
            .ok_or(FramingError::LengthOverflow)?;

        Ok(Self {
            message_type,
            flags,
            payload_len,
        })
    }
}

fn read_u128(bytes: &[u8]) -> u128 {
    let mut raw = [0u8; 16];
    raw.copy_from_slice(&bytes[..16]);
    u128::from_le_bytes(raw)
}

fn write_u128(out: &mut Vec<u8>, value: u128) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn execution_id_from(bytes: &[u8]) -> ExecutionId {
    ExecutionId::from_bytes(read_u128(bytes).to_le_bytes())
}

fn attachment_id_from(bytes: &[u8]) -> AttachmentId {
    AttachmentId::from_bytes(read_u128(bytes).to_le_bytes())
}

fn projection_id_from(bytes: &[u8]) -> ProjectionId {
    ProjectionId::from_bytes(read_u128(bytes).to_le_bytes())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Observer = 0,
    Controller = 1,
}

impl Role {
    fn from_u8(value: u8) -> Result<Self, FramingError> {
        match value {
            0 => Ok(Self::Observer),
            1 => Ok(Self::Controller),
            _ => Err(FramingError::MalformedPayload),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lifecycle {
    Running = 0,
    Terminating = 1,
    Finalized = 2,
}

impl Lifecycle {
    fn from_u8(value: u8) -> Result<Self, FramingError> {
        match value {
            0 => Ok(Self::Running),
            1 => Ok(Self::Terminating),
            2 => Ok(Self::Finalized),
            _ => Err(FramingError::MalformedPayload),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClientHello {
    pub client_capabilities: u32,
}

impl ClientHello {
    pub const WIRE_LEN: usize = 8;

    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(Self::WIRE_LEN);
        out.extend_from_slice(&self.client_capabilities.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, FramingError> {
        exact_len(bytes, Self::WIRE_LEN)?;
        let client_capabilities = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
        let reserved = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        if reserved != 0 {
            return Err(FramingError::MalformedPayload);
        }
        Ok(Self { client_capabilities })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ServerHello {
    pub runtime_id: u128,
    pub server_capabilities: u32,
    pub max_frame_payload: u32,
    pub max_input_payload: u32,
}

impl ServerHello {
    pub const WIRE_LEN: usize = 32;

    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(Self::WIRE_LEN);
        write_u128(&mut out, self.runtime_id);
        out.extend_from_slice(&self.server_capabilities.to_le_bytes());
        out.extend_from_slice(&self.max_frame_payload.to_le_bytes());
        out.extend_from_slice(&self.max_input_payload.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, FramingError> {
        exact_len(bytes, Self::WIRE_LEN)?;
        let runtime_id = read_u128(&bytes[0..16]);
        let server_capabilities = u32::from_le_bytes(bytes[16..20].try_into().unwrap());
        let max_frame_payload = u32::from_le_bytes(bytes[20..24].try_into().unwrap());
        let max_input_payload = u32::from_le_bytes(bytes[24..28].try_into().unwrap());
        let reserved = u32::from_le_bytes(bytes[28..32].try_into().unwrap());
        if reserved != 0 {
            return Err(FramingError::MalformedPayload);
        }
        Ok(Self {
            runtime_id,
            server_capabilities,
            max_frame_payload,
            max_input_payload,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExecutionListEntry {
    pub execution_id: ExecutionId,
    pub lifecycle: Lifecycle,
    pub has_controller: bool,
    pub attachment_count: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionList {
    pub entries: Vec<ExecutionListEntry>,
}

impl ExecutionList {
    const ENTRY_LEN: usize = 20;

    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(4 + self.entries.len() * Self::ENTRY_LEN);
        out.extend_from_slice(&(self.entries.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        for entry in &self.entries {
            write_u128(&mut out, u128::from_le_bytes(entry.execution_id.to_bytes()));
            out.push(entry.lifecycle as u8);
            out.push(entry.has_controller as u8);
            out.extend_from_slice(&entry.attachment_count.to_le_bytes());
        }
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, FramingError> {
        if bytes.len() < 4 {
            return Err(FramingError::TruncatedPayload);
        }
        let count = u16::from_le_bytes([bytes[0], bytes[1]]);
        let reserved = u16::from_le_bytes([bytes[2], bytes[3]]);
        if reserved != 0 {
            return Err(FramingError::MalformedPayload);
        }
        if count > MAX_EXECUTION_LIST_ENTRIES {
            return Err(FramingError::MalformedPayload);
        }
        let expected = 4usize
            .checked_add((count as usize).checked_mul(Self::ENTRY_LEN).ok_or(FramingError::LengthOverflow)?)
            .ok_or(FramingError::LengthOverflow)?;
        exact_len(bytes, expected)?;

        let mut entries = Vec::with_capacity(count as usize);
        let mut offset = 4usize;
        for _ in 0..count {
            let execution_id = execution_id_from(&bytes[offset..offset + 16]);
            let lifecycle = Lifecycle::from_u8(bytes[offset + 16])?;
            let has_controller = match bytes[offset + 17] {
                0 => false,
                1 => true,
                _ => return Err(FramingError::MalformedPayload),
            };
            let attachment_count =
                u16::from_le_bytes([bytes[offset + 18], bytes[offset + 19]]);
            entries.push(ExecutionListEntry {
                execution_id,
                lifecycle,
                has_controller,
                attachment_count,
            });
            offset += Self::ENTRY_LEN;
        }
        Ok(Self { entries })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Attach {
    pub execution_id: ExecutionId,
    pub requested_role: Role,
}

impl Attach {
    pub const WIRE_LEN: usize = 20;

    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(Self::WIRE_LEN);
        write_u128(&mut out, u128::from_le_bytes(self.execution_id.to_bytes()));
        out.push(self.requested_role as u8);
        out.extend_from_slice(&[0u8; 3]);
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, FramingError> {
        exact_len(bytes, Self::WIRE_LEN)?;
        let execution_id = execution_id_from(&bytes[0..16]);
        let requested_role = Role::from_u8(bytes[16])?;
        if bytes[17..20] != [0, 0, 0] {
            return Err(FramingError::MalformedPayload);
        }
        Ok(Self {
            execution_id,
            requested_role,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Attached {
    pub execution_id: ExecutionId,
    pub attachment_id: AttachmentId,
    pub projection_id: ProjectionId,
    pub granted_role: Role,
    pub committed_generation: u64,
    pub region_bytes: u64,
    pub capacity_rows: u16,
    pub capacity_cols: u16,
}

impl Attached {
    pub const WIRE_LEN: usize = 80;

    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(Self::WIRE_LEN);
        write_u128(&mut out, u128::from_le_bytes(self.execution_id.to_bytes()));
        write_u128(&mut out, u128::from_le_bytes(self.attachment_id.to_bytes()));
        write_u128(&mut out, u128::from_le_bytes(self.projection_id.to_bytes()));
        out.push(self.granted_role as u8);
        out.extend_from_slice(&[0u8; 7]);
        out.extend_from_slice(&self.committed_generation.to_le_bytes());
        out.extend_from_slice(&self.region_bytes.to_le_bytes());
        out.extend_from_slice(&self.capacity_rows.to_le_bytes());
        out.extend_from_slice(&self.capacity_cols.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, FramingError> {
        exact_len(bytes, Self::WIRE_LEN)?;
        let execution_id = execution_id_from(&bytes[0..16]);
        let attachment_id = attachment_id_from(&bytes[16..32]);
        let projection_id = projection_id_from(&bytes[32..48]);
        let granted_role = Role::from_u8(bytes[48])?;
        if bytes[49..56] != [0u8; 7] {
            return Err(FramingError::MalformedPayload);
        }
        let committed_generation = u64::from_le_bytes(bytes[56..64].try_into().unwrap());
        let region_bytes = u64::from_le_bytes(bytes[64..72].try_into().unwrap());
        let capacity_rows = u16::from_le_bytes(bytes[72..74].try_into().unwrap());
        let capacity_cols = u16::from_le_bytes(bytes[74..76].try_into().unwrap());
        let reserved = u32::from_le_bytes(bytes[76..80].try_into().unwrap());
        if reserved != 0 {
            return Err(FramingError::MalformedPayload);
        }
        Ok(Self {
            execution_id,
            attachment_id,
            projection_id,
            granted_role,
            committed_generation,
            region_bytes,
            capacity_rows,
            capacity_cols,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Detach {
    pub attachment_id: AttachmentId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Detached {
    pub attachment_id: AttachmentId,
}

macro_rules! impl_single_attachment_id_payload {
    ($type:ty) => {
        impl $type {
            pub const WIRE_LEN: usize = 16;

            pub fn encode(&self) -> Vec<u8> {
                let mut out = Vec::with_capacity(Self::WIRE_LEN);
                write_u128(&mut out, u128::from_le_bytes(self.attachment_id.to_bytes()));
                out
            }

            pub fn decode(bytes: &[u8]) -> Result<Self, FramingError> {
                exact_len(bytes, Self::WIRE_LEN)?;
                Ok(Self {
                    attachment_id: attachment_id_from(&bytes[0..16]),
                })
            }
        }
    };
}

impl_single_attachment_id_payload!(Detach);
impl_single_attachment_id_payload!(Detached);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resync {
    pub attachment_id: AttachmentId,
}

impl_single_attachment_id_payload!(Resync);

/// Borrowed view of an `Input` payload. Bytes are never copied out of the
/// caller-owned receive buffer during decode; the bounded frame length
/// already limits how much memory the borrow can span.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InputRef<'a> {
    pub attachment_id: AttachmentId,
    pub bytes: &'a [u8],
}

impl<'a> InputRef<'a> {
    pub const HEADER_LEN: usize = 20;

    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(Self::HEADER_LEN + self.bytes.len());
        write_u128(&mut out, u128::from_le_bytes(self.attachment_id.to_bytes()));
        out.extend_from_slice(&(self.bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(self.bytes);
        out
    }

    pub fn decode(bytes: &'a [u8]) -> Result<Self, FramingError> {
        if bytes.len() < Self::HEADER_LEN {
            return Err(FramingError::TruncatedPayload);
        }
        let attachment_id = attachment_id_from(&bytes[0..16]);
        let byte_count = u32::from_le_bytes(bytes[16..20].try_into().unwrap());
        if byte_count > MAX_INPUT_BYTES {
            return Err(FramingError::OversizedPayload);
        }
        let expected = Self::HEADER_LEN
            .checked_add(byte_count as usize)
            .ok_or(FramingError::LengthOverflow)?;
        exact_len(bytes, expected)?;
        Ok(Self {
            attachment_id,
            bytes: &bytes[Self::HEADER_LEN..expected],
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resize {
    pub attachment_id: AttachmentId,
    pub rows: u16,
    pub columns: u16,
}

impl Resize {
    pub const WIRE_LEN: usize = 20;

    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(Self::WIRE_LEN);
        write_u128(&mut out, u128::from_le_bytes(self.attachment_id.to_bytes()));
        out.extend_from_slice(&self.rows.to_le_bytes());
        out.extend_from_slice(&self.columns.to_le_bytes());
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, FramingError> {
        exact_len(bytes, Self::WIRE_LEN)?;
        Ok(Self {
            attachment_id: attachment_id_from(&bytes[0..16]),
            rows: u16::from_le_bytes(bytes[16..18].try_into().unwrap()),
            columns: u16::from_le_bytes(bytes[18..20].try_into().unwrap()),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GenerationWake {
    pub attachment_id: AttachmentId,
    pub projection_id: ProjectionId,
    pub committed_generation: u64,
}

impl GenerationWake {
    pub const WIRE_LEN: usize = 40;

    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(Self::WIRE_LEN);
        write_u128(&mut out, u128::from_le_bytes(self.attachment_id.to_bytes()));
        write_u128(&mut out, u128::from_le_bytes(self.projection_id.to_bytes()));
        out.extend_from_slice(&self.committed_generation.to_le_bytes());
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, FramingError> {
        exact_len(bytes, Self::WIRE_LEN)?;
        Ok(Self {
            attachment_id: attachment_id_from(&bytes[0..16]),
            projection_id: projection_id_from(&bytes[16..32]),
            committed_generation: u64::from_le_bytes(bytes[32..40].try_into().unwrap()),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectionReplaced {
    pub execution_id: ExecutionId,
    pub attachment_id: AttachmentId,
    pub projection_id: ProjectionId,
    pub committed_generation: u64,
    pub region_bytes: u64,
    pub capacity_rows: u16,
    pub capacity_cols: u16,
}

impl ProjectionReplaced {
    pub const WIRE_LEN: usize = 72;

    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(Self::WIRE_LEN);
        write_u128(&mut out, u128::from_le_bytes(self.execution_id.to_bytes()));
        write_u128(&mut out, u128::from_le_bytes(self.attachment_id.to_bytes()));
        write_u128(&mut out, u128::from_le_bytes(self.projection_id.to_bytes()));
        out.extend_from_slice(&self.committed_generation.to_le_bytes());
        out.extend_from_slice(&self.region_bytes.to_le_bytes());
        out.extend_from_slice(&self.capacity_rows.to_le_bytes());
        out.extend_from_slice(&self.capacity_cols.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, FramingError> {
        exact_len(bytes, Self::WIRE_LEN)?;
        let execution_id = execution_id_from(&bytes[0..16]);
        let attachment_id = attachment_id_from(&bytes[16..32]);
        let projection_id = projection_id_from(&bytes[32..48]);
        let committed_generation = u64::from_le_bytes(bytes[48..56].try_into().unwrap());
        let region_bytes = u64::from_le_bytes(bytes[56..64].try_into().unwrap());
        let capacity_rows = u16::from_le_bytes(bytes[64..66].try_into().unwrap());
        let capacity_cols = u16::from_le_bytes(bytes[66..68].try_into().unwrap());
        let reserved = u32::from_le_bytes(bytes[68..72].try_into().unwrap());
        if reserved != 0 {
            return Err(FramingError::MalformedPayload);
        }
        Ok(Self {
            execution_id,
            attachment_id,
            projection_id,
            committed_generation,
            region_bytes,
            capacity_rows,
            capacity_cols,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LifecycleMessage {
    pub execution_id: ExecutionId,
    pub lifecycle: Lifecycle,
}

impl LifecycleMessage {
    pub const WIRE_LEN: usize = 24;

    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(Self::WIRE_LEN);
        write_u128(&mut out, u128::from_le_bytes(self.execution_id.to_bytes()));
        out.push(self.lifecycle as u8);
        out.extend_from_slice(&[0u8; 7]);
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, FramingError> {
        exact_len(bytes, Self::WIRE_LEN)?;
        let execution_id = execution_id_from(&bytes[0..16]);
        let lifecycle = Lifecycle::from_u8(bytes[16])?;
        if bytes[17..24] != [0u8; 7] {
            return Err(FramingError::MalformedPayload);
        }
        Ok(Self {
            execution_id,
            lifecycle,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ErrorMessage {
    pub error_code: u16,
    pub offending_message_type: u16,
    pub detail_code: u32,
}

impl ErrorMessage {
    pub const WIRE_LEN: usize = 16;

    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(Self::WIRE_LEN);
        out.extend_from_slice(&self.error_code.to_le_bytes());
        out.extend_from_slice(&self.offending_message_type.to_le_bytes());
        out.extend_from_slice(&self.detail_code.to_le_bytes());
        out.extend_from_slice(&0u64.to_le_bytes());
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, FramingError> {
        exact_len(bytes, Self::WIRE_LEN)?;
        let error_code = u16::from_le_bytes(bytes[0..2].try_into().unwrap());
        let offending_message_type = u16::from_le_bytes(bytes[2..4].try_into().unwrap());
        let detail_code = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        let reserved = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
        if reserved != 0 {
            return Err(FramingError::MalformedPayload);
        }
        Ok(Self {
            error_code,
            offending_message_type,
            detail_code,
        })
    }
}

fn exact_len(bytes: &[u8], expected: usize) -> Result<(), FramingError> {
    if bytes.len() != expected {
        return Err(FramingError::ExactLengthMismatch);
    }
    Ok(())
}

/// The M001 message type IDs (SPEC-004 section 6.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum MessageType {
    ClientHello = 1,
    ServerHello = 2,
    ListExecutions = 3,
    ExecutionList = 4,
    Attach = 5,
    Attached = 6,
    Detach = 7,
    Detached = 8,
    Input = 9,
    Resize = 10,
    Resync = 11,
    GenerationWake = 12,
    ProjectionReplaced = 13,
    Lifecycle = 14,
    Error = 15,
    Goodbye = 16,
}

impl MessageType {
    pub fn from_u16(value: u16) -> Option<Self> {
        Some(match value {
            1 => Self::ClientHello,
            2 => Self::ServerHello,
            3 => Self::ListExecutions,
            4 => Self::ExecutionList,
            5 => Self::Attach,
            6 => Self::Attached,
            7 => Self::Detach,
            8 => Self::Detached,
            9 => Self::Input,
            10 => Self::Resize,
            11 => Self::Resync,
            12 => Self::GenerationWake,
            13 => Self::ProjectionReplaced,
            14 => Self::Lifecycle,
            15 => Self::Error,
            16 => Self::Goodbye,
            _ => return None,
        })
    }
}

/// A fully decoded, typed message body. Borrows `Input` bytes from the
/// caller-owned receive buffer instead of copying them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message<'a> {
    ClientHello(ClientHello),
    ServerHello(ServerHello),
    ListExecutions,
    ExecutionList(ExecutionList),
    Attach(Attach),
    Attached(Attached),
    Detach(Detach),
    Detached(Detached),
    Input(InputRef<'a>),
    Resize(Resize),
    Resync(Resync),
    GenerationWake(GenerationWake),
    ProjectionReplaced(ProjectionReplaced),
    Lifecycle(LifecycleMessage),
    Error(ErrorMessage),
    Goodbye,
}

/// Decodes a full frame (header bytes followed immediately by exactly
/// `header.payload_len` payload bytes) into a typed message.
///
/// An unknown message type under a supported protocol version is nonfatal
/// per SPEC-004 section 6.3: the frame is fully consumed/framed, but this
/// function reports [`FramingError::UnknownMessageType`] so the caller can
/// send `UnknownMessage` and continue.
pub fn decode_message<'a>(
    header: &FrameHeader,
    payload: &'a [u8],
) -> Result<Message<'a>, FramingError> {
    if payload.len() != header.payload_len as usize {
        return Err(FramingError::ExactLengthMismatch);
    }
    let Some(message_type) = MessageType::from_u16(header.message_type) else {
        return Err(FramingError::UnknownMessageType);
    };
    Ok(match message_type {
        MessageType::ClientHello => Message::ClientHello(ClientHello::decode(payload)?),
        MessageType::ServerHello => Message::ServerHello(ServerHello::decode(payload)?),
        MessageType::ListExecutions => {
            if !payload.is_empty() {
                return Err(FramingError::ExactLengthMismatch);
            }
            Message::ListExecutions
        }
        MessageType::ExecutionList => Message::ExecutionList(ExecutionList::decode(payload)?),
        MessageType::Attach => Message::Attach(Attach::decode(payload)?),
        MessageType::Attached => Message::Attached(Attached::decode(payload)?),
        MessageType::Detach => Message::Detach(Detach::decode(payload)?),
        MessageType::Detached => Message::Detached(Detached::decode(payload)?),
        MessageType::Input => Message::Input(InputRef::decode(payload)?),
        MessageType::Resize => Message::Resize(Resize::decode(payload)?),
        MessageType::Resync => Message::Resync(Resync::decode(payload)?),
        MessageType::GenerationWake => Message::GenerationWake(GenerationWake::decode(payload)?),
        MessageType::ProjectionReplaced => {
            Message::ProjectionReplaced(ProjectionReplaced::decode(payload)?)
        }
        MessageType::Lifecycle => Message::Lifecycle(LifecycleMessage::decode(payload)?),
        MessageType::Error => Message::Error(ErrorMessage::decode(payload)?),
        MessageType::Goodbye => {
            if !payload.is_empty() {
                return Err(FramingError::ExactLengthMismatch);
            }
            Message::Goodbye
        }
    })
}

/// Encodes a full frame (header + payload) for the given message type.
pub fn encode_frame(message_type: MessageType, payload: &[u8]) -> Vec<u8> {
    let header = FrameHeader::new(message_type as u16, payload.len() as u32);
    let mut out = Vec::with_capacity(HEADER_LEN + payload.len());
    out.extend_from_slice(&header.encode());
    out.extend_from_slice(payload);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exec_id() -> ExecutionId {
        ExecutionId::from_bytes(1u128.to_le_bytes())
    }

    fn attach_id() -> AttachmentId {
        AttachmentId::from_bytes(2u128.to_le_bytes())
    }

    fn proj_id() -> ProjectionId {
        ProjectionId::from_bytes(3u128.to_le_bytes())
    }

    #[test]
    fn header_round_trips_through_encode_decode() {
        let header = FrameHeader::new(MessageType::Attach as u16, 20);
        let bytes = header.encode();
        let decoded = FrameHeader::decode(&bytes).unwrap();
        assert_eq!(decoded, header);
    }

    #[test]
    fn header_decode_rejects_truncated_bytes() {
        let header = FrameHeader::new(MessageType::Attach as u16, 20);
        let bytes = header.encode();
        assert_eq!(
            FrameHeader::decode(&bytes[..HEADER_LEN - 1]),
            Err(FramingError::TruncatedHeader)
        );
    }

    #[test]
    fn header_decode_rejects_invalid_magic() {
        let mut bytes = FrameHeader::new(MessageType::Attach as u16, 20).encode();
        bytes[0] = b'X';
        assert_eq!(FrameHeader::decode(&bytes), Err(FramingError::InvalidMagic));
    }

    #[test]
    fn header_decode_rejects_nonzero_reserved() {
        let mut bytes = FrameHeader::new(MessageType::Attach as u16, 20).encode();
        bytes[20] = 1;
        assert_eq!(
            FrameHeader::decode(&bytes),
            Err(FramingError::NonzeroReserved)
        );
    }

    #[test]
    fn header_decode_rejects_unsupported_major_version() {
        let mut bytes = FrameHeader::new(MessageType::Attach as u16, 20).encode();
        bytes[8..10].copy_from_slice(&2u16.to_le_bytes());
        assert_eq!(
            FrameHeader::decode(&bytes),
            Err(FramingError::UnsupportedMajorVersion)
        );
    }

    #[test]
    fn header_decode_rejects_unsupported_minor_version() {
        let mut bytes = FrameHeader::new(MessageType::Attach as u16, 20).encode();
        bytes[10..12].copy_from_slice(&1u16.to_le_bytes());
        assert_eq!(
            FrameHeader::decode(&bytes),
            Err(FramingError::UnsupportedMinorVersion)
        );
    }

    #[test]
    fn header_decode_rejects_oversized_payload_length() {
        let mut bytes = FrameHeader::new(MessageType::Attach as u16, 20).encode();
        bytes[16..20].copy_from_slice(&(MAX_FRAME_PAYLOAD + 1).to_le_bytes());
        assert_eq!(
            FrameHeader::decode(&bytes),
            Err(FramingError::OversizedPayload)
        );
    }

    #[test]
    fn header_decode_rejects_length_at_u32_max_without_overflow_panic() {
        let mut bytes = FrameHeader::new(MessageType::Attach as u16, 20).encode();
        bytes[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
        // Must be rejected as oversized, not panic via overflowing arithmetic.
        assert_eq!(
            FrameHeader::decode(&bytes),
            Err(FramingError::OversizedPayload)
        );
    }

    #[test]
    fn header_decode_rejects_unknown_flags() {
        let mut bytes = FrameHeader::new(MessageType::Attach as u16, 20).encode();
        bytes[14..16].copy_from_slice(&1u16.to_le_bytes());
        assert_eq!(
            FrameHeader::decode(&bytes),
            Err(FramingError::MalformedPayload)
        );
    }

    #[test]
    fn attach_payload_round_trips() {
        let attach = Attach {
            execution_id: exec_id(),
            requested_role: Role::Controller,
        };
        let bytes = attach.encode();
        assert_eq!(bytes.len(), Attach::WIRE_LEN);
        assert_eq!(Attach::decode(&bytes).unwrap(), attach);
    }

    #[test]
    fn attach_decode_rejects_invalid_role() {
        let mut bytes = Attach {
            execution_id: exec_id(),
            requested_role: Role::Observer,
        }
        .encode();
        bytes[16] = 2;
        assert_eq!(Attach::decode(&bytes), Err(FramingError::MalformedPayload));
    }

    #[test]
    fn attach_decode_rejects_wrong_length() {
        let bytes = Attach {
            execution_id: exec_id(),
            requested_role: Role::Observer,
        }
        .encode();
        assert_eq!(
            Attach::decode(&bytes[..Attach::WIRE_LEN - 1]),
            Err(FramingError::ExactLengthMismatch)
        );
    }

    #[test]
    fn attached_payload_round_trips() {
        let attached = Attached {
            execution_id: exec_id(),
            attachment_id: attach_id(),
            projection_id: proj_id(),
            granted_role: Role::Controller,
            committed_generation: 42,
            region_bytes: 8192,
            capacity_rows: 24,
            capacity_cols: 80,
        };
        let bytes = attached.encode();
        assert_eq!(bytes.len(), Attached::WIRE_LEN);
        assert_eq!(Attached::decode(&bytes).unwrap(), attached);
    }

    #[test]
    fn input_ref_borrows_payload_bytes_without_copy() {
        let payload = InputRef {
            attachment_id: attach_id(),
            bytes: b"hello",
        };
        let encoded = payload.encode();
        let decoded = InputRef::decode(&encoded).unwrap();
        assert_eq!(decoded.attachment_id, attach_id());
        assert_eq!(decoded.bytes, b"hello");
        // The decoded slice must literally point back into `encoded`.
        assert_eq!(
            decoded.bytes.as_ptr(),
            encoded[InputRef::HEADER_LEN..].as_ptr()
        );
    }

    #[test]
    fn input_ref_decode_rejects_oversized_byte_count() {
        let mut bytes = vec![0u8; InputRef::HEADER_LEN];
        bytes[16..20].copy_from_slice(&(MAX_INPUT_BYTES + 1).to_le_bytes());
        assert_eq!(InputRef::decode(&bytes), Err(FramingError::OversizedPayload));
    }

    #[test]
    fn input_ref_decode_rejects_length_mismatch() {
        let mut bytes = vec![0u8; InputRef::HEADER_LEN];
        bytes[16..20].copy_from_slice(&10u32.to_le_bytes());
        // Declares 10 bytes but supplies none.
        assert_eq!(
            InputRef::decode(&bytes),
            Err(FramingError::ExactLengthMismatch)
        );
    }

    #[test]
    fn execution_list_round_trips_multiple_entries() {
        let list = ExecutionList {
            entries: vec![
                ExecutionListEntry {
                    execution_id: exec_id(),
                    lifecycle: Lifecycle::Running,
                    has_controller: true,
                    attachment_count: 1,
                },
                ExecutionListEntry {
                    execution_id: ExecutionId::from_bytes(99u128.to_le_bytes()),
                    lifecycle: Lifecycle::Finalized,
                    has_controller: false,
                    attachment_count: 0,
                },
            ],
        };
        let bytes = list.encode();
        assert_eq!(ExecutionList::decode(&bytes).unwrap(), list);
    }

    #[test]
    fn execution_list_decode_rejects_count_above_hard_maximum() {
        let mut bytes = vec![0u8; 4];
        bytes[0..2].copy_from_slice(&(MAX_EXECUTION_LIST_ENTRIES + 1).to_le_bytes());
        assert_eq!(
            ExecutionList::decode(&bytes),
            Err(FramingError::MalformedPayload)
        );
    }

    #[test]
    fn generation_wake_round_trips() {
        let wake = GenerationWake {
            attachment_id: attach_id(),
            projection_id: proj_id(),
            committed_generation: 7,
        };
        let bytes = wake.encode();
        assert_eq!(GenerationWake::decode(&bytes).unwrap(), wake);
    }

    #[test]
    fn error_message_round_trips() {
        let error = ErrorMessage {
            error_code: ErrorCode::ControllerBusy as u16,
            offending_message_type: MessageType::Attach as u16,
            detail_code: 0,
        };
        let bytes = error.encode();
        assert_eq!(ErrorMessage::decode(&bytes).unwrap(), error);
    }

    #[test]
    fn decode_message_dispatches_every_known_type() {
        let attach = Attach {
            execution_id: exec_id(),
            requested_role: Role::Observer,
        };
        let payload = attach.encode();
        let header = FrameHeader::new(MessageType::Attach as u16, payload.len() as u32);
        match decode_message(&header, &payload).unwrap() {
            Message::Attach(decoded) => assert_eq!(decoded, attach),
            other => panic!("unexpected message: {other:?}"),
        }
    }

    #[test]
    fn decode_message_reports_unknown_message_type_as_nonfatal() {
        let header = FrameHeader::new(999, 0);
        let error = decode_message(&header, &[]).unwrap_err();
        assert_eq!(error, FramingError::UnknownMessageType);
        assert!(!error.is_fatal());
    }

    #[test]
    fn decode_message_rejects_payload_length_mismatch() {
        let header = FrameHeader::new(MessageType::Goodbye as u16, 0);
        let error = decode_message(&header, &[1, 2, 3]).unwrap_err();
        assert_eq!(error, FramingError::ExactLengthMismatch);
    }

    #[test]
    fn encode_frame_produces_header_and_payload_contiguously() {
        let payload = Detach {
            attachment_id: attach_id(),
        }
        .encode();
        let frame = encode_frame(MessageType::Detach, &payload);
        assert_eq!(frame.len(), HEADER_LEN + payload.len());
        let header = FrameHeader::decode(&frame[..HEADER_LEN]).unwrap();
        assert_eq!(header.message_type, MessageType::Detach as u16);
        assert_eq!(&frame[HEADER_LEN..], payload.as_slice());
    }

    #[test]
    fn fuzz_arbitrary_bytes_never_panics_decoding_header_or_message() {
        // Cheap deterministic pseudo-random smoke coverage; the dedicated
        // cargo-fuzz target provides sustained/coverage-guided fuzzing.
        let mut state: u64 = 0x1234_5678_9abc_def0;
        for _ in 0..20_000 {
            let mut buf = [0u8; 64];
            for byte in buf.iter_mut() {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                *byte = (state & 0xff) as u8;
            }
            if let Ok(header) = FrameHeader::decode(&buf) {
                let _ = decode_message(&header, &buf[HEADER_LEN..HEADER_LEN.min(buf.len())]);
            }
        }
    }
}
