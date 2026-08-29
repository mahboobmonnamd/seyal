#!/usr/bin/env python3
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def read(path):
    return (ROOT / path).read_text()


def write(path, text):
    (ROOT / path).write_text(text)


def replace_once(text, old, new, label):
    count = text.count(old)
    if count == 0:
        if new in text:
            return text
        raise SystemExit(f"missing integration anchor: {label}")
    if count != 1:
        raise SystemExit(f"ambiguous integration anchor ({count}): {label}")
    return text.replace(old, new, 1)


# ---- Rust disposable client: preserve Pass 7.1 rich command Blocks while
# adding the separate Pass 8 execution-level metadata stream/cache.
path = "crates/seyal-client/src/local.rs"
s = read(path)
if "BlockMetadataConflict" not in s:
    s = replace_once(
        s,
        "    io::{Read, Write},\n    os::{fd::AsRawFd, unix::net::UnixStream},",
        "    io::{Read, Write},\n    net::Shutdown,\n    os::{fd::AsRawFd, unix::net::UnixStream},",
        "client Shutdown import",
    )
    s = replace_once(
        s,
        "    },\n};\n\nconst STARTUP_TIMEOUT",
        "    },\n    pass8::{BLOCK_STATE_MESSAGE_TYPE, BlockLifecycle, BlockState, CAP_BLOCK_METADATA},\n};\n\nuse crate::block::{BlockApply, BlockCache, is_epoch_quarantined, quarantine_epoch};\n\nconst STARTUP_TIMEOUT",
        "client Pass 8 imports",
    )
    s = replace_once(
        s,
        "    ResizeProtocolFailure,\n    InvalidGeometry,\n}",
        "    ResizeProtocolFailure,\n    InvalidGeometry,\n    BlockMetadataConflict,\n}",
        "client error variant",
    )
    s = replace_once(
        s,
        "    outbound_wire_bytes: usize,\n    execution_id: ExecutionId,\n    attachment_id: AttachmentId,\n    role: Role,\n    cache: DisplayCache,",
        "    outbound_wire_bytes: usize,\n    runtime_id: u128,\n    execution_id: ExecutionId,\n    attachment_id: AttachmentId,\n    role: Role,\n    block_metadata_negotiated: bool,\n    block_cache: BlockCache,\n    cache: DisplayCache,",
        "client fields",
    )
    s = replace_once(
        s,
        "        let mut stream = connect_stream(&socket_path)?;\n        let hello = hello(&mut stream, true)?;",
        "        let mut stream = connect_stream(&socket_path)?;\n        let mut hello = hello(&mut stream, true, true)?;",
        "connect first hello",
    )
    s = replace_once(
        s,
        "        Self::finish_attach(\n            stream,\n            execution_id,\n            Role::Controller,\n            hello.server_capabilities & CAP_COMMAND_BLOCKS != 0,\n        )",
        "        if is_epoch_quarantined(hello.runtime_id, execution_id) {\n            drop(stream);\n            stream = connect_stream(&socket_path)?;\n            hello = hello(&mut stream, true, false)?;\n        }\n        let block_metadata_negotiated = hello.server_capabilities & CAP_BLOCK_METADATA != 0\n            && !is_epoch_quarantined(hello.runtime_id, execution_id);\n        Self::finish_attach(\n            stream,\n            execution_id,\n            Role::Controller,\n            hello.server_capabilities & CAP_COMMAND_BLOCKS != 0,\n            hello.runtime_id,\n            block_metadata_negotiated,\n        )",
        "connect first attach",
    )
    s = replace_once(
        s,
        "        let mut stream = connect_stream(socket_path)?;\n        let hello = hello(&mut stream, role == Role::Controller)?;\n        Self::finish_attach(\n            stream,\n            execution_id,\n            role,\n            hello.server_capabilities & CAP_COMMAND_BLOCKS != 0,\n        )",
        "        let mut stream = connect_stream(socket_path)?;\n        let mut hello = hello(&mut stream, role == Role::Controller, true)?;\n        if is_epoch_quarantined(hello.runtime_id, execution_id) {\n            drop(stream);\n            stream = connect_stream(socket_path)?;\n            hello = hello(&mut stream, role == Role::Controller, false)?;\n        }\n        let block_metadata_negotiated = hello.server_capabilities & CAP_BLOCK_METADATA != 0\n            && !is_epoch_quarantined(hello.runtime_id, execution_id);\n        Self::finish_attach(\n            stream,\n            execution_id,\n            role,\n            hello.server_capabilities & CAP_COMMAND_BLOCKS != 0,\n            hello.runtime_id,\n            block_metadata_negotiated,\n        )",
        "connect execution attach",
    )
    s = replace_once(
        s,
        "        role: Role,\n        command_blocks_supported: bool,\n    ) -> Result<Self, ClientError>",
        "        role: Role,\n        command_blocks_supported: bool,\n        runtime_id: u128,\n        block_metadata_negotiated: bool,\n    ) -> Result<Self, ClientError>",
        "finish attach signature",
    )
    s = replace_once(
        s,
        "            outbound_wire_bytes: 0,\n            execution_id,\n            attachment_id: attached.attachment_id,\n            role,\n            cache,",
        "            outbound_wire_bytes: 0,\n            runtime_id,\n            execution_id,\n            attachment_id: attached.attachment_id,\n            role,\n            block_metadata_negotiated,\n            block_cache: BlockCache::default(),\n            cache,",
        "client construction",
    )
    s = replace_once(
        s,
        "    pub fn execution_id(&self) -> ExecutionId {\n        self.execution_id\n    }\n\n    pub fn cache(&self) -> &DisplayCache",
        "    pub fn execution_id(&self) -> ExecutionId {\n        self.execution_id\n    }\n\n    /// Disposable Pass 8 execution-level metadata. This never owns terminal\n    /// cells, PTY state, or the Pass 7.1 command transcript.\n    pub fn block_state(&self) -> Option<BlockState> {\n        self.block_cache.visible()\n    }\n\n    pub fn cache(&self) -> &DisplayCache",
        "block state accessor",
    )
    s = replace_once(
        s,
        "                let header = FrameHeader::decode(frame).map_err(|_| ClientError::Protocol)?;\n                let message_type =\n                    MessageType::from_u16(header.message_type).ok_or(ClientError::Protocol)?;\n\n                match message_type {",
        "                let header = FrameHeader::decode(frame).map_err(|_| ClientError::Protocol)?;\n\n                // SPEC-007 type 20 is Runtime→client metadata, not a C→Runtime\n                // control message. Parse it before the control MessageType enum.\n                if header.message_type == BLOCK_STATE_MESSAGE_TYPE {\n                    if !self.block_metadata_negotiated {\n                        return Err(self.quarantine_block_metadata());\n                    }\n                    let incoming = match BlockState::decode(&frame[HEADER_LEN..]) {\n                        Ok(value) => value,\n                        Err(_) => return Err(self.quarantine_block_metadata()),\n                    };\n                    match self.block_cache.apply(self.execution_id, incoming) {\n                        Ok(BlockApply::Applied) => metadata_changed = true,\n                        Ok(BlockApply::Duplicate | BlockApply::Stale) => {}\n                        Err(_) => return Err(self.quarantine_block_metadata()),\n                    }\n                    self.read_offset = frame_end;\n                    parsed_frames += 1;\n                    continue;\n                }\n\n                let message_type =\n                    MessageType::from_u16(header.message_type).ok_or(ClientError::Protocol)?;\n\n                match message_type {",
        "type 20 parser",
    )
    s = replace_once(
        s,
        "                    MessageType::Lifecycle => {}",
        "                    MessageType::Lifecycle => {\n                        let lifecycle = seyal_runtime::local_ipc::framing::LifecycleMessage::decode(\n                            &frame[HEADER_LEN..],\n                        )\n                        .map_err(|_| ClientError::Protocol)?;\n                        if lifecycle.execution_id != self.execution_id {\n                            return Err(ClientError::Protocol);\n                        }\n                        if lifecycle.lifecycle == Lifecycle::Finalized\n                            && self.block_metadata_negotiated\n                            && self\n                                .block_cache\n                                .visible()\n                                .is_some_and(|block| block.state == BlockLifecycle::Current)\n                        {\n                            return Err(self.quarantine_block_metadata());\n                        }\n                    }",
        "finalized consistency",
    )
    s = replace_once(
        s,
        "    fn complete_frame_end(&self) -> Result<Option<usize>, ClientError> {",
        "    fn quarantine_block_metadata(&mut self) -> ClientError {\n        self.block_cache.quarantine();\n        quarantine_epoch(self.runtime_id, self.execution_id);\n        let _ = self.stream.shutdown(Shutdown::Both);\n        ClientError::BlockMetadataConflict\n    }\n\n    fn complete_frame_end(&self) -> Result<Option<usize>, ClientError> {",
        "quarantine method",
    )
    s = replace_once(
        s,
        "fn hello(stream: &mut UnixStream, interactive: bool) -> Result<ServerHello, ClientError> {\n    send_control(\n        stream,\n        MessageType::ClientHello,\n        &ClientHello {\n            client_capabilities: CAP_COMMAND_BLOCKS,\n        }\n        .encode(),\n    )?;",
        "fn hello(\n    stream: &mut UnixStream,\n    interactive: bool,\n    request_block_metadata: bool,\n) -> Result<ServerHello, ClientError> {\n    let client_capabilities = CAP_COMMAND_BLOCKS\n        | if request_block_metadata {\n            CAP_BLOCK_METADATA\n        } else {\n            0\n        };\n    send_control(\n        stream,\n        MessageType::ClientHello,\n        &ClientHello { client_capabilities }.encode(),\n    )?;",
        "hello capability request",
    )
    write(path, s)

