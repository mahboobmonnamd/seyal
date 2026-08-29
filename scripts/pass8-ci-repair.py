#!/usr/bin/env python3
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def replace(path: str, old: str, new: str) -> None:
    p = ROOT / path
    text = p.read_text()
    if new in text:
        return
    if text.count(old) != 1:
        raise SystemExit(f"anchor mismatch in {path}: {text.count(old)}")
    p.write_text(text.replace(old, new, 1))


# Pass 8 is a non-interactive native seam. Restore the visible Metal surface to
# master and consume metadata inside the production bridge instead.
subprocess.run(
    ["git", "show", "origin/master:macos/Seyal/Sources/MetalSurfaceView.swift"],
    cwd=ROOT,
    check=True,
    stdout=(ROOT / "macos/Seyal/Sources/MetalSurfaceView.swift").open("w"),
)

replace(
    "macos/Seyal/Sources/RustDisplayBridge.swift",
    """  private(set) var clientHandle: UInt64 = 0\n  private(set) var isConnected = false\n  private var reconnectRequested = false\n""",
    """  private(set) var clientHandle: UInt64 = 0\n  private(set) var isConnected = false\n  private(set) var runtimeBlockMetadata: RuntimeBlockMetadata?\n  private var reconnectRequested = false\n""",
)
replace(
    "macos/Seyal/Sources/RustDisplayBridge.swift",
    """    socketFileDescriptor = fileDescriptor\n    isConnected = true\n    let source = DispatchSource.makeReadSource(fileDescriptor: fileDescriptor, queue: .main)\n""",
    """    socketFileDescriptor = fileDescriptor\n    isConnected = true\n    runtimeBlockMetadata = currentBlockMetadata()\n    let source = DispatchSource.makeReadSource(fileDescriptor: fileDescriptor, queue: .main)\n""",
)
replace(
    "macos/Seyal/Sources/RustDisplayBridge.swift",
    """    isConnected = false\n    socketFileDescriptor = -1\n    _ = seyal_bridge_select(clientHandle)\n""",
    """    isConnected = false\n    runtimeBlockMetadata = nil\n    socketFileDescriptor = -1\n    _ = seyal_bridge_select(clientHandle)\n""",
)
replace(
    "macos/Seyal/Sources/RustDisplayBridge.swift",
    """      let result = seyal_bridge_poll()\n      publishHistoryRanges()\n      publishComposerResult()\n""",
    """      let result = seyal_bridge_poll()\n      runtimeBlockMetadata = currentBlockMetadata()\n      publishHistoryRanges()\n      publishComposerResult()\n""",
)

# Raw fallback must preserve Pass 7.1 command Blocks while independently
# disabling Pass 8 metadata.
replace(
    "crates/seyal-client/src/local.rs",
    """fn hello(\n    stream: &mut UnixStream,\n    interactive: bool,\n    request_block_metadata: bool,\n) -> Result<ServerHello, ClientError> {\n    let client_capabilities = CAP_COMMAND_BLOCKS\n        | if request_block_metadata {\n            CAP_BLOCK_METADATA\n        } else {\n            0\n        };\n""",
    """fn requested_capabilities(request_block_metadata: bool) -> u32 {\n    CAP_COMMAND_BLOCKS\n        | if request_block_metadata {\n            CAP_BLOCK_METADATA\n        } else {\n            0\n        }\n}\n\nfn hello(\n    stream: &mut UnixStream,\n    interactive: bool,\n    request_block_metadata: bool,\n) -> Result<ServerHello, ClientError> {\n    let client_capabilities = requested_capabilities(request_block_metadata);\n""",
)
replace(
    "crates/seyal-client/src/local.rs",
    """    fn geometry(rows: u16, columns: u16) -> GridGeometry {\n        GridGeometry { rows, columns }\n    }\n""",
    """    fn geometry(rows: u16, columns: u16) -> GridGeometry {\n        GridGeometry { rows, columns }\n    }\n\n    #[test]\n    fn raw_metadata_fallback_keeps_pass71_but_drops_only_pass8_capability() {\n        let full = requested_capabilities(true);\n        let fallback = requested_capabilities(false);\n        assert_ne!(full & CAP_BLOCK_METADATA, 0);\n        assert_eq!(fallback & CAP_BLOCK_METADATA, 0);\n        assert_ne!(full & CAP_COMMAND_BLOCKS, 0);\n        assert_ne!(fallback & CAP_COMMAND_BLOCKS, 0);\n    }\n""",
)

# Keep Linux/all-features builds warning-free without widening macOS-only wire
# projection ownership.
replace(
    "crates/seyal-runtime/src/block.rs",
    """use seyal_protocol::pass8::{\n    BlockKind as WireBlockKind, BlockLifecycle as WireBlockLifecycle, BlockState as WireBlockState,\n};\n""",
    """#[cfg(target_os = \"macos\")]\nuse seyal_protocol::pass8::{\n    BlockKind as WireBlockKind, BlockLifecycle as WireBlockLifecycle, BlockState as WireBlockState,\n};\n""",
)
replace(
    "crates/seyal-runtime/src/block.rs",
    """impl BlockSummary {\n    pub(crate) fn to_wire(self) -> WireBlockState {\n""",
    """impl BlockSummary {\n    #[cfg(target_os = \"macos\")]\n    pub(crate) fn to_wire(self) -> WireBlockState {\n""",
)
replace(
    "crates/seyal-runtime/src/runtime.rs",
    """        #[cfg(not(target_os = \"macos\"))]\n        let _ = block_completion;\n""",
    """        #[cfg(not(target_os = \"macos\"))]\n        match block_completion {\n            BlockCompletion::Completed(record) => {\n                let _ = record;\n            }\n            BlockCompletion::None | BlockCompletion::Failed => {}\n        }\n""",
)
