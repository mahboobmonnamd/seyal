#![cfg(target_os = "macos")]

use std::{env, fs, path::PathBuf};

use seyal_runtime::local_ipc::attachment::AttachmentRegistry;
use seyal_runtime::local_ipc::connection::ConnectionState;
use seyal_runtime::local_ipc::framing::{
    FrameHeader, HEADER_LEN, MessageType, Role, decode_message,
};
use seyal_runtime::projection::layout::{
    CELL_LEN, CellRecord, DAMAGE_LEN, DamageRecord, REGION_HEADER_LEN, RegionHeader,
    SLOT_HEADER_LEN, SlotHeader,
};
use seyal_runtime::{AttachmentId, ExecutionId, ProjectionId};

fn input() -> Vec<u8> {
    let path =
        PathBuf::from(env::var_os("SEYAL_FUZZ_INPUT").expect("SEYAL_FUZZ_INPUT is required"));
    fs::read(path).expect("read retained fuzz seed")
}

/// Activates the `local-binary-protocol-decode` target against the real
/// SPEC-004 wire decoder (`FrameHeader::decode` + `decode_message`), never
/// a no-op copy. A crash, panic or unsafe-arithmetic overflow on any byte
/// sequence fails the retained seed.
#[test]
#[ignore = "executed by fuzz/targets/local-binary-protocol-decode with a retained seed"]
fn local_binary_protocol_decode_seed() {
    let bytes = input();
    if bytes.len() < HEADER_LEN {
        return;
    }
    let Ok(header) = FrameHeader::decode(&bytes[..HEADER_LEN]) else {
        return;
    };
    let payload_end = HEADER_LEN.saturating_add(header.payload_len as usize);
    let Some(payload) = bytes.get(HEADER_LEN..payload_end.min(bytes.len())) else {
        return;
    };
    if payload.len() != header.payload_len as usize {
        return;
    }
    let _ = decode_message(&header, payload);
}

/// Activates the `shared-projection-validation` target against the real
/// SPEC-004 projection ABI validators (`RegionHeader`/`SlotHeader`/
/// `CellRecord`/`DamageRecord::decode`), never a no-op copy.
#[test]
#[ignore = "executed by fuzz/targets/shared-projection-validation with a retained seed"]
fn shared_projection_validation_seed() {
    let bytes = input();
    if bytes.len() >= REGION_HEADER_LEN {
        let _ = RegionHeader::decode(&bytes[..REGION_HEADER_LEN]);
    }
    if bytes.len() >= SLOT_HEADER_LEN {
        let _ = SlotHeader::decode(&bytes[..SLOT_HEADER_LEN], 256, 512);
    }
    for chunk in bytes.chunks_exact(CELL_LEN) {
        let _ = CellRecord::decode(chunk);
    }
    for chunk in bytes.chunks_exact(DAMAGE_LEN) {
        let _ = DamageRecord::decode(chunk, 256);
    }
}

/// Activates the `reconnect-resync-state-machine` target against the real
/// SPEC-004 connection state machine (`ConnectionState::validate_incoming`)
/// and attachment/authority registry (`AttachmentRegistry`), driving a
/// byte-derived sequence of attach/detach/resync/reconnect operations.
/// Never a no-op copy: every operation below calls into production
/// decision logic, not a fuzz-only reimplementation.
#[test]
#[ignore = "executed by fuzz/targets/reconnect-resync-state-machine with a retained seed"]
fn reconnect_resync_state_machine_seed() {
    let bytes = input();
    let mut registry = AttachmentRegistry::with_capacity(4);
    let mut state = ConnectionState::AwaitHello;
    let mut current_attachment: Option<AttachmentId> = None;
    let mut current_execution: Option<ExecutionId> = None;

    for chunk in bytes.chunks(3) {
        if chunk.len() < 3 {
            break;
        }
        let op = chunk[0] % 6;
        let execution_id = ExecutionId::from_bytes((chunk[1] as u128).to_le_bytes());
        let projection_id = ProjectionId::from_bytes((chunk[2] as u128).to_le_bytes());

        match op {
            0 => {
                let _ = state.validate_incoming(MessageType::ClientHello);
                state = ConnectionState::Ready;
            }
            1 => {
                if state.validate_incoming(MessageType::Attach).is_ok() {
                    let role = if chunk[2] % 2 == 0 {
                        Role::Observer
                    } else {
                        Role::Controller
                    };
                    if let Ok(id) = registry.create_attachment(execution_id, role, projection_id) {
                        current_attachment = Some(id);
                        current_execution = Some(execution_id);
                        state = ConnectionState::Attached;
                    }
                }
            }
            2 => {
                if let Some(id) = current_attachment
                    && state.validate_incoming(MessageType::Detach).is_ok()
                {
                    let _ = registry.detach(id);
                    current_attachment = None;
                    state = ConnectionState::Ready;
                }
            }
            3 => {
                if let Some(id) = current_attachment {
                    let _ = state.validate_incoming(MessageType::Resync);
                    let _ = registry.execution_of(id);
                    let _ = registry.is_live(id, projection_id);
                }
            }
            4 => {
                // Reconnect: this connection is gone; a fresh connection
                // for the same execution must be able to attach again.
                if let Some(id) = current_attachment {
                    let _ = registry.detach(id);
                }
                current_attachment = None;
                state = ConnectionState::AwaitHello;
                if let Some(execution_id) = current_execution {
                    let _ = registry.attachments_for_execution(execution_id);
                }
            }
            _ => {
                let _ = registry.has_controller(execution_id);
                let _ = registry.len();
            }
        }
    }

    // Invariants that must hold regardless of the fuzzed operation
    // sequence: the registry never exceeds its configured capacity, and
    // reaching this point at all means no operation above panicked.
    assert!(registry.len() <= 4);
}
