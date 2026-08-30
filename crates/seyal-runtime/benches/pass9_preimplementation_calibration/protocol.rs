use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
};

use seyal_protocol::pass8::BLOCK_STATE_MESSAGE_TYPE;
use seyal_render::{
    CellSource, CommittedDisplay, CursorState, PreparedSurface, RenderAttributes, RenderCell,
    RenderColor, RowDamage,
};
use seyal_runtime::{
    display::{DisplayAttributes, DisplayCache, DisplayCell, DisplayColor},
    local_ipc::framing::{ErrorMessage, FrameHeader, HEADER_LEN, MessageType, encode_frame},
};

pub(crate) fn send_frame(stream: &mut UnixStream, kind: MessageType, payload: &[u8]) {
    stream
        .write_all(&encode_frame(kind, payload))
        .expect("write local IPC frame");
}

pub(crate) fn read_until(stream: &mut UnixStream, wanted: u16) -> Vec<u8> {
    loop {
        let (kind, payload) = read_frame(stream);
        if kind == wanted {
            return payload;
        }
        if kind == MessageType::Error as u16 {
            panic_server_error(&payload, &format!("waiting for {wanted}"));
        }
        assert!(
            kind == MessageType::DisplaySnapshot as u16
                || kind == MessageType::DisplayDelta as u16
                || kind == MessageType::Attached as u16
                || kind == MessageType::ServerHello as u16
                || kind == MessageType::BlockTimeline as u16
                || kind == BLOCK_STATE_MESSAGE_TYPE,
            "unexpected local IPC frame type {kind} while waiting for {wanted}"
        );
    }
}

pub(crate) fn panic_server_error(payload: &[u8], context: &str) -> ! {
    let error = ErrorMessage::decode(payload).expect("Error frame decode");
    panic!(
        "Runtime Error during {context}: code={} offending_type={} detail={}",
        error.error_code, error.offending_message_type, error.detail_code
    );
}

pub(crate) fn read_frame(stream: &mut UnixStream) -> (u16, Vec<u8>) {
    let mut header_bytes = [0u8; HEADER_LEN];
    stream
        .read_exact(&mut header_bytes)
        .expect("read local IPC frame header");
    let header = FrameHeader::decode(&header_bytes).expect("decode local IPC frame header");
    let mut payload = vec![0u8; header.payload_len as usize];
    stream
        .read_exact(&mut payload)
        .expect("read local IPC frame payload");
    (header.message_type, payload)
}

struct RuntimeCells<'a>(&'a [DisplayCell]);

impl CellSource for RuntimeCells<'_> {
    fn len(&self) -> usize {
        self.0.len()
    }

    fn cell(&self, index: usize) -> Option<RenderCell> {
        self.0.get(index).copied().map(runtime_cell_to_render)
    }
}

pub(crate) fn prepare_surface(prepared: &mut PreparedSurface, cache: &DisplayCache) {
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
            RowDamage::full(cache.rows),
            true,
        )
        .expect("PreparedSurface commit");
}

fn runtime_cell_to_render(cell: DisplayCell) -> RenderCell {
    RenderCell {
        scalar: cell.scalar,
        foreground: match cell.foreground {
            DisplayColor::Default => RenderColor::Default,
            DisplayColor::Indexed(index) => RenderColor::Indexed(index),
            DisplayColor::Rgb { r, g, b } => RenderColor::Rgb { r, g, b },
        },
        background: match cell.background {
            DisplayColor::Default => RenderColor::Default,
            DisplayColor::Indexed(index) => RenderColor::Indexed(index),
            DisplayColor::Rgb { r, g, b } => RenderColor::Rgb { r, g, b },
        },
        attributes: runtime_attributes_to_render(cell.attributes),
    }
}

fn runtime_attributes_to_render(attributes: DisplayAttributes) -> RenderAttributes {
    RenderAttributes {
        bold: attributes.bold,
        underline: attributes.underline,
        inverse: attributes.inverse,
    }
}
