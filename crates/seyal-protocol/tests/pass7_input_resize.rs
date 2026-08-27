use seyal_protocol::{
    AttachmentId,
    framing::{
        CAP_CORRELATED_RESIZE, CAP_SEMANTIC_TERMINAL_KEY, ErrorCode, FramingError, Message,
        MessageType, ResizeRequest, ResizeResult, ResizeResultCode, TerminalKey, TerminalKeyKind,
        TerminalKeyModifiers, decode_message, FrameHeader,
    },
};

fn attachment_id() -> AttachmentId {
    AttachmentId::from_bytes(0x112233445566778899aabbccddeeff00u128.to_le_bytes())
}

#[test]
fn pass7_capabilities_and_message_ids_are_stable() {
    assert_eq!(CAP_SEMANTIC_TERMINAL_KEY, 1 << 2);
    assert_eq!(CAP_CORRELATED_RESIZE, 1 << 3);
    assert_eq!(MessageType::from_u16(17), Some(MessageType::TerminalKey));
    assert_eq!(MessageType::from_u16(18), Some(MessageType::ResizeRequest));
    assert_eq!(MessageType::from_u16(19), Some(MessageType::ResizeResult));
}

#[test]
fn terminal_key_wire_layout_is_exact_and_round_trips() {
    let key = TerminalKey {
        attachment_id: attachment_id(),
        kind: TerminalKeyKind::ControlAscii,
        modifiers: TerminalKeyModifiers::CONTROL,
        scalar: b'?' as u32,
    };
    let encoded = key.encode();
    assert_eq!(encoded.len(), 24);
    assert_eq!(&encoded[0..16], &attachment_id().to_bytes());
    assert_eq!(u16::from_le_bytes(encoded[16..18].try_into().unwrap()), 9);
    assert_eq!(u16::from_le_bytes(encoded[18..20].try_into().unwrap()), 1);
    assert_eq!(u32::from_le_bytes(encoded[20..24].try_into().unwrap()), b'?' as u32);
    assert_eq!(TerminalKey::decode(&encoded).unwrap(), key);

    let header = FrameHeader::new(MessageType::TerminalKey as u16, encoded.len() as u32);
    assert_eq!(decode_message(&header, &encoded).unwrap(), Message::TerminalKey(key));
}

#[test]
fn terminal_key_rejects_invalid_m001_combinations() {
    let mut arrow = TerminalKey {
        attachment_id: attachment_id(),
        kind: TerminalKeyKind::ArrowUp,
        modifiers: TerminalKeyModifiers::NONE,
        scalar: 0,
    }
    .encode();
    arrow[18..20].copy_from_slice(&1u16.to_le_bytes());
    assert_eq!(TerminalKey::decode(&arrow), Err(FramingError::MalformedPayload));

    let mut control = TerminalKey {
        attachment_id: attachment_id(),
        kind: TerminalKeyKind::ControlAscii,
        modifiers: TerminalKeyModifiers::CONTROL,
        scalar: b'A' as u32,
    }
    .encode();
    control[20..24].copy_from_slice(&('é' as u32).to_le_bytes());
    assert_eq!(TerminalKey::decode(&control), Err(FramingError::MalformedPayload));

    let mut unknown = control;
    unknown[16..18].copy_from_slice(&99u16.to_le_bytes());
    assert_eq!(TerminalKey::decode(&unknown), Err(FramingError::MalformedPayload));
}

#[test]
fn resize_request_wire_layout_is_exact_and_validates_identity_and_geometry() {
    let request = ResizeRequest {
        attachment_id: attachment_id(),
        request_id: 42,
        rows: 40,
        columns: 120,
    };
    let encoded = request.encode();
    assert_eq!(encoded.len(), 32);
    assert_eq!(&encoded[0..16], &attachment_id().to_bytes());
    assert_eq!(u64::from_le_bytes(encoded[16..24].try_into().unwrap()), 42);
    assert_eq!(u16::from_le_bytes(encoded[24..26].try_into().unwrap()), 40);
    assert_eq!(u16::from_le_bytes(encoded[26..28].try_into().unwrap()), 120);
    assert_eq!(u32::from_le_bytes(encoded[28..32].try_into().unwrap()), 0);
    assert_eq!(ResizeRequest::decode(&encoded).unwrap(), request);

    let mut zero_id = encoded.clone();
    zero_id[16..24].copy_from_slice(&0u64.to_le_bytes());
    assert_eq!(ResizeRequest::decode(&zero_id), Err(FramingError::MalformedPayload));

    let mut reserved = encoded.clone();
    reserved[28..32].copy_from_slice(&1u32.to_le_bytes());
    assert_eq!(ResizeRequest::decode(&reserved), Err(FramingError::MalformedPayload));
}

#[test]
fn resize_result_carries_applied_generation_and_is_exactly_correlated() {
    let applied = ResizeResult {
        attachment_id: attachment_id(),
        request_id: 42,
        result_code: ResizeResultCode::Applied,
        detail_code: 0,
        applied_generation: 77,
    };
    let encoded = applied.encode();
    assert_eq!(encoded.len(), 40);
    assert_eq!(u64::from_le_bytes(encoded[16..24].try_into().unwrap()), 42);
    assert_eq!(u16::from_le_bytes(encoded[24..26].try_into().unwrap()), 0);
    assert_eq!(u16::from_le_bytes(encoded[26..28].try_into().unwrap()), 0);
    assert_eq!(u32::from_le_bytes(encoded[28..32].try_into().unwrap()), 0);
    assert_eq!(u64::from_le_bytes(encoded[32..40].try_into().unwrap()), 77);
    assert_eq!(ResizeResult::decode(&encoded).unwrap(), applied);

    let failure = ResizeResult {
        attachment_id: attachment_id(),
        request_id: 43,
        result_code: ResizeResultCode::Error(ErrorCode::InternalFailure),
        detail_code: 0,
        applied_generation: 0,
    };
    assert_eq!(ResizeResult::decode(&failure.encode()).unwrap(), failure);
}

#[test]
fn resize_result_rejects_ambiguous_success_and_failure_shapes() {
    let mut applied_without_generation = ResizeResult {
        attachment_id: attachment_id(),
        request_id: 1,
        result_code: ResizeResultCode::Applied,
        detail_code: 0,
        applied_generation: 1,
    }
    .encode();
    applied_without_generation[32..40].copy_from_slice(&0u64.to_le_bytes());
    assert_eq!(
        ResizeResult::decode(&applied_without_generation),
        Err(FramingError::MalformedPayload)
    );

    let mut failed_with_generation = ResizeResult {
        attachment_id: attachment_id(),
        request_id: 2,
        result_code: ResizeResultCode::Error(ErrorCode::InternalFailure),
        detail_code: 0,
        applied_generation: 0,
    }
    .encode();
    failed_with_generation[32..40].copy_from_slice(&9u64.to_le_bytes());
    assert_eq!(
        ResizeResult::decode(&failed_with_generation),
        Err(FramingError::MalformedPayload)
    );

    let mut unknown_code = failed_with_generation;
    unknown_code[24..26].copy_from_slice(&99u16.to_le_bytes());
    unknown_code[32..40].copy_from_slice(&0u64.to_le_bytes());
    assert_eq!(ResizeResult::decode(&unknown_code), Err(FramingError::MalformedPayload));
}
