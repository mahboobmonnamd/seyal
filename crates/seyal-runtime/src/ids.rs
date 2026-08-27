use std::{
    fmt,
    sync::{
        OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);
static PROCESS_ID_PREFIX: OnceLock<u64> = OnceLock::new();
const DEFAULT_WORKSPACE: u128 = 0x5345_5941_4c2d_4d30_3031_2d57_4f52_4b01;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RuntimeId(u128);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WorkspaceId(u128);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ExecutionId(u128);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AttachmentId(u128);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ProjectionId(u128);

impl RuntimeId {
    pub(crate) fn new() -> Self {
        Self(unique_id(0x5255_4e54_494d_4501))
    }
}

impl WorkspaceId {
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

macro_rules! impl_id_wire_bytes {
    ($type:ty) => {
        impl $type {
            pub fn to_bytes(self) -> [u8; 16] {
                self.0.to_le_bytes()
            }

            pub fn from_bytes(bytes: [u8; 16]) -> Self {
                Self(u128::from_le_bytes(bytes))
            }
        }
    };
}

impl_id_wire_bytes!(RuntimeId);
impl_id_wire_bytes!(ExecutionId);
impl_id_wire_bytes!(AttachmentId);
impl_id_wire_bytes!(ProjectionId);

fn unique_id(domain: u64) -> u128 {
    // Keep the monotonic sequence in a disjoint half of the identifier. The
    // previous XOR composition allowed timestamp changes to cancel sequence
    // changes and could therefore collide during rapid attachment creation.
    // A single process-global sequence now makes same-process uniqueness an
    // invariant rather than a probabilistic property.
    let sequence = NEXT_ID
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .expect("Seyal process-local identifier sequence exhausted");
    compose_unique_id(process_id_prefix(), domain, sequence)
}

fn process_id_prefix() -> u64 {
    *PROCESS_ID_PREFIX.get_or_init(|| {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let low = nanos as u64;
        let high = (nanos >> 64) as u64;
        let pid = std::process::id() as u64;
        let address = (&NEXT_ID as *const AtomicU64 as usize) as u64;
        mix64(low ^ high.rotate_left(17) ^ pid.rotate_left(31) ^ address)
    })
}

fn compose_unique_id(process_prefix: u64, domain: u64, sequence: u64) -> u128 {
    let namespace = mix64(process_prefix ^ domain);
    ((namespace as u128) << 64) | sequence as u128
}

fn mix64(mut value: u64) -> u64 {
    // SplitMix64 finalizer: cheap avalanche for the once-per-process prefix and
    // domain namespace. It is not used as the uniqueness mechanism; the
    // disjoint monotonic low 64 bits provide that guarantee within a process.
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
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
impl_id_display!(ProjectionId);

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

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
    fn composition_keeps_sequence_changes_disjoint_from_prefix_changes() {
        let prefix = 0x0123_4567_89ab_cdef;
        let domain = 0x4154_5441_4348_0001;
        let first = compose_unique_id(prefix, domain, 1);
        let second = compose_unique_id(prefix, domain, 2);
        assert_ne!(first, second);
        assert_eq!(first as u64, 1);
        assert_eq!(second as u64, 2);
    }

    #[test]
    fn rapid_attachment_ids_are_unique() {
        let mut seen = HashSet::with_capacity(100_000);
        for _ in 0..100_000 {
            assert!(seen.insert(AttachmentId::new()));
        }
    }

    #[test]
    fn default_workspace_identity_is_runtime_independent() {
        let before = WorkspaceId::m001_default();
        let _runtime_a = RuntimeId::new();
        let _runtime_b = RuntimeId::new();
        assert_eq!(before, WorkspaceId::m001_default());
    }

    #[test]
    fn wire_ids_round_trip_through_raw_little_endian_bytes() {
        let runtime = RuntimeId::new();
        let execution = ExecutionId::new();
        let attachment = AttachmentId::new();
        let projection = ProjectionId::from_bytes([0x5a; 16]);
        assert_eq!(RuntimeId::from_bytes(runtime.to_bytes()), runtime);
        assert_eq!(ExecutionId::from_bytes(execution.to_bytes()), execution);
        assert_eq!(AttachmentId::from_bytes(attachment.to_bytes()), attachment);
        assert_eq!(ProjectionId::from_bytes(projection.to_bytes()), projection);
    }
}