# ---- FFI: preserve multi-pane/rich command-block ABI and append only a
# read-only execution metadata value plus the stable -18 conflict diagnostic.
path = "crates/seyal-client/src/ffi.rs"
s = read(path)
if "SeyalExecutionBlockMetadata" not in s:
    anchor = "#[derive(Clone, Copy, Debug)]\n#[repr(C)]\npub struct SeyalBlockRecord {"
    addition = """#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct SeyalExecutionBlockMetadata {
    pub block_id_low: u64,
    pub block_id_high: u64,
    pub revision: u64,
    pub start_line_id: u64,
    pub state: u8,
    pub reserved: [u8; 7],
}

impl SeyalExecutionBlockMetadata {
    const fn empty() -> Self {
        Self {
            block_id_low: 0,
            block_id_high: 0,
            revision: 0,
            start_line_id: 0,
            state: 0,
            reserved: [0; 7],
        }
    }
}

""" + anchor
    s = replace_once(s, anchor, addition, "FFI execution metadata struct")
    anchor = "#[unsafe(no_mangle)]\npub extern \"C\" fn seyal_bridge_execution_id_high() -> u64 {\n    with_active_client(|client| {\n        u64::from_le_bytes(client.execution_id().to_bytes()[8..16].try_into().unwrap())\n    })\n    .unwrap_or(0)\n}\n"
    addition = anchor + """
/// Read-only Pass 8 execution metadata for the active Pane client. No command
/// text, terminal cells, history, cwd, or PTY bytes cross this seam.
#[unsafe(no_mangle)]
pub extern "C" fn seyal_bridge_execution_block_metadata() -> SeyalExecutionBlockMetadata {
    with_active_client(|client| client.block_state())
        .flatten()
        .map(|block| {
            let bytes = block.block_id.to_bytes();
            SeyalExecutionBlockMetadata {
                block_id_low: u64::from_le_bytes(bytes[..8].try_into().unwrap()),
                block_id_high: u64::from_le_bytes(bytes[8..].try_into().unwrap()),
                revision: block.revision,
                start_line_id: block.start_line_id,
                state: match block.state {
                    seyal_runtime::pass8::BlockLifecycle::Current => 1,
                    seyal_runtime::pass8::BlockLifecycle::Completed => 2,
                },
                reserved: [0; 7],
            }
        })
        .unwrap_or_else(SeyalExecutionBlockMetadata::empty)
}
"""
    s = replace_once(s, anchor, addition, "FFI execution metadata accessor")
    s = replace_once(
        s,
        "        ClientError::InvalidGeometry => -17,\n        ClientError::Server(code)",
        "        ClientError::InvalidGeometry => -17,\n        ClientError::BlockMetadataConflict => -18,\n        ClientError::Server(code)",
        "FFI error mapping",
    )
    write(path, s)

