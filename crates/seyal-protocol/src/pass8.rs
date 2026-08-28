//! M001 Pass 8 Block metadata value contract from SPEC-007.
//!
//! This module owns only fixed-size wire/value validation. It does not own
//! Workspace Block lifecycle, terminal state, authorization, transport, or UI.

use crate::{BlockId, ExecutionId};

/// SPEC-007 capability bit for the read-only Block metadata projection.
pub const CAP_BLOCK_METADATA: u32 = 1 << 4;
/// SPEC-007 R→C message type allocated to `BlockState`.
pub const BLOCK_STATE_MESSAGE_TYPE: u16 = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum BlockKind {
    TerminalActivity = 1,
}

impl BlockKind {
    fn from_u8(value: u8) -> Result<Self, BlockStateError> {
        match value {
            1 => Ok(Self::TerminalActivity),
            _ => Err(BlockStateError::UnknownKind),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum BlockLifecycle {
    Current = 1,
    Completed = 2,
}

impl BlockLifecycle {
    fn from_u8(value: u8) -> Result<Self, BlockStateError> {
        match value {
            1 => Ok(Self::Current),
            2 => Ok(Self::Completed),
            _ => Err(BlockStateError::UnknownState),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockStateError {
    ExactLengthMismatch,
    ZeroExecutionId,
    ZeroBlockId,
    ZeroRevision,
    ZeroStartLineId,
    UnknownKind,
    UnknownState,
    NonzeroReserved,
}

/// Fixed 56-byte read-only Block metadata payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockState {
    pub execution_id: ExecutionId,
    pub block_id: BlockId,
    pub revision: u64,
    pub start_line_id: u64,
    pub kind: BlockKind,
    pub state: BlockLifecycle,
}

impl BlockState {
    pub const WIRE_LEN: usize = 56;

    pub fn encode(&self) -> Result<[u8; Self::WIRE_LEN], BlockStateError> {
        validate_nonzero(self)?;
        let mut out = [0u8; Self::WIRE_LEN];
        out[0..16].copy_from_slice(&self.execution_id.to_bytes());
        out[16..32].copy_from_slice(&self.block_id.to_bytes());
        out[32..40].copy_from_slice(&self.revision.to_le_bytes());
        out[40..48].copy_from_slice(&self.start_line_id.to_le_bytes());
        out[48] = self.kind as u8;
        out[49] = self.state as u8;
        // bytes 50..56 are SPEC-007 reserved0/reserved1 and remain zero.
        Ok(out)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, BlockStateError> {
        if bytes.len() != Self::WIRE_LEN {
            return Err(BlockStateError::ExactLengthMismatch);
        }
        if bytes[50..56] != [0u8; 6] {
            return Err(BlockStateError::NonzeroReserved);
        }

        let mut execution_bytes = [0u8; 16];
        execution_bytes.copy_from_slice(&bytes[0..16]);
        let mut block_bytes = [0u8; 16];
        block_bytes.copy_from_slice(&bytes[16..32]);
        let value = Self {
            execution_id: ExecutionId::from_bytes(execution_bytes),
            block_id: BlockId::from_bytes(block_bytes),
            revision: u64::from_le_bytes(bytes[32..40].try_into().unwrap()),
            start_line_id: u64::from_le_bytes(bytes[40..48].try_into().unwrap()),
            kind: BlockKind::from_u8(bytes[48])?,
            state: BlockLifecycle::from_u8(bytes[49])?,
        };
        validate_nonzero(&value)?;
        Ok(value)
    }
}

fn validate_nonzero(value: &BlockState) -> Result<(), BlockStateError> {
    if value.execution_id.to_bytes() == [0u8; 16] {
        return Err(BlockStateError::ZeroExecutionId);
    }
    if value.block_id.to_bytes() == [0u8; 16] {
        return Err(BlockStateError::ZeroBlockId);
    }
    if value.revision == 0 {
        return Err(BlockStateError::ZeroRevision);
    }
    if value.start_line_id == 0 {
        return Err(BlockStateError::ZeroStartLineId);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn current() -> BlockState {
        BlockState {
            execution_id: ExecutionId::from_bytes(1u128.to_le_bytes()),
            block_id: BlockId::from_bytes(2u128.to_le_bytes()),
            revision: 1,
            start_line_id: 9,
            kind: BlockKind::TerminalActivity,
            state: BlockLifecycle::Current,
        }
    }

    #[test]
    fn block_state_is_exactly_56_bytes_and_round_trips_little_endian() {
        let value = current();
        let encoded = value.encode().unwrap();
        assert_eq!(encoded.len(), BlockState::WIRE_LEN);
        assert_eq!(&encoded[0..16], &1u128.to_le_bytes());
        assert_eq!(&encoded[16..32], &2u128.to_le_bytes());
        assert_eq!(&encoded[32..40], &1u64.to_le_bytes());
        assert_eq!(&encoded[40..48], &9u64.to_le_bytes());
        assert_eq!(encoded[48], 1);
        assert_eq!(encoded[49], 1);
        assert_eq!(&encoded[50..56], &[0u8; 6]);
        assert_eq!(BlockState::decode(&encoded).unwrap(), value);
    }

    #[test]
    fn completed_state_uses_revision_two_without_changing_identity_or_anchor() {
        let mut value = current();
        value.revision = 2;
        value.state = BlockLifecycle::Completed;
        assert_eq!(BlockState::decode(&value.encode().unwrap()).unwrap(), value);
    }

    #[test]
    fn decoder_rejects_length_reserved_zero_identity_revision_anchor_and_unknown_enums() {
        let valid = current().encode().unwrap();
        assert_eq!(
            BlockState::decode(&valid[..55]),
            Err(BlockStateError::ExactLengthMismatch)
        );

        let mut reserved = valid;
        reserved[50] = 1;
        assert_eq!(
            BlockState::decode(&reserved),
            Err(BlockStateError::NonzeroReserved)
        );

        let mut zero_execution = valid;
        zero_execution[0..16].fill(0);
        assert_eq!(
            BlockState::decode(&zero_execution),
            Err(BlockStateError::ZeroExecutionId)
        );

        let mut zero_block = valid;
        zero_block[16..32].fill(0);
        assert_eq!(
            BlockState::decode(&zero_block),
            Err(BlockStateError::ZeroBlockId)
        );

        let mut zero_revision = valid;
        zero_revision[32..40].fill(0);
        assert_eq!(
            BlockState::decode(&zero_revision),
            Err(BlockStateError::ZeroRevision)
        );

        let mut zero_anchor = valid;
        zero_anchor[40..48].fill(0);
        assert_eq!(
            BlockState::decode(&zero_anchor),
            Err(BlockStateError::ZeroStartLineId)
        );

        let mut unknown_kind = valid;
        unknown_kind[48] = 2;
        assert_eq!(
            BlockState::decode(&unknown_kind),
            Err(BlockStateError::UnknownKind)
        );

        let mut unknown_state = valid;
        unknown_state[49] = 3;
        assert_eq!(
            BlockState::decode(&unknown_state),
            Err(BlockStateError::UnknownState)
        );
    }

    #[test]
    fn capability_and_message_allocations_match_spec007() {
        assert_eq!(CAP_BLOCK_METADATA, 1 << 4);
        assert_eq!(BLOCK_STATE_MESSAGE_TYPE, 20);
    }
}