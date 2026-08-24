//! SPEC-004 section 9 projection ABI v1.0 fixed-width layout.
//!
//! Every field lives at an explicit byte offset with defined endianness.
//! This module never relies on Rust struct memory layout, pointers, `Vec`
//! internals or parser/`TerminalState` types; it operates only on raw byte
//! slices so the same validation applies whether the bytes came from a
//! Runtime-owned mapping or an untrusted decode fuzz target.

pub const REGION_MAGIC: [u8; 8] = *b"SEYALPRJ";
pub const ABI_MAJOR: u16 = 1;
pub const ABI_MINOR: u16 = 0;
pub const REGION_HEADER_LEN: usize = 128;
pub const SLOT_HEADER_LEN: usize = 64;
pub const CELL_LEN: usize = 16;
pub const DAMAGE_LEN: usize = 8;
pub const SLOT_COUNT: u32 = 2;
pub const MAX_REGION_BYTES: u64 = 8 * 1024 * 1024;
pub const MAX_CAPACITY_ROWS: u16 = 256;
pub const MAX_CAPACITY_COLS: u16 = 512;
pub const MAX_CAPACITY_CELLS: u32 = 131_072;

/// Byte offset of the atomic region publication word inside the region
/// header. Accessed exclusively through atomic operations by
/// [`crate::projection::writer`]; never read/written as a plain integer.
pub const PUBLICATION_WORD_OFFSET: usize = 96;

/// Byte offset of the atomic slot sequence word inside a slot header.
/// Accessed exclusively through atomic operations by
/// [`crate::projection::writer`]; never read/written as a plain integer.
pub const SLOT_SEQUENCE_OFFSET: usize = 0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutError {
    TooShort,
    InvalidMagic,
    UnsupportedAbiVersion,
    InvalidHeaderBytes,
    InvalidSlotCount,
    InvalidSlotHeaderBytes,
    InvalidCellBytes,
    InvalidDamageBytes,
    RegionTooLarge,
    SlotNotAligned,
    SlotOutOfBounds,
    CapacityExceedsMaximum,
    NonzeroReserved,
    InvalidRowsColumns,
    InvalidCellCount,
    InvalidDamageCount,
    InvalidCursorPosition,
    InvalidOffsets,
    InvalidSnapshotFlags,
    InvalidModeFlags,
    InvalidDamageFlags,
    InvalidDamageRange,
    InvalidUnicodeScalar,
    InvalidColorEncoding,
    InvalidAttributeFlags,
    LengthOverflow,
}

/// The static (non-atomic) fields of the 128-byte region header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegionHeader {
    pub region_bytes: u64,
    pub execution_id: u128,
    pub attachment_id: u128,
    pub projection_id: u128,
    pub slot_stride: u64,
    pub slot0_offset: u64,
    pub capacity_rows: u16,
    pub capacity_cols: u16,
}