# ---- C header: keep Pass 7.1 command Block ABI intact and append a distinct
# execution-metadata type.
path = "macos/Seyal/Sources/SeyalBridge.h"
s = read(path)
if "SeyalExecutionBlockMetadata" not in s:
    anchor = "typedef struct SeyalBlockRecord {"
    addition = """typedef struct SeyalExecutionBlockMetadata {
    uint64_t block_id_low;
    uint64_t block_id_high;
    uint64_t revision;
    uint64_t start_line_id;
    uint8_t state;
    uint8_t reserved[7];
} SeyalExecutionBlockMetadata;

""" + anchor
    s = replace_once(s, anchor, addition, "header execution metadata struct")
    s = replace_once(
        s,
        "uint64_t seyal_bridge_execution_id_high(void);\nint32_t seyal_bridge_poll(void);",
        "uint64_t seyal_bridge_execution_id_high(void);\nSeyalExecutionBlockMetadata seyal_bridge_execution_block_metadata(void);\nint32_t seyal_bridge_poll(void);",
        "header execution metadata function",
    )
    write(path, s)

# ---- Swift bridge: expose the opaque execution metadata without manufacturing
# command cards or touching the rich Pass 7.1 transcript model.
path = "macos/Seyal/Sources/RustDisplayBridge.swift"
s = read(path)
if "struct RuntimeBlockMetadata" not in s:
    s = replace_once(
        s,
        "import Foundation\n\n/// UI registries",
        """import Foundation

struct RuntimeBlockMetadata: Equatable, Sendable {
  enum State: UInt8, Sendable {
    case current = 1
    case completed = 2
  }

  let blockIDLow: UInt64
  let blockIDHigh: UInt64
  let revision: UInt64
  let startLineID: UInt64
  let state: State
}

/// UI registries""",
        "Swift execution metadata model",
    )
    s = replace_once(
        s,
        "  func publishCurrentFrame() {\n    guard let frame = currentFrame() else { return }\n    onFrame(frame)\n  }\n\n  func currentTimeline()",
        """  func publishCurrentFrame() {
    guard let frame = currentFrame() else { return }
    onFrame(frame)
  }

  /// Minimal read-only Pass 8 presentation seam. The rich command transcript
  /// remains the independent Pass 7.1 timeline above.
  func currentBlockMetadata() -> RuntimeBlockMetadata? {
    guard isConnected, selectClient() else { return nil }
    let value = seyal_bridge_execution_block_metadata()
    guard value.revision > 0,
      value.start_line_id > 0,
      let state = RuntimeBlockMetadata.State(rawValue: value.state)
    else { return nil }
    return RuntimeBlockMetadata(
      blockIDLow: value.block_id_low,
      blockIDHigh: value.block_id_high,
      revision: value.revision,
      startLineID: value.start_line_id,
      state: state
    )
  }

  func currentTimeline()""",
        "Swift execution metadata accessor",
    )
    s = replace_once(
        s,
        "    if result == -3 || result == -10 {",
        "    if result == -3 || result == -10 || result == -18 {",
        "Swift conflict reconnect",
    )
    write(path, s)

