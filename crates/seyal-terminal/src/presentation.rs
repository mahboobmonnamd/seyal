//! Bounded, untrusted host-presentation hooks from OSC sequences.
//!
//! Payloads are opaque bytes for the host chrome. They must never become
//! filesystem, process, network, or approval authority by themselves.

/// Maximum queued presentation events awaiting the host.
pub const MAX_HOST_PRESENTATION_EVENTS: usize = 16;

/// Maximum retained payload bytes per presentation field.
pub const MAX_PRESENTATION_PAYLOAD_BYTES: usize = 1024;

/// Opaque bounded byte payload copied out of an OSC string.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresentationPayload {
    len: u16,
    bytes: [u8; MAX_PRESENTATION_PAYLOAD_BYTES],
}

impl PresentationPayload {
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..usize::from(self.len)]
    }

    pub(crate) fn from_slice(data: &[u8]) -> Self {
        let take = data.len().min(MAX_PRESENTATION_PAYLOAD_BYTES);
        let mut bytes = [0_u8; MAX_PRESENTATION_PAYLOAD_BYTES];
        bytes[..take].copy_from_slice(&data[..take]);
        Self {
            len: take as u16,
            bytes,
        }
    }
}

/// Host-facing presentation signals derived from OSC 0/2/7/8.
///
/// These events are untrusted terminal text. Embedders may display them or
/// apply explicit host policy; they must not execute payload content.
/// Large payloads are boxed so the enum discriminant stays small on the queue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostPresentationEvent {
    /// OSC 0 — icon name and window title.
    IconAndWindowTitle(Box<PresentationPayload>),
    /// OSC 2 — window title only.
    WindowTitle(Box<PresentationPayload>),
    /// OSC 7 — working directory URI (typically `file://…`).
    WorkingDirectory(Box<PresentationPayload>),
    /// OSC 8 — hyperlink start (`params` may contain `id=…`) or end when `uri` is empty.
    Hyperlink {
        id: Box<PresentationPayload>,
        uri: Box<PresentationPayload>,
    },
}

pub(crate) fn parse_osc_presentation(bytes: &[u8]) -> Option<HostPresentationEvent> {
    let (code, rest) = split_osc_code(bytes)?;
    match code {
        0 => Some(HostPresentationEvent::IconAndWindowTitle(Box::new(
            PresentationPayload::from_slice(rest),
        ))),
        2 => Some(HostPresentationEvent::WindowTitle(Box::new(
            PresentationPayload::from_slice(rest),
        ))),
        7 => Some(HostPresentationEvent::WorkingDirectory(Box::new(
            PresentationPayload::from_slice(rest),
        ))),
        8 => parse_osc8(rest),
        _ => None,
    }
}

fn split_osc_code(bytes: &[u8]) -> Option<(u16, &[u8])> {
    let mut value = 0u32;
    let mut idx = 0usize;
    while idx < bytes.len() {
        match bytes[idx] {
            b'0'..=b'9' => {
                value = value
                    .saturating_mul(10)
                    .saturating_add(u32::from(bytes[idx] - b'0'));
                if value > u32::from(u16::MAX) {
                    return None;
                }
                idx += 1;
            }
            b';' => {
                return Some((value as u16, &bytes[idx + 1..]));
            }
            _ => return None,
        }
    }
    // OSC with only a numeric code and no `;` payload.
    if idx == 0 {
        None
    } else {
        Some((value as u16, &[]))
    }
}

fn parse_osc8(rest: &[u8]) -> Option<HostPresentationEvent> {
    // OSC 8 ; params ; uri
    let semi = rest.iter().position(|b| *b == b';')?;
    let params = &rest[..semi];
    let uri = &rest[semi + 1..];
    let id = extract_hyperlink_id(params);
    Some(HostPresentationEvent::Hyperlink {
        id: Box::new(id),
        uri: Box::new(PresentationPayload::from_slice(uri)),
    })
}

fn extract_hyperlink_id(params: &[u8]) -> PresentationPayload {
    for part in params.split(|b| *b == b':') {
        if let Some(value) = part.strip_prefix(b"id=") {
            return PresentationPayload::from_slice(value);
        }
    }
    PresentationPayload::from_slice(&[])
}
