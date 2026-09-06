//! Bounded terminal-generated protocol replies for the child PTY.
//!
//! Protocol semantics stay in `seyal-terminal`. Transport layers only deliver
//! opaque bytes; they must not interpret DECRQM/DSR/DA or similar queries.

/// Maximum queued replies awaiting transport. Matches shell-integration bound.
pub const MAX_PROTOCOL_REPLIES: usize = 16;

/// Maximum bytes in one reply payload (stack-resident; no heap on enqueue).
pub const MAX_PROTOCOL_REPLY_BYTES: usize = 64;

/// Opaque VT protocol bytes that must be written back to the same child PTY.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProtocolReply {
    len: u8,
    bytes: [u8; MAX_PROTOCOL_REPLY_BYTES],
}

impl ProtocolReply {
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..usize::from(self.len)]
    }

    pub(crate) fn from_slice(data: &[u8]) -> Option<Self> {
        if data.is_empty() || data.len() > MAX_PROTOCOL_REPLY_BYTES {
            return None;
        }
        let mut bytes = [0_u8; MAX_PROTOCOL_REPLY_BYTES];
        bytes[..data.len()].copy_from_slice(data);
        Some(Self {
            len: data.len() as u8,
            bytes,
        })
    }
}

pub(crate) fn encode_dsr_cpr(row: u16, col: u16) -> Option<ProtocolReply> {
    // CSI row ; col R with 1-based coordinates.
    let mut buf = [0_u8; MAX_PROTOCOL_REPLY_BYTES];
    let mut len = 0usize;
    buf[len] = 0x1b;
    len += 1;
    buf[len] = b'[';
    len += 1;
    len += write_u16(&mut buf[len..], row.saturating_add(1))?;
    buf[len] = b';';
    len += 1;
    len += write_u16(&mut buf[len..], col.saturating_add(1))?;
    buf[len] = b'R';
    len += 1;
    ProtocolReply::from_slice(&buf[..len])
}

pub(crate) fn encode_decrqm_private(mode: u16, status: u16) -> Option<ProtocolReply> {
    // CSI ? mode ; status $ y
    let mut buf = [0_u8; MAX_PROTOCOL_REPLY_BYTES];
    let mut len = 0usize;
    buf[len] = 0x1b;
    len += 1;
    buf[len] = b'[';
    len += 1;
    buf[len] = b'?';
    len += 1;
    len += write_u16(&mut buf[len..], mode)?;
    buf[len] = b';';
    len += 1;
    len += write_u16(&mut buf[len..], status)?;
    buf[len] = b'$';
    len += 1;
    buf[len] = b'y';
    len += 1;
    ProtocolReply::from_slice(&buf[..len])
}

fn write_u16(out: &mut [u8], value: u16) -> Option<usize> {
    let mut tmp = [0_u8; 5];
    let mut n = value;
    let mut i = tmp.len();
    loop {
        i -= 1;
        tmp[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    let digits = &tmp[i..];
    if out.len() < digits.len() {
        return None;
    }
    out[..digits.len()].copy_from_slice(digits);
    Some(digits.len())
}
