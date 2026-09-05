use seyal_render::{
    CellSource, CommittedDisplay, CursorState, PreparationResult, PreparedSurface, RenderAttributes,
    RenderCell, RenderColor, RowDamage,
};
use seyal_runtime::{
    display::{
        DISPLAY_CELL_LEN, DISPLAY_CHUNK_HEADER_LEN, DecodedDisplayChunk, DisplayAttributes,
        DisplayCache, DisplayCell, DisplayColor, DisplayError, DisplayKind, MAX_DISPLAY_BATCH_BYTES,
        MAX_DISPLAY_CELLS,
    },
    local_ipc::framing::HEADER_LEN,
};

use super::{ClientError, LocalDisplayClient};

#[derive(Debug, Default)]
pub(crate) struct PendingDisplayBatch {
    chunks: Vec<DecodedDisplayChunk>,
    cells: usize,
    rows: usize,
    wire_bytes: usize,
}

impl PendingDisplayBatch {
    pub(crate) fn push(&mut self, chunk: DecodedDisplayChunk) -> Result<bool, ClientError> {
        let expected_count = usize::from(chunk.chunk_count);
        if expected_count == 0 || expected_count > usize::from(chunk.rows) {
            return Err(ClientError::Capacity);
        }

        if self.chunks.is_empty() {
            if chunk.chunk_index != 0 {
                return Err(ClientError::Protocol);
            }
            if chunk.kind == DisplayKind::Snapshot && chunk.first_row != 0 {
                return Err(ClientError::Protocol);
            }
            self.chunks.reserve(expected_count);
        } else {
            let first = self.chunks.first().ok_or(ClientError::Protocol)?;
            let previous = self.chunks.last().ok_or(ClientError::Protocol)?;
            if chunk.kind != first.kind
                || chunk.generation != first.generation
                || chunk.base_generation != first.base_generation
                || chunk.rows != first.rows
                || chunk.columns != first.columns
                || chunk.cursor_row != first.cursor_row
                || chunk.cursor_col != first.cursor_col
                || chunk.cursor_visible != first.cursor_visible
                || chunk.alternate_screen != first.alternate_screen
                || chunk.chunk_count != first.chunk_count
                || usize::from(chunk.chunk_index) != self.chunks.len()
            {
                return Err(ClientError::Protocol);
            }
            let expected_first_row = previous
                .first_row
                .checked_add(previous.row_count)
                .ok_or(ClientError::Capacity)?;
            if chunk.first_row != expected_first_row {
                return Err(ClientError::Protocol);
            }
        }

        if self.chunks.len() >= expected_count {
            return Err(ClientError::Protocol);
        }

        let next_rows = self
            .rows
            .checked_add(usize::from(chunk.row_count))
            .ok_or(ClientError::Capacity)?;
        if next_rows > usize::from(chunk.rows) {
            return Err(ClientError::Capacity);
        }

        let geometry_cells = usize::from(chunk.rows)
            .checked_mul(usize::from(chunk.columns))
            .ok_or(ClientError::Capacity)?;
        let next_cells = self
            .cells
            .checked_add(chunk.cells.len())
            .ok_or(ClientError::Capacity)?;
        if next_cells > geometry_cells || next_cells > MAX_DISPLAY_CELLS {
            return Err(ClientError::Capacity);
        }

        let chunk_wire_bytes = HEADER_LEN
            .checked_add(DISPLAY_CHUNK_HEADER_LEN)
            .and_then(|value| {
                chunk
                    .cells
                    .len()
                    .checked_mul(DISPLAY_CELL_LEN)
                    .and_then(|cell_bytes| value.checked_add(cell_bytes))
            })
            .ok_or(ClientError::Capacity)?;
        let next_wire_bytes = self
            .wire_bytes
            .checked_add(chunk_wire_bytes)
            .ok_or(ClientError::Capacity)?;
        if next_wire_bytes > MAX_DISPLAY_BATCH_BYTES {
            return Err(ClientError::Capacity);
        }

        self.rows = next_rows;
        self.cells = next_cells;
        self.wire_bytes = next_wire_bytes;
        self.chunks.push(chunk);
        Ok(self.chunks.len() == expected_count)
    }