# ---- Native component coverage required by repository policy.
path = "macos/Seyal/Tests/SeyalTests/SeyalShellComponentTests.swift"
s = read(path)
if "testRuntimeBlockMetadataKeepsOpaqueExecutionIdentity" not in s:
    anchor = "final class SeyalShellComponentTests: XCTestCase {\n"
    addition = anchor + """
  func testRuntimeBlockMetadataKeepsOpaqueExecutionIdentityAnchorAndState() {
    let current = RuntimeBlockMetadata(
      blockIDLow: 0x0123,
      blockIDHigh: 0x4567,
      revision: 1,
      startLineID: 99,
      state: .current
    )
    let completed = RuntimeBlockMetadata(
      blockIDLow: current.blockIDLow,
      blockIDHigh: current.blockIDHigh,
      revision: 2,
      startLineID: current.startLineID,
      state: .completed
    )

    XCTAssertEqual(current.blockIDLow, completed.blockIDLow)
    XCTAssertEqual(current.blockIDHigh, completed.blockIDHigh)
    XCTAssertEqual(current.startLineID, completed.startLineID)
    XCTAssertEqual(current.revision, 1)
    XCTAssertEqual(completed.revision, 2)
    XCTAssertEqual(current.state, .current)
    XCTAssertEqual(completed.state, .completed)
    XCTAssertNotEqual(current, completed)
  }
"""
    s = replace_once(s, anchor, addition, "XCTest execution metadata coverage")
    write(path, s)
