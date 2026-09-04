use std::{env, fs, path::PathBuf};

use seyal_protocol::framing::{
    BlockTimeline, ComposerCommandRef, ComposerResult, ComposerStatus, FrameHeader, HEADER_LEN,
    HistoryRangeRequest, HistoryRangeSnapshot, MessageType, ResizeRequest, ResizeResult,
    TerminalKey, decode_message,
};

fn input() -> Vec<u8> {
    let path =
        PathBuf::from(env::var_os("SEYAL_FUZZ_INPUT").expect("SEYAL_FUZZ_INPUT is required"));
    fs::read(path).expect("read retained Pass 7 fuzz seed")
}

fn decode_pass7_payload(kind: MessageType, payload: &[u8]) {
    match kind {
        MessageType::TerminalKey => {
            let _ = TerminalKey::decode(payload);
        }
        MessageType::ResizeRequest => {
            let _ = ResizeRequest::decode(payload);
        }
        MessageType::ResizeResult => {
            let _ = ResizeResult::decode(payload);
        }
        MessageType::ComposerCommand => {
            let _ = ComposerCommandRef::decode(payload);
        }
        MessageType::ComposerResult => {
            let _ = ComposerResult::decode(payload);
        }
        MessageType::ComposerStatus => {
            let _ = ComposerStatus::decode(payload);
        }
        MessageType::BlockTimeline => {
            let _ = BlockTimeline::decode(payload);
        }
        MessageType::HistoryRangeRequest => {
            let _ = HistoryRangeRequest::decode(payload);
        }
        MessageType::HistoryRangeSnapshot => {
            let _ = HistoryRangeSnapshot::decode(payload);
        }
        _ => {}
    }
}

#[test]
#[ignore = "executed by fuzz/targets/pass7-protocol-decode with retained seeds"]
fn pass7_protocol_decode_seed() {
    let bytes = input();
    if bytes.len() >= HEADER_LEN
        && let Ok(header) = FrameHeader::decode(&bytes[..HEADER_LEN])
    {
        let end = HEADER_LEN.saturating_add(header.payload_len as usize);
        if let Some(payload) = bytes.get(HEADER_LEN..end.min(bytes.len()))
            && payload.len() == header.payload_len as usize
        {
            let _ = decode_message(&header, payload);
            if let Some(kind) = MessageType::from_u16(header.message_type) {
                decode_pass7_payload(kind, payload);
            }
        }
    }

    decode_pass7_payload(MessageType::TerminalKey, &bytes);
    decode_pass7_payload(MessageType::ResizeRequest, &bytes);
    decode_pass7_payload(MessageType::ResizeResult, &bytes);
    decode_pass7_payload(MessageType::ComposerCommand, &bytes);
    decode_pass7_payload(MessageType::ComposerResult, &bytes);
    decode_pass7_payload(MessageType::ComposerStatus, &bytes);
    decode_pass7_payload(MessageType::BlockTimeline, &bytes);
    decode_pass7_payload(MessageType::HistoryRangeRequest, &bytes);
    decode_pass7_payload(MessageType::HistoryRangeSnapshot, &bytes);
}
