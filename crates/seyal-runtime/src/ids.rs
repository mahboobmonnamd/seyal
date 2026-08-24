use std::{
    fmt,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);
const DEFAULT_WORKSPACE: u128 = 0x5345_5941_4c2d_4d30_3031_2d57_4f52_4b01;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RuntimeId(u128);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WorkspaceId(u128);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ExecutionId(u128);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AttachmentId(u128);

impl RuntimeId {
    pub(crate) fn new() -> Self {
        Self(unique_id(0x5255_4e54_494d_4501))
    }
}

impl WorkspaceId {
    /// The M001 implicit Workspace has one stable semantic identity inside each
    /// local-user scope. The enclosing singleton/user scope disambiguates users;
    /// this value deliberately does not derive from RuntimeId, cwd or UI state.
    pub const fn m001_default() -> Self {
        Self(DEFAULT_WORKSPACE)
    }
}

impl ExecutionId {
    pub(crate) fn new() -> Self {
        Self(unique_id(0x4558_4543_5554_4501))
    }
}

impl AttachmentId {
    pub(crate) fn new() -> Self {
        Self(unique_id(0x4154_5441_4348_0001))
    }
}

fn unique_id(domain: u64) -> u128 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = NEXT_ID.fetch_add(1, Ordering::Relaxed) as u128;
    let pid = std::process::id() as u128;
    let value = nanos.rotate_left(29)
        ^ (sequence << 32)
        ^ (pid << 80)
        ^ ((domain as u128) << 48)
        ^ domain as u128;
    value.max(1)
}

macro_rules! impl_id_display {
    ($type:ty) => {
        impl fmt::Display for $type {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{:032x}", self.0)
            }
        }
    };
}

impl_id_display!(RuntimeId);
impl_id_display!(WorkspaceId);
impl_id_display!(ExecutionId);
impl_id_display!(AttachmentId);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_and_execution_ids_do_not_reuse_process_local_counter_values() {
        let first_runtime = RuntimeId::new();
        let second_runtime = RuntimeId::new();
        let first_execution = ExecutionId::new();
        let second_execution = ExecutionId::new();
        assert_ne!(first_runtime, second_runtime);
        assert_ne!(first_execution, second_execution);
        assert_ne!(first_runtime.to_string(), first_execution.to_string());
    }

    #[test]
    fn default_workspace_identity_is_runtime_independent() {
        let before = WorkspaceId::m001_default();
        let _runtime_a = RuntimeId::new();
        let _runtime_b = RuntimeId::new();
        assert_eq!(before, WorkspaceId::m001_default());
    }
}
