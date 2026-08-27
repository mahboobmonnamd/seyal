use crate::{
    AttachmentId,
    display::{MAX_DISPLAY_COLUMNS, MAX_DISPLAY_ROWS},
    framing::{ErrorCode, FramingError},
};

pub const CAP_SEMANTIC_TERMINAL_KEY: u32 = 1 << 2;
pub const CAP_CORRELATED_RESIZE: u32 = 1 << 3;

fn exact_len(bytes: &[u8], expected: usize) -> Result<(), FramingError> {
    if bytes.len() != expected {
        return Err(FramingError::ExactLengthMismatch);
    }
    Ok(())
}

fn attachment_id_from(bytes: &[u8]) -> AttachmentId {
    let mut raw = [0u8; 16];
    raw.copy_from_slice(&bytes[..16]);
    AttachmentId::from_bytes(raw)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum TerminalKeyKind {
    Enter = 1,
    Tab = 2,
    Backspace = 3,
    Escape = 4,
    ArrowUp = 5,
    ArrowDown = 6,
    ArrowRight = 7,
    ArrowLeft = 8,
    ControlAscii = 9,
}

impl TerminalKeyKind {
    fn from_u16(value: u16) -> Result<Self, FramingError> {
        match value {
            1 => Ok(Self::Enter),
            2 => Ok(Self::Tab),
            3 => Ok(Self::Backspace),
            4 => Ok(Self::Escape),
            5 => Ok(Self::ArrowUp),
            6 => Ok(Self::ArrowDown),
            7 => Ok(Self::ArrowRight),
            8 => Ok(Self::ArrowLeft),
            9 => Ok(Self::ControlAscii),
            _ => Err(FramingError::MalformedPayload),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminalKeyModifiers(u16);

impl TerminalKeyModifiers {
    pub const NONE: Self = Self(0);
    pub const CONTROL: Self = Self(1 << 0);

    pub const fn bits(self) -> u16 {
        self.0
    }

    fn from_bits(bits: u16) -> Result<Self, FramingError> {
        match bits {
            0 => Ok(Self::NONE),
            1 => Ok(Self::CONTROL),
            _ => Err(FramingError::MalformedPayload),
        }
    }
}

fn valid_control_ascii_scalar(scalar: u32) -> bool {
    matches!(
        scalar,
        0x20 | 0x3f | 0x40 | 0x41..=0x5f
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminalKey {
    pub attachment_id: AttachmentId,
    pub kind: TerminalKeyKind,
    pub modifiers: TerminalKeyModifiers,
    pub scalar: u32,
}

impl TerminalKey {
    pub const WIRE_LEN: usize = 24;

    pub fn encode(&self) -> Vec<u8> {
        debug_assert!(self.validate().is_ok());
        let mut out = Vec::with_capacity(Self::WIRE_LEN);
        out.extend_from_slice(&self.attachment_id.to_bytes());
        out.extend_from_slice(&(self.kind as u16).to_le_bytes());
        out.extend_from_slice(&self.modifiers.bits().to_le_bytes());
        out.extend_from_slice(&self.scalar.to_le_bytes());
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, FramingError> {
        exact_len(bytes, Self::WIRE_LEN)?;
        let value = Self {
            attachment_id: attachment_id_from(&bytes[..16]),
            kind: TerminalKeyKind::from_u16(u16::from_le_bytes(
                bytes[16..18].try_into().unwrap(),
            ))?,
            modifiers: TerminalKeyModifiers::from_bits(u16::from_le_bytes(
                bytes[18..20].try_into().unwrap(),
            ))?,
            scalar: u32::from_le_bytes(bytes[20..24].try_into().unwrap()),
        };
        value.validate()?;
        Ok(value)
    }

    fn validate(&self) -> Result<(), FramingError> {
        match self.kind {
            TerminalKeyKind::ControlAscii => {
                if self.modifiers != TerminalKeyModifiers::CONTROL
                    || !valid_control_ascii_scalar(self.scalar)
                {
                    return Err(FramingError::MalformedPayload);
                }
            }
            _ => {
                if self.modifiers != TerminalKeyModifiers::NONE || self.scalar != 0 {
                    return Err(FramingError::MalformedPayload);
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResizeRequest {
    pub attachment_id: AttachmentId,
    pub request_id: u64,
    pub rows: u16,
    pub columns: u16,
}

impl ResizeRequest {
    pub const WIRE_LEN: usize = 32;

    pub fn encode(&self) -> Vec<u8> {
        debug_assert!(self.validate().is_ok());
        let mut out = Vec::with_capacity(Self::WIRE_LEN);
        out.extend_from_slice(&self.attachment_id.to_bytes());
        out.extend_from_slice(&self.request_id.to_le_bytes());
        out.extend_from_slice(&self.rows.to_le_bytes());
        out.extend_from_slice(&self.columns.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, FramingError> {
        exact_len(bytes, Self::WIRE_LEN)?;
        if u32::from_le_bytes(bytes[28..32].try_into().unwrap()) != 0 {
            return Err(FramingError::MalformedPayload);
        }
        let value = Self {
            attachment_id: attachment_id_from(&bytes[..16]),
            request_id: u64::from_le_bytes(bytes[16..24].try_into().unwrap()),
            rows: u16::from_le_bytes(bytes[24..26].try_into().unwrap()),
            columns: u16::from_le_bytes(bytes[26..28].try_into().unwrap()),
        };
        value.validate()?;
        Ok(value)
    }

    fn validate(&self) -> Result<(), FramingError> {
        if self.request_id == 0
            || self.rows == 0
            || self.columns == 0
            || self.rows > MAX_DISPLAY_ROWS
            || self.columns > MAX_DISPLAY_COLUMNS
        {
            return Err(FramingError::MalformedPayload);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResizeResultCode {
    Applied,
    Error(ErrorCode),
}

impl ResizeResultCode {
    pub const fn wire_value(self) -> u16 {
        match self {
            Self::Applied => 0,
            Self::Error(error) => error as u16,
        }
    }

    fn from_u16(value: u16) -> Result<Self, FramingError> {
        if value == 0 {
            return Ok(Self::Applied);
        }
        ErrorCode::from_u16(value)
            .map(Self::Error)
            .ok_or(FramingError::MalformedPayload)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResizeResult {
    pub attachment_id: AttachmentId,
    pub request_id: u64,
    pub result_code: ResizeResultCode,
    pub detail_code: u32,
    pub applied_generation: u64,
}

impl ResizeResult {
    pub const WIRE_LEN: usize = 40;

    pub fn encode(&self) -> Vec<u8> {
        debug_assert!(self.validate().is_ok());
        let mut out = Vec::with_capacity(Self::WIRE_LEN);
        out.extend_from_slice(&self.attachment_id.to_bytes());
        out.extend_from_slice(&self.request_id.to_le_bytes());
        out.extend_from_slice(&self.result_code.wire_value().to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&self.detail_code.to_le_bytes());
        out.extend_from_slice(&self.applied_generation.to_le_bytes());
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, FramingError> {
        exact_len(bytes, Self::WIRE_LEN)?;
        if u16::from_le_bytes(bytes[26..28].try_into().unwrap()) != 0 {
            return Err(FramingError::MalformedPayload);
        }
        let value = Self {
            attachment_id: attachment_id_from(&bytes[..16]),
            request_id: u64::from_le_bytes(bytes[16..24].try_into().unwrap()),
            result_code: ResizeResultCode::from_u16(u16::from_le_bytes(
                bytes[24..26].try_into().unwrap(),
            ))?,
            detail_code: u32::from_le_bytes(bytes[28..32].try_into().unwrap()),
            applied_generation: u64::from_le_bytes(bytes[32..40].try_into().unwrap()),
        };
        value.validate()?;
        Ok(value)
    }

    fn validate(&self) -> Result<(), FramingError> {
        if self.request_id == 0 || self.detail_code != 0 {
            return Err(FramingError::MalformedPayload);
        }
        match self.result_code {
            ResizeResultCode::Applied if self.applied_generation != 0 => Ok(()),
            ResizeResultCode::Error(_) if self.applied_generation == 0 => Ok(()),
            _ => Err(FramingError::MalformedPayload),
        }
    }
}
