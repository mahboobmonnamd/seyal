#![no_main]

use libfuzzer_sys::fuzz_target;

#[cfg(target_os = "macos")]
use seyal_runtime::{
    AttachmentId, ExecutionId,
    local_ipc::{
        attachment::AttachmentRegistry,
        connection::ConnectionState,
        framing::{MessageType, Role},
    },
};

#[cfg(target_os = "macos")]
#[derive(Clone, Copy)]
struct ClientState {
    token: u64,
    state: ConnectionState,
    attachment: Option<AttachmentId>,
}

#[cfg(target_os = "macos")]
impl ClientState {
    const fn new(token: u64) -> Self {
        Self {
            token,
            state: ConnectionState::AwaitHello,
            attachment: None,
        }
    }
}

#[cfg(target_os = "macos")]
fn forged_attachment(seed: u8) -> AttachmentId {
    AttachmentId::from_bytes((0xf00d_0000u128 | seed as u128).to_le_bytes())
}

#[cfg(target_os = "macos")]
fn fuzz_state_machine(data: &[u8]) {
    let execution = ExecutionId::from_bytes(1u128.to_le_bytes());
    let mut registry = AttachmentRegistry::new();
    let mut clients = [ClientState::new(1), ClientState::new(2)];
    let mut next_token = 3u64;

    for operation in data.chunks(4).take(128) {
        if operation.len() < 4 {
            break;
        }
        let index = (operation[0] as usize) & 1;
        let client = &mut clients[index];

        match operation[1] % 8 {
            // Hello -> Ready.
            0 => {
                if client
                    .state
                    .validate_incoming(MessageType::ClientHello)
                    .is_ok()
                {
                    client.state = ConnectionState::Ready;
                }
            }
            // Observer/controller attach. Controller exclusivity and capacity
            // are enforced by the production AttachmentRegistry.
            1 => {
                if client.state.validate_incoming(MessageType::Attach).is_ok() {
                    let role = if operation[2] & 1 == 0 {
                        Role::Observer
                    } else {
                        Role::Controller
                    };
                    if let Ok(id) = registry.create_attachment(execution, role, client.token) {
                        client.attachment = Some(id);
                        client.state = ConnectionState::Attached;
                    }
                }
            }
            // Resync identity validation, including stale/cross-connection ids.
            2 => {
                if client.state.validate_incoming(MessageType::Resync).is_ok() {
                    let id = if operation[2] & 1 == 0 {
                        client
                            .attachment
                            .unwrap_or_else(|| forged_attachment(operation[3]))
                    } else {
                        forged_attachment(operation[3])
                    };
                    let _ = registry.execution_for_connection(client.token, id);
                }
            }
            // Detach/reattach boundary.
            3 => {
                if client.state.validate_incoming(MessageType::Detach).is_ok() {
                    let id = if operation[2] & 1 == 0 {
                        client
                            .attachment
                            .unwrap_or_else(|| forged_attachment(operation[3]))
                    } else {
                        forged_attachment(operation[3])
                    };
                    if registry.detach_for_connection(client.token, id).is_ok() {
                        if client.attachment == Some(id) {
                            client.attachment = None;
                            client.state = ConnectionState::Ready;
                        }
                    }
                }
            }
            // Mutation authority. Observers and stale identities must remain
            // rejected by the same production authorization routine.
            4 => {
                if client.state.validate_incoming(MessageType::Input).is_ok() {
                    let id = if operation[2] & 1 == 0 {
                        client
                            .attachment
                            .unwrap_or_else(|| forged_attachment(operation[3]))
                    } else {
                        forged_attachment(operation[3])
                    };
                    let _ = registry.authorize_mutation(client.token, id);
                }
            }
            // Disconnect/reconnect: release the old attachment and assign a new
            // connection token so old identities cannot acquire authority.
            5 => {
                if let Some(id) = client.attachment.take() {
                    let _ = registry.detach_for_connection(client.token, id);
                }
                client.token = next_token;
                next_token = next_token.wrapping_add(1).max(3);
                client.state = ConnectionState::AwaitHello;
            }
            // Execution finalization invalidates every outstanding attachment.
            6 => {
                registry.remove_all_for_execution(execution);
                for state in &mut clients {
                    state.attachment = None;
                    if state.state == ConnectionState::Attached {
                        state.state = ConnectionState::Ready;
                    }
                }
            }
            // Invalid-state traffic is intentionally validated but never
            // transitions the harness itself.
            _ => {
                let kind = match operation[2] % 5 {
                    0 => MessageType::ClientHello,
                    1 => MessageType::Attach,
                    2 => MessageType::Resync,
                    3 => MessageType::Detach,
                    _ => MessageType::Resize,
                };
                let _ = client.state.validate_incoming(kind);
            }
        }

        // Authority invariants are queried from production state, not mirrored
        // in the fuzz harness.
        for state in &clients {
            if let Some(id) = state.attachment {
                assert_eq!(
                    registry.execution_for_connection(state.token, id),
                    Ok(execution)
                );
            }
        }
    }
}

fuzz_target!(|data: &[u8]| {
    #[cfg(target_os = "macos")]
    fuzz_state_machine(data);

    #[cfg(not(target_os = "macos"))]
    let _ = data;
});
