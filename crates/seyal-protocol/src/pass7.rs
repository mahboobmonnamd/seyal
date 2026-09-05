use crate::{
    AttachmentId,
    display::{MAX_DISPLAY_COLUMNS, MAX_DISPLAY_ROWS},
    framing::{ErrorCode, FramingError},
};

pub const MAX_COMPOSER_COMMAND_BYTES: usize = 16 * 1024;
// A full replacement timeline must fit in one bounded IPC frame even when
// every command reaches the 16 KiB admission limit. Larger histories are
// intentionally rejected at the Runtime boundary until a separately
// versioned continuation protocol exists.
pub const MAX_COMMAND_BLOCK_RECORDS: usize = 128;
/// History range requests are deliberately independent from BlockTimeline.
/// They carry only a bounded primary-screen projection and never change block
/// identity or lifecycle authority.
pub const MAX_HISTORY_RANGE_LINES: usize = 512;
pub const MAX_HISTORY_RANGE_CELLS: usize = 131_072;
pub const MAX_HISTORY_RANGE_BYTES: usize = 196_608;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ComposerEligibility {
    Available = 0,
    Busy = 1,
    Unsupported = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ComposerStatus {
    pub attachment_id: AttachmentId,
    pub eligibility: ComposerEligibility,
    pub revision: u64,
}

impl ComposerStatus {
    const WIRE_LEN: usize = 32;

    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(Self::WIRE_LEN);
        out.extend_from_slice(&self.attachment_id.to_bytes());
        out.push(self.eligibility as u8);
        out.extend_from_slice(&[0; 7]);
        out.extend_from_slice(&self.revision.to_le_bytes());
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, FramingError> {
        exact_len(bytes, Self::WIRE_LEN)?;
        if bytes[17..24] != [0; 7] {
            return Err(FramingError::MalformedPayload);
        }
        let eligibility = match bytes[16] {
            0 => ComposerEligibility::Available,
            1 => ComposerEligibility::Busy,
            2 => ComposerEligibility::Unsupported,
            _ => return Err(FramingError::MalformedPayload),
        };
        Ok(Self {
            attachment_id: attachment_id_from(&bytes[..16]),
            eligibility,
            revision: u64::from_le_bytes(bytes[24..32].try_into().unwrap()),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ComposerResultCode {
    Accepted = 0,
    Busy = 1,
    Unsupported = 2,
    Backpressure = 3,
    Invalid = 4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ComposerResult {
    pub attachment_id: AttachmentId,
    pub code: ComposerResultCode,
    pub block_id: u64,
    pub request_id: u64,
}

impl ComposerResult {
    const WIRE_LEN: usize = 40;

    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(Self::WIRE_LEN);
        out.extend_from_slice(&self.attachment_id.to_bytes());
        out.push(self.code as u8);
        out.extend_from_slice(&[0; 7]);
        out.extend_from_slice(&self.block_id.to_le_bytes());
        out.extend_from_slice(&self.request_id.to_le_bytes());
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, FramingError> {
        exact_len(bytes, Self::WIRE_LEN)?;
        if bytes[17..24] != [0; 7] {
            return Err(FramingError::MalformedPayload);
        }
        let code = match bytes[16] {
            0 => ComposerResultCode::Accepted,
            1 => ComposerResultCode::Busy,
            2 => ComposerResultCode::Unsupported,
            3 => ComposerResultCode::Backpressure,
            4 => ComposerResultCode::Invalid,
            _ => return Err(FramingError::MalformedPayload),
        };
        Ok(Self {
            attachment_id: attachment_id_from(&bytes[..16]),
            code,
            block_id: u64::from_le_bytes(bytes[24..32].try_into().unwrap()),
            request_id: u64::from_le_bytes(bytes[32..40].try_into().unwrap()),
        })
    }
}

/// A complete UTF-8 command committed from the unique Pane composer. The
/// borrowed command is bounded at the framing boundary and is never inferred
/// from terminal cells or prompts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ComposerCommandRef<'a> {
    pub attachment_id: AttachmentId,
    pub request_id: u64,
    pub command: &'a str,
}

impl<'a> ComposerCommandRef<'a> {
    const HEADER_LEN: usize = 32;

    pub fn encode(&self) -> Vec<u8> {
        let bytes = self.command.as_bytes();
        debug_assert!(!bytes.is_empty() && bytes.len() <= MAX_COMPOSER_COMMAND_BYTES);
        let mut out = Vec::with_capacity(Self::HEADER_LEN + bytes.len());
        out.extend_from_slice(&self.attachment_id.to_bytes());
        out.extend_from_slice(&self.request_id.to_le_bytes());
        out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(bytes);
        out
    }

    pub fn decode(bytes: &'a [u8]) -> Result<Self, FramingError> {
        if bytes.len() < Self::HEADER_LEN {
            return Err(FramingError::TruncatedPayload);
        }
        let request_id = u64::from_le_bytes(bytes[16..24].try_into().unwrap());
        let declared = u32::from_le_bytes(bytes[24..28].try_into().unwrap()) as usize;
        if request_id == 0
            || u32::from_le_bytes(bytes[28..32].try_into().unwrap()) != 0
            || declared == 0
            || declared > MAX_COMPOSER_COMMAND_BYTES
            || bytes.len() != Self::HEADER_LEN + declared
        {
            return Err(FramingError::MalformedPayload);
        }
        let command = std::str::from_utf8(&bytes[Self::HEADER_LEN..])
            .map_err(|_| FramingError::MalformedPayload)?;
        Ok(Self {
            attachment_id: attachment_id_from(&bytes[..16]),
            request_id,
            command,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandBlockState {
    Running,
    Completed { exit_status: i32 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandBlock {
    pub id: u64,
    pub command: String,
    pub start_line: u64,
    pub end_line: Option<u64>,
    pub state: CommandBlockState,
}

/// A bounded full replacement cache for one attached execution. A client must
/// atomically replace its disposable projection when this arrives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockTimeline {
    pub revision: u64,
    pub records: Vec<CommandBlock>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HistoryRangeRequest {
    pub attachment_id: AttachmentId,
    pub request_id: u64,
    pub block_id: u64,
    pub start_line: u64,
    pub end_line: u64,
    pub max_lines: u16,
    pub max_cells: u32,
}

impl HistoryRangeRequest {
    const WIRE_LEN: usize = 64;

    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(Self::WIRE_LEN);
        out.extend_from_slice(&self.attachment_id.to_bytes());
        out.extend_from_slice(&self.request_id.to_le_bytes());
        out.extend_from_slice(&self.block_id.to_le_bytes());
        out.extend_from_slice(&self.start_line.to_le_bytes());
        out.extend_from_slice(&self.end_line.to_le_bytes());
        out.extend_from_slice(&self.max_lines.to_le_bytes());
        out.extend_from_slice(&[0; 2]);
        out.extend_from_slice(&self.max_cells.to_le_bytes());
        out.extend_from_slice(&[0; 4]);
        out.extend_from_slice(&[0; 4]);
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, FramingError> {
        exact_len(bytes, Self::WIRE_LEN)?;
        let request_id = u64::from_le_bytes(bytes[16..24].try_into().unwrap());
        let block_id = u64::from_le_bytes(bytes[24..32].try_into().unwrap());
        let start_line = u64::from_le_bytes(bytes[32..40].try_into().unwrap());
        let end_line = u64::from_le_bytes(bytes[40..48].try_into().unwrap());
        if bytes[50..52] != [0; 2]
            || bytes[56..64] != [0; 8]
            || request_id == 0
            || block_id == 0
            || start_line == 0
            || end_line < start_line
        {
            return Err(FramingError::MalformedPayload);
        }
        let max_lines = u16::from_le_bytes(bytes[48..50].try_into().unwrap());
        let max_cells = u32::from_le_bytes(bytes[52..56].try_into().unwrap());
        if max_lines == 0
            || usize::from(max_lines) > MAX_HISTORY_RANGE_LINES
            || max_cells == 0
            || usize::try_from(max_cells).unwrap_or(usize::MAX) > MAX_HISTORY_RANGE_CELLS
        {
            return Err(FramingError::MalformedPayload);
        }
        Ok(Self {
            attachment_id: attachment_id_from(&bytes[..16]),
            request_id,
            block_id,
            start_line,
            end_line,
            max_lines,
            max_cells,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum HistoryRangeStatus {
    Complete = 0,
    Truncated = 1,
    Stale = 2,
    Unsupported = 3,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryRow {
    pub line_id: u64,
    pub cells: Vec<HistoryCell>,
}

/// Canonical terminal cells retain style as well as scalar. The packed colors
/// use the same tagged representation as `PreparedCell` and are resolved by
/// the native renderer, so the UI never reconstructs style from text.
///
/// Layout matches `SeyalHistoryCell` in `macos/Seyal/Sources/SeyalBridge.h`
/// (`reserved` is an explicit ABI field, not accidental padding).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct HistoryCell {
    pub scalar: u32,
    pub foreground: u32,
    pub background: u32,
    pub flags: u16,
    pub reserved: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryRangeSnapshot {
    pub request_id: u64,
    pub block_id: u64,
    pub revision: u64,
    pub status: HistoryRangeStatus,
    pub rows: Vec<HistoryRow>,
}

impl HistoryRangeSnapshot {
    const HEADER_LEN: usize = 32;

    pub fn try_encode(&self) -> Result<Vec<u8>, FramingError> {
        if self.rows.len() > MAX_HISTORY_RANGE_LINES
            || self.rows.iter().map(|row| row.cells.len()).sum::<usize>() > MAX_HISTORY_RANGE_CELLS
        {
            return Err(FramingError::OversizedPayload);
        }
        let mut out = Vec::with_capacity(Self::HEADER_LEN);
        out.extend_from_slice(&self.request_id.to_le_bytes());
        out.extend_from_slice(&self.block_id.to_le_bytes());
        out.extend_from_slice(&self.revision.to_le_bytes());
        out.push(self.status as u8);
        out.push(0);
        out.extend_from_slice(&(self.rows.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        for row in &self.rows {
            if row.line_id == 0 || row.cells.len() > u32::MAX as usize {
                return Err(FramingError::MalformedPayload);
            }
            out.extend_from_slice(&row.line_id.to_le_bytes());
            out.extend_from_slice(&(row.cells.len() as u32).to_le_bytes());
            out.extend_from_slice(&0u32.to_le_bytes());
            for cell in &row.cells {
                out.extend_from_slice(&cell.scalar.to_le_bytes());
                out.extend_from_slice(&cell.foreground.to_le_bytes());
                out.extend_from_slice(&cell.background.to_le_bytes());
                out.extend_from_slice(&cell.flags.to_le_bytes());
                out.extend_from_slice(&cell.reserved.to_le_bytes());
            }
            if out.len() > crate::framing::MAX_FRAME_PAYLOAD as usize
                || out.len() > MAX_HISTORY_RANGE_BYTES
            {
                return Err(FramingError::OversizedPayload);
            }
        }
        Ok(out)
    }

    pub fn encode(&self) -> Vec<u8> {
        self.try_encode().expect("bounded history snapshot")
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, FramingError> {
        if bytes.len() > crate::framing::MAX_FRAME_PAYLOAD as usize
            || bytes.len() > MAX_HISTORY_RANGE_BYTES
        {
            return Err(FramingError::OversizedPayload);
        }
        if bytes.len() < Self::HEADER_LEN || bytes[25] != 0 || bytes[28..32] != [0; 4] {
            return Err(FramingError::MalformedPayload);
        }
        let status = match bytes[24] {
            0 => HistoryRangeStatus::Complete,
            1 => HistoryRangeStatus::Truncated,
            2 => HistoryRangeStatus::Stale,
            3 => HistoryRangeStatus::Unsupported,
            _ => return Err(FramingError::MalformedPayload),
        };
        let count = u16::from_le_bytes(bytes[26..28].try_into().unwrap()) as usize;
        if count > MAX_HISTORY_RANGE_LINES {
            return Err(FramingError::MalformedPayload);
        }
        let mut offset = Self::HEADER_LEN;
        let mut total_cells = 0usize;
        let mut rows = Vec::with_capacity(count);
        for _ in 0..count {
            let end = offset
                .checked_add(16)
                .ok_or(FramingError::MalformedPayload)?;
            if end > bytes.len() {
                return Err(FramingError::TruncatedPayload);
            }
            let line_id = u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
            let cells =
                u32::from_le_bytes(bytes[offset + 8..offset + 12].try_into().unwrap()) as usize;
            if line_id == 0 || bytes[offset + 12..offset + 16] != [0; 4] {
                return Err(FramingError::MalformedPayload);
            }
            total_cells = total_cells
                .checked_add(cells)
                .ok_or(FramingError::OversizedPayload)?;
            if total_cells > MAX_HISTORY_RANGE_CELLS {
                return Err(FramingError::OversizedPayload);
            }
            let bytes_len = cells
                .checked_mul(16)
                .ok_or(FramingError::OversizedPayload)?;
            let cells_end = end
                .checked_add(bytes_len)
                .ok_or(FramingError::OversizedPayload)?;
            if cells_end > bytes.len() {
                return Err(FramingError::TruncatedPayload);
            }
            let (cell_chunks, remainder) = bytes[end..cells_end].as_chunks::<16>();
            if !remainder.is_empty() {
                return Err(FramingError::MalformedPayload);
            }
            let values = cell_chunks
                .iter()
                .map(|chunk| {
                    let reserved = u16::from_le_bytes(chunk[14..16].try_into().unwrap());
                    if reserved != 0 {
                        return Err(FramingError::MalformedPayload);
                    }
                    Ok(HistoryCell {
                        scalar: u32::from_le_bytes(chunk[..4].try_into().unwrap()),
                        foreground: u32::from_le_bytes(chunk[4..8].try_into().unwrap()),
                        background: u32::from_le_bytes(chunk[8..12].try_into().unwrap()),
                        flags: u16::from_le_bytes(chunk[12..14].try_into().unwrap()),
                        reserved,
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            rows.push(HistoryRow {
                line_id,
                cells: values,
            });
            offset = cells_end;
        }
        if offset != bytes.len() {
            return Err(FramingError::ExactLengthMismatch);
        }
        Ok(Self {
            request_id: u64::from_le_bytes(bytes[..8].try_into().unwrap()),
            block_id: u64::from_le_bytes(bytes[8..16].try_into().unwrap()),
            revision: u64::from_le_bytes(bytes[16..24].try_into().unwrap()),
            status,
            rows,
        })
    }
}

impl BlockTimeline {
    const HEADER_LEN: usize = 16;
    const RECORD_HEADER_LEN: usize = 36;

    pub fn try_encode(&self) -> Result<Vec<u8>, FramingError> {
        if self.records.len() > MAX_COMMAND_BLOCK_RECORDS {
            return Err(FramingError::OversizedPayload);
        }
        let mut out = Vec::with_capacity(Self::HEADER_LEN);
        out.extend_from_slice(&self.revision.to_le_bytes());
        out.extend_from_slice(&(self.records.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        for record in &self.records {
            let command = record.command.as_bytes();
            if command.is_empty() || command.len() > MAX_COMPOSER_COMMAND_BYTES {
                return Err(FramingError::MalformedPayload);
            }
            let (state, exit_status) = match record.state {
                CommandBlockState::Running => (0u8, 0i32),
                CommandBlockState::Completed { exit_status } => (1u8, exit_status),
            };
            out.extend_from_slice(&record.id.to_le_bytes());
            out.extend_from_slice(&record.start_line.to_le_bytes());
            out.extend_from_slice(&record.end_line.unwrap_or(0).to_le_bytes());
            out.push(state);
            out.extend_from_slice(&[0; 3]);
            out.extend_from_slice(&exit_status.to_le_bytes());
            out.extend_from_slice(&(command.len() as u16).to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(command);
            if out.len() > crate::framing::MAX_FRAME_PAYLOAD as usize {
                return Err(FramingError::OversizedPayload);
            }
        }
        Ok(out)
    }

    pub fn encode(&self) -> Vec<u8> {
        self.try_encode()
            .expect("BlockTimeline must satisfy bounded wire limits")
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, FramingError> {
        if bytes.len() > crate::framing::MAX_FRAME_PAYLOAD as usize {
            return Err(FramingError::OversizedPayload);
        }
        if bytes.len() < Self::HEADER_LEN {
            return Err(FramingError::TruncatedPayload);
        }
        let revision = u64::from_le_bytes(bytes[..8].try_into().unwrap());
        let count = u16::from_le_bytes(bytes[8..10].try_into().unwrap()) as usize;
        if u16::from_le_bytes(bytes[10..12].try_into().unwrap()) != 0
            || u32::from_le_bytes(bytes[12..16].try_into().unwrap()) != 0
            || count > MAX_COMMAND_BLOCK_RECORDS
        {
            return Err(FramingError::MalformedPayload);
        }
        let mut offset = Self::HEADER_LEN;
        let mut records = Vec::with_capacity(count);
        for _ in 0..count {
            let header_end = offset
                .checked_add(Self::RECORD_HEADER_LEN)
                .ok_or(FramingError::MalformedPayload)?;
            if header_end > bytes.len() {
                return Err(FramingError::TruncatedPayload);
            }
            let id = u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
            let start_line = u64::from_le_bytes(bytes[offset + 8..offset + 16].try_into().unwrap());
            let end_raw = u64::from_le_bytes(bytes[offset + 16..offset + 24].try_into().unwrap());
            let state_tag = bytes[offset + 24];
            if bytes[offset + 25..offset + 28] != [0; 3]
                || u16::from_le_bytes(bytes[offset + 30..offset + 32].try_into().unwrap()) != 0
            {
                return Err(FramingError::MalformedPayload);
            }
            let exit_status =
                i32::from_le_bytes(bytes[offset + 28..offset + 32].try_into().unwrap());
            let command_len =
                u16::from_le_bytes(bytes[offset + 32..offset + 34].try_into().unwrap()) as usize;
            // The record fixed prefix includes the command length/reserved fields.
            let command_start = offset + 36;
            if command_len == 0
                || command_len > MAX_COMPOSER_COMMAND_BYTES
                || u16::from_le_bytes(bytes[offset + 34..offset + 36].try_into().unwrap()) != 0
            {
                return Err(FramingError::MalformedPayload);
            }
            let command_end = command_start
                .checked_add(command_len)
                .ok_or(FramingError::MalformedPayload)?;
            if command_end > bytes.len() {
                return Err(FramingError::TruncatedPayload);
            }
            let command = std::str::from_utf8(&bytes[command_start..command_end])
                .map_err(|_| FramingError::MalformedPayload)?
                .to_owned();
            let (end_line, state) = match state_tag {
                0 if end_raw == 0 && exit_status == 0 => (None, CommandBlockState::Running),
                1 if end_raw >= start_line => {
                    (Some(end_raw), CommandBlockState::Completed { exit_status })
                }
                _ => return Err(FramingError::MalformedPayload),
            };
            if id == 0 || start_line == 0 {
                return Err(FramingError::MalformedPayload);
            }
            records.push(CommandBlock {
                id,
                command,
                start_line,
                end_line,
                state,
            });
            offset = command_end;
        }
        if offset != bytes.len() {
            return Err(FramingError::ExactLengthMismatch);
        }
        Ok(Self { revision, records })
    }
}

#[cfg(test)]
mod command_block_tests {
    use super::*;

    fn attachment() -> AttachmentId {
        AttachmentId::from_bytes(7u128.to_le_bytes())
    }

    #[test]
    fn composer_command_requires_exact_bounded_utf8_payload() {
        let request = ComposerCommandRef {
            attachment_id: attachment(),
            request_id: 3,
            command: "printf hello",
        };
        assert_eq!(ComposerCommandRef::decode(&request.encode()), Ok(request));
        let mut malformed = request.encode();
        malformed[24..28].copy_from_slice(&1u32.to_le_bytes());
        assert_eq!(
            ComposerCommandRef::decode(&malformed),
            Err(FramingError::MalformedPayload)
        );
    }

    #[test]
    fn timeline_round_trip_preserves_only_metadata_and_anchors() {
        let timeline = BlockTimeline {
            revision: 9,
            records: vec![
                CommandBlock {
                    id: 1,
                    command: "printf one".into(),
                    start_line: 31,
                    end_line: None,
                    state: CommandBlockState::Running,
                },
                CommandBlock {
                    id: 2,
                    command: "false".into(),
                    start_line: 34,
                    end_line: Some(36),
                    state: CommandBlockState::Completed { exit_status: 1 },
                },
            ],
        };
        assert_eq!(BlockTimeline::decode(&timeline.encode()), Ok(timeline));
    }

    #[test]
    fn timeline_try_encode_rejects_unbounded_record_count() {
        let record = CommandBlock {
            id: 1,
            command: "echo".into(),
            start_line: 1,
            end_line: None,
            state: CommandBlockState::Running,
        };
        let timeline = BlockTimeline {
            revision: 1,
            records: vec![record; MAX_COMMAND_BLOCK_RECORDS + 1],
        };
        assert_eq!(timeline.try_encode(), Err(FramingError::OversizedPayload));
    }

    #[test]
    fn composer_status_and_result_are_correlated_to_attachment() {
        let status = ComposerStatus {
            attachment_id: attachment(),
            eligibility: ComposerEligibility::Busy,
            revision: 12,
        };
        assert_eq!(ComposerStatus::decode(&status.encode()), Ok(status));
        let result = ComposerResult {
            attachment_id: attachment(),
            code: ComposerResultCode::Accepted,
            block_id: 44,
            request_id: 3,
        };
        assert_eq!(ComposerResult::decode(&result.encode()), Ok(result));
    }

    #[test]
    fn history_range_request_rejects_unbounded_or_reversed_ranges() {
        let request = HistoryRangeRequest {
            attachment_id: attachment(),
            request_id: 9,
            block_id: 44,
            start_line: 4,
            end_line: 8,
            max_lines: 32,
            max_cells: 4096,
        };
        assert_eq!(HistoryRangeRequest::decode(&request.encode()), Ok(request));
        let mut reversed = request.encode();
        reversed[40..48].copy_from_slice(&3u64.to_le_bytes());
        assert_eq!(
            HistoryRangeRequest::decode(&reversed),
            Err(FramingError::MalformedPayload)
        );
        let mut oversized = request.encode();
        oversized[48..50].copy_from_slice(&u16::MAX.to_le_bytes());
        assert_eq!(
            HistoryRangeRequest::decode(&oversized),
            Err(FramingError::MalformedPayload)
        );
    }

    #[test]
    fn history_snapshot_round_trip_is_bounded_and_keeps_rows_distinct() {
        let snapshot = HistoryRangeSnapshot {
            request_id: 9,
            block_id: 44,
            revision: 17,
            status: HistoryRangeStatus::Complete,
            rows: vec![
                HistoryRow {
                    line_id: 4,
                    cells: "café"
                        .chars()
                        .map(|c| HistoryCell {
                            scalar: c as u32,
                            foreground: 0x0200_00ff,
                            background: 0,
                            flags: 1,
                            reserved: 0,
                        })
                        .collect(),
                },
                HistoryRow {
                    line_id: 5,
                    cells: vec![
                        HistoryCell {
                            scalar: b' ' as u32,
                            foreground: 0,
                            background: 0,
                            flags: 0,
                            reserved: 0,
                        };
                        3
                    ],
                },
            ],
        };
        assert_eq!(
            HistoryRangeSnapshot::decode(&snapshot.encode()),
            Ok(snapshot)
        );
        let too_many = HistoryRangeSnapshot {
            request_id: 1,
            block_id: 1,
            revision: 1,
            status: HistoryRangeStatus::Complete,
            rows: vec![HistoryRow {
                line_id: 1,
                cells: vec![
                    HistoryCell {
                        scalar: 0,
                        foreground: 0,
                        background: 0,
                        flags: 0,
                        reserved: 0,
                    };
                    MAX_HISTORY_RANGE_CELLS + 1
                ],
            }],
        };
        assert_eq!(too_many.try_encode(), Err(FramingError::OversizedPayload));
    }

    #[test]
    fn history_snapshot_rejects_nonzero_cell_reserved() {
        let snapshot = HistoryRangeSnapshot {
            request_id: 1,
            block_id: 2,
            revision: 3,
            status: HistoryRangeStatus::Complete,
            rows: vec![HistoryRow {
                line_id: 1,
                cells: vec![HistoryCell {
                    scalar: b'x' as u32,
                    foreground: 0,
                    background: 0,
                    flags: 0,
                    reserved: 0,
                }],
            }],
        };
        let mut encoded = snapshot.encode();
        // scalar(4)+fg(4)+bg(4)+flags(2)+reserved(2) — flip reserved.
        let reserved_at = encoded.len() - 2;
        encoded[reserved_at] = 1;
        assert_eq!(
            HistoryRangeSnapshot::decode(&encoded),
            Err(FramingError::MalformedPayload)
        );
    }
}

pub const CAP_SEMANTIC_TERMINAL_KEY: u32 = 1 << 2;
pub const CAP_CORRELATED_RESIZE: u32 = 1 << 3;

fn exact_len(bytes: &[u8], expected: usize) -> Result<(), FramingError> {
    if bytes.len() != expected {
        return Err(FramingError::ExactLengthMismatch);
    }
    Ok(())
}

fn attachment_id_from(bytes: &[u8]) -> AttachmentId {
    let mut raw = [0u8; 16];
    raw.copy_from_slice(&bytes[..16]);
    AttachmentId::from_bytes(raw)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum TerminalKeyKind {
    Enter = 1,
    Tab = 2,
    Backspace = 3,
    Escape = 4,
    ArrowUp = 5,
    ArrowDown = 6,
    ArrowRight = 7,
    ArrowLeft = 8,
    ControlAscii = 9,
}

impl TerminalKeyKind {
    fn from_u16(value: u16) -> Result<Self, FramingError> {
        match value {
            1 => Ok(Self::Enter),
            2 => Ok(Self::Tab),
            3 => Ok(Self::Backspace),
            4 => Ok(Self::Escape),
            5 => Ok(Self::ArrowUp),
            6 => Ok(Self::ArrowDown),
            7 => Ok(Self::ArrowRight),
            8 => Ok(Self::ArrowLeft),
            9 => Ok(Self::ControlAscii),
            _ => Err(FramingError::MalformedPayload),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminalKeyModifiers(u16);

impl TerminalKeyModifiers {
    pub const NONE: Self = Self(0);
    pub const CONTROL: Self = Self(1 << 0);

    pub const fn bits(self) -> u16 {
        self.0
    }

    fn from_bits(bits: u16) -> Result<Self, FramingError> {
        match bits {
            0 => Ok(Self::NONE),
            1 => Ok(Self::CONTROL),
            _ => Err(FramingError::MalformedPayload),
        }
    }
}

fn valid_control_ascii_scalar(scalar: u32) -> bool {
    matches!(scalar, 0x20 | 0x3f | 0x40 | 0x41..=0x5f)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminalKey {
    pub attachment_id: AttachmentId,
    pub kind: TerminalKeyKind,
    pub modifiers: TerminalKeyModifiers,
    pub scalar: u32,
}

impl TerminalKey {
    pub const WIRE_LEN: usize = 24;

    pub fn encode(&self) -> Vec<u8> {
        debug_assert!(self.validate().is_ok());
        let mut out = Vec::with_capacity(Self::WIRE_LEN);
        out.extend_from_slice(&self.attachment_id.to_bytes());
        out.extend_from_slice(&(self.kind as u16).to_le_bytes());
        out.extend_from_slice(&self.modifiers.bits().to_le_bytes());
        out.extend_from_slice(&self.scalar.to_le_bytes());
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, FramingError> {
        exact_len(bytes, Self::WIRE_LEN)?;
        let value = Self {
            attachment_id: attachment_id_from(&bytes[..16]),
            kind: TerminalKeyKind::from_u16(u16::from_le_bytes(bytes[16..18].try_into().unwrap()))?,
            modifiers: TerminalKeyModifiers::from_bits(u16::from_le_bytes(
                bytes[18..20].try_into().unwrap(),
            ))?,
            scalar: u32::from_le_bytes(bytes[20..24].try_into().unwrap()),
        };
        value.validate()?;
        Ok(value)
    }

    fn validate(&self) -> Result<(), FramingError> {
        match self.kind {
            TerminalKeyKind::ControlAscii => {
                if self.modifiers != TerminalKeyModifiers::CONTROL
                    || !valid_control_ascii_scalar(self.scalar)
                {
                    return Err(FramingError::MalformedPayload);
                }
            }
            _ => {
                if self.modifiers != TerminalKeyModifiers::NONE || self.scalar != 0 {
                    return Err(FramingError::MalformedPayload);
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResizeRequest {
    pub attachment_id: AttachmentId,
    pub request_id: u64,
    pub rows: u16,
    pub columns: u16,
}

impl ResizeRequest {
    pub const WIRE_LEN: usize = 32;

    pub fn encode(&self) -> Vec<u8> {
        debug_assert!(self.validate().is_ok());
        let mut out = Vec::with_capacity(Self::WIRE_LEN);
        out.extend_from_slice(&self.attachment_id.to_bytes());
        out.extend_from_slice(&self.request_id.to_le_bytes());
        out.extend_from_slice(&self.rows.to_le_bytes());
        out.extend_from_slice(&self.columns.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, FramingError> {
        exact_len(bytes, Self::WIRE_LEN)?;
        if u32::from_le_bytes(bytes[28..32].try_into().unwrap()) != 0 {
            return Err(FramingError::MalformedPayload);
        }
        let value = Self {
            attachment_id: attachment_id_from(&bytes[..16]),
            request_id: u64::from_le_bytes(bytes[16..24].try_into().unwrap()),
            rows: u16::from_le_bytes(bytes[24..26].try_into().unwrap()),
            columns: u16::from_le_bytes(bytes[26..28].try_into().unwrap()),
        };
        value.validate()?;
        Ok(value)
    }

    fn validate(&self) -> Result<(), FramingError> {
        if self.request_id == 0
            || self.rows == 0
            || self.columns == 0
            || self.rows > MAX_DISPLAY_ROWS
            || self.columns > MAX_DISPLAY_COLUMNS
        {
            return Err(FramingError::MalformedPayload);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResizeResultCode {
    Applied,
    Error(ErrorCode),
}

impl ResizeResultCode {
    pub const fn wire_value(self) -> u16 {
        match self {
            Self::Applied => 0,
            Self::Error(error) => error as u16,
        }
    }

    fn from_u16(value: u16) -> Result<Self, FramingError> {
        if value == 0 {
            return Ok(Self::Applied);
        }
        ErrorCode::from_u16(value)
            .map(Self::Error)
            .ok_or(FramingError::MalformedPayload)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResizeResult {
    pub attachment_id: AttachmentId,
    pub request_id: u64,
    pub result_code: ResizeResultCode,
    pub detail_code: u32,
    pub applied_generation: u64,
}

impl ResizeResult {
    pub const WIRE_LEN: usize = 40;

    pub fn encode(&self) -> Vec<u8> {
        debug_assert!(self.validate().is_ok());
        let mut out = Vec::with_capacity(Self::WIRE_LEN);
        out.extend_from_slice(&self.attachment_id.to_bytes());
        out.extend_from_slice(&self.request_id.to_le_bytes());
        out.extend_from_slice(&self.result_code.wire_value().to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&self.detail_code.to_le_bytes());
        out.extend_from_slice(&self.applied_generation.to_le_bytes());
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, FramingError> {
        exact_len(bytes, Self::WIRE_LEN)?;
        if u16::from_le_bytes(bytes[26..28].try_into().unwrap()) != 0 {
            return Err(FramingError::MalformedPayload);
        }
        let value = Self {
            attachment_id: attachment_id_from(&bytes[..16]),
            request_id: u64::from_le_bytes(bytes[16..24].try_into().unwrap()),
            result_code: ResizeResultCode::from_u16(u16::from_le_bytes(
                bytes[24..26].try_into().unwrap(),
            ))?,
            detail_code: u32::from_le_bytes(bytes[28..32].try_into().unwrap()),
            applied_generation: u64::from_le_bytes(bytes[32..40].try_into().unwrap()),
        };
        value.validate()?;
        Ok(value)
    }

    fn validate(&self) -> Result<(), FramingError> {
        if self.request_id == 0 || self.detail_code != 0 {
            return Err(FramingError::MalformedPayload);
        }
        match self.result_code {
            ResizeResultCode::Applied if self.applied_generation != 0 => Ok(()),
            ResizeResultCode::Error(_) if self.applied_generation == 0 => Ok(()),
            _ => Err(FramingError::MalformedPayload),
        }
    }
}