    pub(crate) fn chunks(&self) -> &[DecodedDisplayChunk] {
        &self.chunks
    }

    pub(crate) fn clear(&mut self) {
        self.chunks.clear();
        self.cells = 0;
        self.rows = 0;
        self.wire_bytes = 0;
    }
}

impl LocalDisplayClient {
    /// Builds the initial PreparedSurface after attach snapshot commit.
    /// Idempotent; subsequent calls are no-ops until the next attach.
    pub fn ensure_prepared_surface(&mut self) -> Result<PreparationResult, ClientError> {
        if !self.needs_initial_prepare {
            return Ok(self.last_preparation);
        }
        if self.cache.rows == 0 || self.cache.columns == 0 {
            return Err(ClientError::Protocol);
        }
        let result = prepare_cache(
            &mut self.prepared,
            &self.cache,
            RowDamage::full(self.cache.rows),
            true,
        )?;
        self.last_preparation = result;
        self.needs_initial_prepare = false;
        Ok(result)
    }

    pub(crate) fn complete_frame_end(&self) -> Result<Option<usize>, ClientError> {
        let available = self.buffered.len().saturating_sub(self.read_offset);
        if available < HEADER_LEN {
            return Ok(None);
        }
        let header_end = self
            .read_offset
            .checked_add(HEADER_LEN)
            .ok_or(ClientError::Capacity)?;
        let header = seyal_runtime::local_ipc::framing::FrameHeader::decode(
            &self.buffered[self.read_offset..header_end],
        )
        .map_err(|_| ClientError::Protocol)?;
        let total = HEADER_LEN
            .checked_add(header.payload_len as usize)
            .ok_or(ClientError::Capacity)?;
        if available < total {
            return Ok(None);
        }
        self.read_offset
            .checked_add(total)
            .map(Some)
            .ok_or(ClientError::Capacity)
    }

    pub(crate) fn accept_display_chunk(
        &mut self,
        chunk: DecodedDisplayChunk,
        damage: &mut RowDamage,
        full_invalidation: &mut bool,
    ) -> Result<bool, ClientError> {
        if !self.pending_batch.push(chunk)? {
            return Ok(false);
        }

        let first_kind = self
            .pending_batch
            .chunks()
            .first()
            .map(|first| first.kind)
            .ok_or(ClientError::Protocol)?;
        match self.cache.apply_chunks(self.pending_batch.chunks()) {
            Ok(()) => {}
            Err(DisplayError::GenerationMismatch | DisplayError::DimensionMismatch) => {
                self.pending_batch.clear();
                self.request_resync()?;
                return Ok(false);
            }
            Err(_) => return Err(ClientError::Display),
        }

        match first_kind {
            DisplayKind::Snapshot => {
                *damage = RowDamage::full(self.cache.rows);
                *full_invalidation = true;
            }
            DisplayKind::Delta => {
                for committed in self.pending_batch.chunks() {
                    damage.union(
                        RowDamage::from_range(committed.first_row, committed.row_count)
                            .map_err(|_| ClientError::Prepare)?,
                    );
                }
            }
        }
        self.pending_batch.clear();
        self.observe_projection()?;
        Ok(true)
    }

    pub(crate) fn compact_buffer(&mut self) {
        if self.read_offset == 0 {
            return;
        }
        if self.read_offset >= self.buffered.len() {
            self.buffered.clear();
        } else {
            self.buffered.drain(..self.read_offset);
        }
        self.read_offset = 0;
    }
}

pub(crate) struct RuntimeCells<'a>(pub(crate) &'a [DisplayCell]);

impl CellSource for RuntimeCells<'_> {
    fn len(&self) -> usize {
        self.0.len()
    }

    fn cell(&self, index: usize) -> Option<RenderCell> {
        self.0.get(index).copied().map(runtime_cell_to_render)
    }
}

