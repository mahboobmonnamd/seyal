#![no_main]

use libfuzzer_sys::fuzz_target;
use seyal_runtime::{
    display,
    local_ipc::framing::{decode_message, FrameHeader, MessageType, HEADER_LEN},
};

const MAX_FRAMES_PER_INPUT: usize = 64;

fuzz_target!(|data: &[u8]| {
    // Exercise the exact production envelope and payload decoders. A single
    // fuzz input may contain concatenated frames; a trailing partial frame is
    // deliberately presented to the production decoder as truncated input.
    let mut offset = 0usize;
    let mut decoded = 0usize;

    while offset < data.len() && decoded < MAX_FRAMES_PER_INPUT {
        let remaining = &data[offset..];
        let header = match FrameHeader::decode(remaining) {
            Ok(header) => header,
            Err(_) => break,
        };

        let Some(total) = HEADER_LEN.checked_add(header.payload_len as usize) else {
            break;
        };
        if remaining.len() < total {
            let partial_payload = remaining.get(HEADER_LEN..).unwrap_or_default();
            let _ = decode_message(&header, partial_payload);
            break;
        }

        let payload = &remaining[HEADER_LEN..total];
        let _ = decode_message(&header, payload);

        // Display frames share the same production envelope. Keep their deep
        // payload validation on the existing display decoder rather than
        // inventing a second interpretation here.
        if matches!(
            MessageType::from_u16(header.message_type),
            Some(MessageType::DisplaySnapshot | MessageType::DisplayDelta)
        ) {
            let _ = display::decode_chunk(&remaining[..total]);
        }

        offset += total;
        decoded += 1;
    }
});