impl RegionHeader {
    pub fn encode(&self, out: &mut [u8]) -> Result<(), LayoutError> {
        if out.len() < REGION_HEADER_LEN {
            return Err(LayoutError::TooShort);
        }
        out[0..8].copy_from_slice(&REGION_MAGIC);
        out[8..10].copy_from_slice(&ABI_MAJOR.to_le_bytes());
        out[10..12].copy_from_slice(&ABI_MINOR.to_le_bytes());
        out[12..16].copy_from_slice(&(REGION_HEADER_LEN as u32).to_le_bytes());
        out[16..24].copy_from_slice(&self.region_bytes.to_le_bytes());
        out[24..40].copy_from_slice(&self.execution_id.to_le_bytes());
        out[40..56].copy_from_slice(&self.attachment_id.to_le_bytes());
        out[56..72].copy_from_slice(&self.projection_id.to_le_bytes());
        out[72..76].copy_from_slice(&SLOT_COUNT.to_le_bytes());
        out[76..80].copy_from_slice(&(SLOT_HEADER_LEN as u32).to_le_bytes());
        out[80..88].copy_from_slice(&self.slot_stride.to_le_bytes());
        out[88..96].copy_from_slice(&self.slot0_offset.to_le_bytes());
        // Bytes 96..104 are the atomic publication word; the writer
        // initializes it separately through an atomic store.
        out[104..106].copy_from_slice(&self.capacity_rows.to_le_bytes());
        out[106..108].copy_from_slice(&self.capacity_cols.to_le_bytes());
        out[108..110].copy_from_slice(&(CELL_LEN as u16).to_le_bytes());
        out[110..112].copy_from_slice(&(DAMAGE_LEN as u16).to_le_bytes());
        out[112..128].fill(0);
        Ok(())
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, LayoutError> {
        if bytes.len() < REGION_HEADER_LEN {
            return Err(LayoutError::TooShort);
        }
        if bytes[0..8] != REGION_MAGIC {
            return Err(LayoutError::InvalidMagic);
        }
        let major = u16::from_le_bytes(bytes[8..10].try_into().unwrap());
        let minor = u16::from_le_bytes(bytes[10..12].try_into().unwrap());
        if major != ABI_MAJOR || minor != ABI_MINOR {
            return Err(LayoutError::UnsupportedAbiVersion);
        }
        let header_bytes = u32::from_le_bytes(bytes[12..16].try_into().unwrap());
        if header_bytes as usize != REGION_HEADER_LEN {
            return Err(LayoutError::InvalidHeaderBytes);
        }
        let region_bytes = u64::from_le_bytes(bytes[16..24].try_into().unwrap());
        if region_bytes > MAX_REGION_BYTES {
            return Err(LayoutError::RegionTooLarge);
        }
        let execution_id = u128::from_le_bytes(bytes[24..40].try_into().unwrap());
        let attachment_id = u128::from_le_bytes(bytes[40..56].try_into().unwrap());
        let projection_id = u128::from_le_bytes(bytes[56..72].try_into().unwrap());
        let slot_count = u32::from_le_bytes(bytes[72..76].try_into().unwrap());
        if slot_count != SLOT_COUNT {
            return Err(LayoutError::InvalidSlotCount);
        }
        let slot_header_bytes = u32::from_le_bytes(bytes[76..80].try_into().unwrap());
        if slot_header_bytes as usize != SLOT_HEADER_LEN {
            return Err(LayoutError::InvalidSlotHeaderBytes);
        }
        let slot_stride = u64::from_le_bytes(bytes[80..88].try_into().unwrap());
        let slot0_offset = u64::from_le_bytes(bytes[88..96].try_into().unwrap());
        let capacity_rows = u16::from_le_bytes(bytes[104..106].try_into().unwrap());
        let capacity_cols = u16::from_le_bytes(bytes[106..108].try_into().unwrap());
        let cell_bytes = u16::from_le_bytes(bytes[108..110].try_into().unwrap());
        if cell_bytes as usize != CELL_LEN {
            return Err(LayoutError::InvalidCellBytes);
        }
        let damage_bytes = u16::from_le_bytes(bytes[110..112].try_into().unwrap());
        if damage_bytes as usize != DAMAGE_LEN {
            return Err(LayoutError::InvalidDamageBytes);
        }
        if bytes[112..128].iter().any(|&b| b != 0) {
            return Err(LayoutError::NonzeroReserved);
        }
        if capacity_rows > MAX_CAPACITY_ROWS || capacity_cols > MAX_CAPACITY_COLS {
            return Err(LayoutError::CapacityExceedsMaximum);
        }
        if slot0_offset % 64 != 0 {
            return Err(LayoutError::SlotNotAligned);
        }
        // Both slots must fit fully inside region_bytes; every offset/length
        // computation here is checked for overflow before use.
        let slot1_offset = slot0_offset
            .checked_add(slot_stride)
            .ok_or(LayoutError::LengthOverflow)?;
        let slot1_end = slot1_offset
            .checked_add(SLOT_HEADER_LEN as u64)
            .ok_or(LayoutError::LengthOverflow)?;
        let slot0_end = slot0_offset
            .checked_add(SLOT_HEADER_LEN as u64)
            .ok_or(LayoutError::LengthOverflow)?;
        if slot0_end > region_bytes || slot1_end > region_bytes {
            return Err(LayoutError::SlotOutOfBounds);
        }

        Ok(Self {
            region_bytes,
            execution_id,
            attachment_id,
            projection_id,
            slot_stride,
            slot0_offset,
            capacity_rows,
            capacity_cols,
        })
    }

    pub fn slot_offset(&self, slot: u8) -> Result<u64, LayoutError> {
        match slot {
            0 => Ok(self.slot0_offset),
            1 => self
                .slot0_offset
                .checked_add(self.slot_stride)
                .ok_or(LayoutError::LengthOverflow),
            _ => Err(LayoutError::InvalidSlotCount),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModeFlags {
    pub alternate_screen: bool,
    pub cursor_visible: bool,
}

impl ModeFlags {
    fn to_u16(self) -> u16 {
        (self.alternate_screen as u16) | ((self.cursor_visible as u16) << 1)
    }

    fn from_u16(value: u16) -> Result<Self, LayoutError> {
        if value & !0b11 != 0 {
            return Err(LayoutError::InvalidModeFlags);
        }
        Ok(Self {
            alternate_screen: value & 0b01 != 0,
            cursor_visible: value & 0b10 != 0,
        })
    }
}

/// The static (non-atomic) fields of a 64-byte slot header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlotHeader {
    pub generation: u64,
    pub payload_bytes: u32,
    pub rows: u16,
    pub columns: u16,
    pub cursor_row: u16,
    pub cursor_col: u16,
    pub cursor_visible: bool,
    pub cursor_style: u8,
    pub mode_flags: ModeFlags,
    pub cell_count: u32,
    pub damage_count: u16,
    pub full_snapshot: bool,
    pub cells_offset: u32,
    pub damages_offset: u32,
    pub source_damage_generation: u64,
}

impl SlotHeader {
    pub fn encode(&self, out: &mut [u8]) -> Result<(), LayoutError> {
        if out.len() < SLOT_HEADER_LEN {
            return Err(LayoutError::TooShort);
        }
        // Bytes 0..8 are the atomic slot sequence; the writer sets it
        // separately through an atomic store.
        out[8..16].copy_from_slice(&self.generation.to_le_bytes());
        out[16..20].copy_from_slice(&self.payload_bytes.to_le_bytes());
        out[20..22].copy_from_slice(&self.rows.to_le_bytes());
        out[22..24].copy_from_slice(&self.columns.to_le_bytes());
        out[24..26].copy_from_slice(&self.cursor_row.to_le_bytes());
        out[26..28].copy_from_slice(&self.cursor_col.to_le_bytes());
        out[28] = self.cursor_visible as u8;
        out[29] = self.cursor_style;
        out[30..32].copy_from_slice(&self.mode_flags.to_u16().to_le_bytes());
        out[32..36].copy_from_slice(&self.cell_count.to_le_bytes());
        out[36..38].copy_from_slice(&self.damage_count.to_le_bytes());
        out[38] = self.full_snapshot as u8;
        out[39] = 0;
        out[40..44].copy_from_slice(&self.cells_offset.to_le_bytes());
        out[44..48].copy_from_slice(&self.damages_offset.to_le_bytes());
        out[48..56].copy_from_slice(&self.source_damage_generation.to_le_bytes());
        out[56..64].fill(0);
        Ok(())
    }

    /// Validates and decodes the static slot-header fields against the
    /// owning region's `capacity_rows`/`capacity_cols` (SPEC-004 section
    /// 9.3). Does not read the atomic sequence word at offset 0.
    pub fn decode(
        bytes: &[u8],
        capacity_rows: u16,
        capacity_cols: u16,
    ) -> Result<Self, LayoutError> {
        if bytes.len() < SLOT_HEADER_LEN {
            return Err(LayoutError::TooShort);
        }
        let generation = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
        let payload_bytes = u32::from_le_bytes(bytes[16..20].try_into().unwrap());
        let rows = u16::from_le_bytes(bytes[20..22].try_into().unwrap());
        let columns = u16::from_le_bytes(bytes[22..24].try_into().unwrap());
        let cursor_row = u16::from_le_bytes(bytes[24..26].try_into().unwrap());
        let cursor_col = u16::from_le_bytes(bytes[26..28].try_into().unwrap());
        let cursor_visible = match bytes[28] {
            0 => false,
            1 => true,
            _ => return Err(LayoutError::NonzeroReserved),
        };
        let cursor_style = bytes[29];
        let mode_flags = ModeFlags::from_u16(u16::from_le_bytes(bytes[30..32].try_into().unwrap()))?;
        let cell_count = u32::from_le_bytes(bytes[32..36].try_into().unwrap());
        let damage_count = u16::from_le_bytes(bytes[36..38].try_into().unwrap());
        let full_snapshot = match bytes[38] {
            0 => false,
            1 => true,
            _ => return Err(LayoutError::InvalidSnapshotFlags),
        };
        if bytes[39] != 0 {
            return Err(LayoutError::NonzeroReserved);
        }
        let cells_offset = u32::from_le_bytes(bytes[40..44].try_into().unwrap());
        let damages_offset = u32::from_le_bytes(bytes[44..48].try_into().unwrap());
        let source_damage_generation = u64::from_le_bytes(bytes[48..56].try_into().unwrap());
        if bytes[56..64].iter().any(|&b| b != 0) {
            return Err(LayoutError::NonzeroReserved);
        }

        if rows > capacity_rows || columns > capacity_cols {
            return Err(LayoutError::InvalidRowsColumns);
        }
        if !full_snapshot {
            // ABI 1.0 slots always contain a complete visible snapshot.
            return Err(LayoutError::InvalidSnapshotFlags);
        }
        let expected_cells = (rows as u32)
            .checked_mul(columns as u32)
            .ok_or(LayoutError::LengthOverflow)?;
        if cell_count != expected_cells || cell_count > MAX_CAPACITY_CELLS {
            return Err(LayoutError::InvalidCellCount);
        }
        if damage_count as u32 > rows as u32 {
            return Err(LayoutError::InvalidDamageCount);
        }
        if rows > 0 && columns > 0 && (cursor_row >= rows || cursor_col >= columns) {
            return Err(LayoutError::InvalidCursorPosition);
        }

        Ok(Self {
            generation,
            payload_bytes,
            rows,
            columns,
            cursor_row,
            cursor_col,
            cursor_visible,
            cursor_style,
            mode_flags,
            cell_count,
            damage_count,
            full_snapshot,
            cells_offset,
            damages_offset,
            source_damage_generation,
        })
    }

    /// Computes and validates the byte ranges of the cell/damage arrays
    /// within a slot of `slot_stride` total bytes, rejecting any offset or
    /// count that would read outside the slot.
    pub fn cell_and_damage_ranges(
        &self,
        slot_stride: u64,
    ) -> Result<(std::ops::Range<usize>, std::ops::Range<usize>), LayoutError> {
        let cells_start = self.cells_offset as usize;
        let cells_len = (self.cell_count as usize)
            .checked_mul(CELL_LEN)
            .ok_or(LayoutError::LengthOverflow)?;
        let cells_end = cells_start
            .checked_add(cells_len)
            .ok_or(LayoutError::LengthOverflow)?;

        let damages_start = self.damages_offset as usize;
        let damages_len = (self.damage_count as usize)
            .checked_mul(DAMAGE_LEN)
            .ok_or(LayoutError::LengthOverflow)?;
        let damages_end = damages_start
            .checked_add(damages_len)
            .ok_or(LayoutError::LengthOverflow)?;

        let stride = slot_stride as usize;
        if cells_end > stride || damages_end > stride {
            return Err(LayoutError::InvalidOffsets);
        }
        Ok((cells_start..cells_end, damages_start..damages_end))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WireColor {
    Default,
    Indexed(u8),
    Rgb { r: u8, g: u8, b: u8 },
}

impl WireColor {
    fn to_u32(self) -> u32 {
        match self {
            Self::Default => 0,
            Self::Indexed(index) => (0b01u32 << 30) | index as u32,
            Self::Rgb { r, g, b } => {
                (0b10u32 << 30) | ((r as u32) << 16) | ((g as u32) << 8) | b as u32
            }
        }
    }

    fn from_u32(value: u32) -> Result<Self, LayoutError> {
        let kind = value >> 30;
        let payload = value & 0x3fff_ffff;
        match kind {
            0b00 => {
                if payload != 0 {
                    return Err(LayoutError::InvalidColorEncoding);
                }
                Ok(Self::Default)
            }
            0b01 => {
                if payload > 0xff {
                    return Err(LayoutError::InvalidColorEncoding);
                }
                Ok(Self::Indexed(payload as u8))
            }
            0b10 => {
                if payload > 0x00ff_ffff {
                    return Err(LayoutError::InvalidColorEncoding);
                }
                Ok(Self::Rgb {
                    r: ((payload >> 16) & 0xff) as u8,
                    g: ((payload >> 8) & 0xff) as u8,
                    b: (payload & 0xff) as u8,
                })
            }
            _ => Err(LayoutError::InvalidColorEncoding),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WireAttributes {
    pub bold: bool,
    pub underline: bool,
    pub inverse: bool,
}

impl WireAttributes {
    fn to_u16(self) -> u16 {
        (self.bold as u16) | ((self.underline as u16) << 1) | ((self.inverse as u16) << 2)
    }

    fn from_u16(value: u16) -> Result<Self, LayoutError> {
        if value & !0b111 != 0 {
            return Err(LayoutError::InvalidAttributeFlags);
        }
        Ok(Self {
            bold: value & 0b001 != 0,
            underline: value & 0b010 != 0,
            inverse: value & 0b100 != 0,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellRecord {
    pub scalar: char,
    pub foreground: WireColor,
    pub background: WireColor,
    pub attributes: WireAttributes,
}

impl CellRecord {
    pub fn encode(&self, out: &mut [u8]) -> Result<(), LayoutError> {
        if out.len() < CELL_LEN {
            return Err(LayoutError::TooShort);
        }
        out[0..4].copy_from_slice(&(self.scalar as u32).to_le_bytes());
        out[4..8].copy_from_slice(&self.foreground.to_u32().to_le_bytes());
        out[8..12].copy_from_slice(&self.background.to_u32().to_le_bytes());
        out[12..14].copy_from_slice(&self.attributes.to_u16().to_le_bytes());
        out[14..16].fill(0);
        Ok(())
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, LayoutError> {
        if bytes.len() < CELL_LEN {
            return Err(LayoutError::TooShort);
        }
        let scalar_value = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
        let scalar = char::from_u32(scalar_value).ok_or(LayoutError::InvalidUnicodeScalar)?;
        let foreground = WireColor::from_u32(u32::from_le_bytes(bytes[4..8].try_into().unwrap()))?;
        let background = WireColor::from_u32(u32::from_le_bytes(bytes[8..12].try_into().unwrap()))?;
        let attributes =
            WireAttributes::from_u16(u16::from_le_bytes(bytes[12..14].try_into().unwrap()))?;
        if bytes[14..16] != [0, 0] {
            return Err(LayoutError::NonzeroReserved);
        }
        Ok(Self {
            scalar,
            foreground,
            background,
            attributes,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DamageRecord {
    pub first_row: u16,
    pub last_row: u16,
    pub full: bool,
}

impl DamageRecord {
    pub fn encode(&self, out: &mut [u8]) -> Result<(), LayoutError> {
        if out.len() < DAMAGE_LEN {
            return Err(LayoutError::TooShort);
        }
        out[0..2].copy_from_slice(&self.first_row.to_le_bytes());
        out[2..4].copy_from_slice(&self.last_row.to_le_bytes());
        let flags: u16 = self.full as u16;
        out[4..6].copy_from_slice(&flags.to_le_bytes());
        out[6..8].fill(0);
        Ok(())
    }

    pub fn decode(bytes: &[u8], rows: u16) -> Result<Self, LayoutError> {
        if bytes.len() < DAMAGE_LEN {
            return Err(LayoutError::TooShort);
        }
        let first_row = u16::from_le_bytes(bytes[0..2].try_into().unwrap());
        let last_row = u16::from_le_bytes(bytes[2..4].try_into().unwrap());
        let flags = u16::from_le_bytes(bytes[4..6].try_into().unwrap());
        if flags & !0b1 != 0 {
            return Err(LayoutError::InvalidDamageFlags);
        }
        if bytes[6..8] != [0, 0] {
            return Err(LayoutError::NonzeroReserved);
        }
        if first_row > last_row || last_row >= rows {
            return Err(LayoutError::InvalidDamageRange);
        }
        Ok(Self {
            first_row,
            last_row,
            full: flags & 0b1 != 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_region_header() -> RegionHeader {
        RegionHeader {
            region_bytes: REGION_HEADER_LEN as u64 + 2 * 4096,
            execution_id: 1,
            attachment_id: 2,
            projection_id: 3,
            slot_stride: 4096,
            slot0_offset: REGION_HEADER_LEN as u64,
            capacity_rows: 24,
            capacity_cols: 80,
        }
    }

    #[test]
    fn region_header_round_trips() {
        let header = sample_region_header();
        let mut bytes = [0u8; REGION_HEADER_LEN];
        header.encode(&mut bytes).unwrap();
        assert_eq!(RegionHeader::decode(&bytes).unwrap(), header);
    }

    #[test]
    fn region_header_decode_rejects_bad_magic() {
        let header = sample_region_header();
        let mut bytes = [0u8; REGION_HEADER_LEN];
        header.encode(&mut bytes).unwrap();
        bytes[0] = b'X';
        assert_eq!(RegionHeader::decode(&bytes), Err(LayoutError::InvalidMagic));
    }

    #[test]
    fn region_header_decode_rejects_unsupported_abi_version() {
        let header = sample_region_header();
        let mut bytes = [0u8; REGION_HEADER_LEN];
        header.encode(&mut bytes).unwrap();
        bytes[8..10].copy_from_slice(&2u16.to_le_bytes());
        assert_eq!(
            RegionHeader::decode(&bytes),
            Err(LayoutError::UnsupportedAbiVersion)
        );
    }

    #[test]
    fn region_header_decode_rejects_region_larger_than_maximum() {
        let mut header = sample_region_header();
        header.region_bytes = MAX_REGION_BYTES + 1;
        let mut bytes = [0u8; REGION_HEADER_LEN];
        header.encode(&mut bytes).unwrap();
        // encode() writes the actual field regardless; decode must reject it.
        bytes[16..24].copy_from_slice(&header.region_bytes.to_le_bytes());
        assert_eq!(
            RegionHeader::decode(&bytes),
            Err(LayoutError::RegionTooLarge)
        );
    }

    #[test]
    fn region_header_decode_rejects_capacity_above_maximum() {
        let mut header = sample_region_header();
        header.capacity_rows = MAX_CAPACITY_ROWS + 1;
        let mut bytes = [0u8; REGION_HEADER_LEN];
        header.encode(&mut bytes).unwrap();
        bytes[104..106].copy_from_slice(&header.capacity_rows.to_le_bytes());
        assert_eq!(
            RegionHeader::decode(&bytes),
            Err(LayoutError::CapacityExceedsMaximum)
        );
    }

    #[test]
    fn region_header_decode_rejects_unaligned_slot0_offset() {
        let mut header = sample_region_header();
        header.slot0_offset = REGION_HEADER_LEN as u64 + 1;
        let mut bytes = [0u8; REGION_HEADER_LEN];
        header.encode(&mut bytes).unwrap();
        bytes[88..96].copy_from_slice(&header.slot0_offset.to_le_bytes());
        assert_eq!(
            RegionHeader::decode(&bytes),
            Err(LayoutError::SlotNotAligned)
        );
    }

    #[test]
    fn region_header_decode_rejects_slot_out_of_bounds() {
        let mut header = sample_region_header();
        header.region_bytes = REGION_HEADER_LEN as u64 + 10; // too small for a 4096 slot
        let mut bytes = [0u8; REGION_HEADER_LEN];
        header.encode(&mut bytes).unwrap();
        bytes[16..24].copy_from_slice(&header.region_bytes.to_le_bytes());
        assert_eq!(
            RegionHeader::decode(&bytes),
            Err(LayoutError::SlotOutOfBounds)
        );
    }

    #[test]
    fn region_header_decode_rejects_nonzero_reserved() {
        let header = sample_region_header();
        let mut bytes = [0u8; REGION_HEADER_LEN];
        header.encode(&mut bytes).unwrap();
        bytes[112] = 1;
        assert_eq!(
            RegionHeader::decode(&bytes),
            Err(LayoutError::NonzeroReserved)
        );
    }

    fn sample_slot_header() -> SlotHeader {
        SlotHeader {
            generation: 5,
            payload_bytes: 24 * 80 * CELL_LEN as u32,
            rows: 24,
            columns: 80,
            cursor_row: 0,
            cursor_col: 0,
            cursor_visible: true,
            cursor_style: 0,
            mode_flags: ModeFlags {
                alternate_screen: false,
                cursor_visible: true,
            },
            cell_count: 24 * 80,
            damage_count: 1,
            full_snapshot: true,
            cells_offset: SLOT_HEADER_LEN as u32,
            damages_offset: SLOT_HEADER_LEN as u32 + 24 * 80 * CELL_LEN as u32,
            source_damage_generation: 5,
        }
    }

    #[test]
    fn slot_header_round_trips() {
        let header = sample_slot_header();
        let mut bytes = [0u8; SLOT_HEADER_LEN];
        header.encode(&mut bytes).unwrap();
        assert_eq!(SlotHeader::decode(&bytes, 24, 80).unwrap(), header);
    }

    #[test]
    fn slot_header_decode_rejects_rows_above_capacity() {
        let mut header = sample_slot_header();
        header.rows = 25;
        header.cell_count = 25 * 80;
        let mut bytes = [0u8; SLOT_HEADER_LEN];
        header.encode(&mut bytes).unwrap();
        assert_eq!(
            SlotHeader::decode(&bytes, 24, 80),
            Err(LayoutError::InvalidRowsColumns)
        );
    }

    #[test]
    fn slot_header_decode_rejects_incomplete_snapshot_flag() {
        let header = sample_slot_header();
        let mut bytes = [0u8; SLOT_HEADER_LEN];
        header.encode(&mut bytes).unwrap();
        bytes[38] = 0;
        assert_eq!(
            SlotHeader::decode(&bytes, 24, 80),
            Err(LayoutError::InvalidSnapshotFlags)
        );
    }

    #[test]
    fn slot_header_decode_rejects_cell_count_mismatch() {
        let mut header = sample_slot_header();
        header.cell_count -= 1;
        let mut bytes = [0u8; SLOT_HEADER_LEN];
        header.encode(&mut bytes).unwrap();
        assert_eq!(
            SlotHeader::decode(&bytes, 24, 80),
            Err(LayoutError::InvalidCellCount)
        );
    }

    #[test]
    fn slot_header_decode_rejects_cursor_out_of_bounds() {
        let mut header = sample_slot_header();
        header.cursor_row = 24;
        let mut bytes = [0u8; SLOT_HEADER_LEN];
        header.encode(&mut bytes).unwrap();
        assert_eq!(
            SlotHeader::decode(&bytes, 24, 80),
            Err(LayoutError::InvalidCursorPosition)
        );
    }

    #[test]
    fn slot_header_decode_rejects_damage_count_above_rows() {
        let mut header = sample_slot_header();
        header.damage_count = 25;
        let mut bytes = [0u8; SLOT_HEADER_LEN];
        header.encode(&mut bytes).unwrap();
        assert_eq!(
            SlotHeader::decode(&bytes, 24, 80),
            Err(LayoutError::InvalidDamageCount)
        );
    }

    #[test]
    fn cell_and_damage_ranges_reject_offsets_beyond_slot_stride() {
        let mut header = sample_slot_header();
        header.damages_offset = 1_000_000;
        assert_eq!(
            header.cell_and_damage_ranges(4096),
            Err(LayoutError::InvalidOffsets)
        );
    }

    #[test]
    fn cell_record_round_trips_rgb_and_attributes() {
        let cell = CellRecord {
            scalar: 'A',
            foreground: WireColor::Rgb { r: 10, g: 20, b: 30 },
            background: WireColor::Indexed(200),
            attributes: WireAttributes {
                bold: true,
                underline: false,
                inverse: true,
            },
        };
        let mut bytes = [0u8; CELL_LEN];
        cell.encode(&mut bytes).unwrap();
        assert_eq!(CellRecord::decode(&bytes).unwrap(), cell);
    }

    #[test]
    fn cell_record_decode_rejects_invalid_unicode_scalar() {
        let mut bytes = [0u8; CELL_LEN];
        // 0xD800 is a surrogate: not a valid Unicode scalar value.
        bytes[0..4].copy_from_slice(&0xD800u32.to_le_bytes());
        assert_eq!(
            CellRecord::decode(&bytes),
            Err(LayoutError::InvalidUnicodeScalar)
        );
    }

    #[test]
    fn cell_record_decode_rejects_reserved_color_kind() {
        let mut bytes = [0u8; CELL_LEN];
        bytes[0..4].copy_from_slice(&('a' as u32).to_le_bytes());
        bytes[4..8].copy_from_slice(&(0b11u32 << 30).to_le_bytes());
        assert_eq!(
            CellRecord::decode(&bytes),
            Err(LayoutError::InvalidColorEncoding)
        );
    }

    #[test]
    fn damage_record_round_trips() {
        let damage = DamageRecord {
            first_row: 2,
            last_row: 5,
            full: false,
        };
        let mut bytes = [0u8; DAMAGE_LEN];
        damage.encode(&mut bytes).unwrap();
        assert_eq!(DamageRecord::decode(&bytes, 24).unwrap(), damage);
    }

    #[test]
    fn damage_record_decode_rejects_last_row_beyond_rows() {
        let damage = DamageRecord {
            first_row: 0,
            last_row: 24,
            full: false,
        };
        let mut bytes = [0u8; DAMAGE_LEN];
        damage.encode(&mut bytes).unwrap();
        assert_eq!(
            DamageRecord::decode(&bytes, 24),
            Err(LayoutError::InvalidDamageRange)
        );
    }

    #[test]
    fn damage_record_decode_rejects_first_row_after_last_row() {
        let mut bytes = [0u8; DAMAGE_LEN];
        bytes[0..2].copy_from_slice(&5u16.to_le_bytes());
        bytes[2..4].copy_from_slice(&2u16.to_le_bytes());
        assert_eq!(
            DamageRecord::decode(&bytes, 24),
            Err(LayoutError::InvalidDamageRange)
        );
    }

    #[test]
    fn fuzz_arbitrary_bytes_never_panic_decoding_region_or_slot_headers() {
        let mut state: u64 = 0x0fed_cba9_8765_4321;
        for _ in 0..20_000 {
            let mut region_buf = [0u8; REGION_HEADER_LEN];
            for byte in region_buf.iter_mut() {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                *byte = (state & 0xff) as u8;
            }
            let _ = RegionHeader::decode(&region_buf);

            let mut slot_buf = [0u8; SLOT_HEADER_LEN];
            for byte in slot_buf.iter_mut() {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                *byte = (state & 0xff) as u8;
            }
            let _ = SlotHeader::decode(&slot_buf, 256, 512);

            let mut cell_buf = [0u8; CELL_LEN];
            for byte in cell_buf.iter_mut() {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                *byte = (state & 0xff) as u8;
            }
            let _ = CellRecord::decode(&cell_buf);
        }
    }
}