pub(crate) fn prepare_cache(
    prepared: &mut PreparedSurface,
    cache: &DisplayCache,
    damage: RowDamage,
    full_invalidation: bool,
) -> Result<PreparationResult, ClientError> {
    let source = RuntimeCells(&cache.cells);
    prepared
        .prepare(
            CommittedDisplay {
                generation: cache.generation,
                rows: cache.rows,
                columns: cache.columns,
                cursor: CursorState::new(cache.cursor_row, cache.cursor_col, cache.cursor_visible),
                alternate_screen: cache.alternate_screen,
                cells: &source,
            },
            damage,
            full_invalidation,
        )
        .map_err(|_| ClientError::Prepare)
}

fn runtime_cell_to_render(cell: DisplayCell) -> RenderCell {
    RenderCell {
        scalar: cell.scalar,
        foreground: runtime_color_to_render(cell.foreground),
        background: runtime_color_to_render(cell.background),
        attributes: runtime_attributes_to_render(cell.attributes),
    }
}

fn runtime_color_to_render(color: DisplayColor) -> RenderColor {
    match color {
        DisplayColor::Default => RenderColor::Default,
        DisplayColor::Indexed(index) => RenderColor::Indexed(index),
        DisplayColor::Rgb { r, g, b } => RenderColor::Rgb { r, g, b },
    }
}

fn runtime_attributes_to_render(attributes: DisplayAttributes) -> RenderAttributes {
    RenderAttributes {
        bold: attributes.bold,
        underline: attributes.underline,
        inverse: attributes.inverse,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use seyal_runtime::display::{DisplayAttributes, DisplayColor};

    fn display_cell() -> DisplayCell {
        DisplayCell {
            scalar: 'x',
            foreground: DisplayColor::Default,
            background: DisplayColor::Default,
            attributes: DisplayAttributes::default(),
        }
    }

    fn decoded_chunk(
        chunk_index: u16,
        chunk_count: u16,
        first_row: u16,
        row_count: u16,
    ) -> DecodedDisplayChunk {
        let columns = 1;
        DecodedDisplayChunk {
            kind: DisplayKind::Delta,
            generation: 2,
            base_generation: 1,
            rows: 4,
            columns,
            cursor_row: 0,
            cursor_col: 0,
            cursor_visible: true,
            alternate_screen: false,
            first_row,
            row_count,
            chunk_index,
            chunk_count,
            cells: vec![display_cell(); usize::from(row_count) * usize::from(columns)],
        }
    }

    #[test]
    fn runtime_cell_adapter_preserves_scalar_style_and_color_without_copying_cache() {
        let cells = [DisplayCell {
            scalar: 'Q',
            foreground: DisplayColor::Indexed(5),
            background: DisplayColor::Rgb { r: 1, g: 2, b: 3 },
            attributes: DisplayAttributes {
                bold: true,
                underline: true,
                inverse: false,
            },
        }];
        let source = RuntimeCells(&cells);
        let converted = source.cell(0).unwrap();
        assert_eq!(converted.scalar, 'Q');
        assert_eq!(converted.foreground, RenderColor::Indexed(5));
        assert_eq!(converted.background, RenderColor::Rgb { r: 1, g: 2, b: 3 });
        assert!(converted.attributes.bold);
        assert!(converted.attributes.underline);
    }

    #[test]
    fn pending_display_batch_rejects_impossible_chunk_count_before_allocation_growth() {
        let mut batch = PendingDisplayBatch::default();
        let mut chunk = decoded_chunk(0, 5, 0, 1);
        chunk.rows = 4;

        assert_eq!(batch.push(chunk), Err(ClientError::Capacity));
        assert!(batch.chunks().is_empty());
    }

    #[test]
    fn pending_display_batch_rejects_replayed_or_noncontiguous_rows() {
        let mut batch = PendingDisplayBatch::default();
        assert!(!batch.push(decoded_chunk(0, 2, 0, 1)).unwrap());

        let replayed = decoded_chunk(1, 2, 0, 1);
        assert_eq!(batch.push(replayed), Err(ClientError::Protocol));
        assert_eq!(batch.chunks().len(), 1);
    }
}
