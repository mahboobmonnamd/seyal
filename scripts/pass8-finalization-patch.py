#!/usr/bin/env python3
from pathlib import Path

root = Path(__file__).resolve().parents[1]


def replace(path: str, old: str, new: str) -> None:
    target = root / path
    text = target.read_text()
    if new in text:
        return
    count = text.count(old)
    if count == 0:
        raise SystemExit(f"missing patch anchor in {path}")
    if count == 1:
        target.write_text(text.replace(old, new, 1))
        return
    if path == "macos/Seyal/Sources/RustDisplayBridge.swift" and old.startswith("      onError(result)"):
        target.write_text(text.replace(old, new))
        return
    raise SystemExit(f"expected one patch anchor in {path}, found {count}")


def insert_before(path: str, marker: str, addition: str, sentinel: str) -> None:
    target = root / path
    text = target.read_text()
    if sentinel in text:
        return
    count = text.count(marker)
    if count == 0:
        raise SystemExit(f"missing insertion anchor in {path}")
    if count == 1:
        target.write_text(text.replace(marker, addition + marker, 1))
        return
    if marker == "}\n":
        index = text.rfind(marker)
        target.write_text(text[:index] + addition + text[index:])
        return
    raise SystemExit(f"expected one insertion anchor in {path}, found {count}")


# ---------------------------------------------------------------------------
# Protocol allocation reconciliation with merged Pass 7.1.
# Pass 7.1 already owns capability bit 4 and control message type 20, so Pass 8
# receives the next independent capability bit and an R->C-only type that stays
# outside MessageType. This preserves the raw fallback while retaining command
# Blocks/composer support.
# ---------------------------------------------------------------------------
replace(
    "crates/seyal-protocol/src/pass8.rs",
    "pub const CAP_BLOCK_METADATA: u32 = 1 << 4;\n/// SPEC-007 R→C message type allocated to `BlockState`.\npub const BLOCK_STATE_MESSAGE_TYPE: u16 = 20;",
    "pub const CAP_BLOCK_METADATA: u32 = 1 << 5;\n/// SPEC-007 R→C message type allocated to `BlockState`. This intentionally\n/// remains outside the bidirectional control `MessageType` enum.\npub const BLOCK_STATE_MESSAGE_TYPE: u16 = 26;",
)
replace(
    "crates/seyal-protocol/src/pass8.rs",
    "fn dedicated_frame_uses_existing_header_and_type_twenty()",
    "fn dedicated_frame_uses_existing_header_and_type_twenty_six()",
)
insert_before(
    "crates/seyal-protocol/src/pass8.rs",
    "    #[test]\n    fn block_state_is_exactly_56_bytes_and_round_trips_little_endian()",
    """    #[test]\n    fn pass8_allocation_is_disjoint_from_pass71_control_surface() {\n        assert_eq!(CAP_BLOCK_METADATA, 1 << 5);\n        assert_eq!(\n            CAP_BLOCK_METADATA & crate::framing::CAP_COMMAND_BLOCKS,\n            0,\n            \"Pass 8 metadata capability must be independently disableable\"\n        );\n        assert_eq!(BLOCK_STATE_MESSAGE_TYPE, 26);\n        assert_eq!(\n            crate::framing::MessageType::from_u16(BLOCK_STATE_MESSAGE_TYPE),\n            None,\n            \"Pass 8 R→C metadata must not become a client→Runtime control message\"\n        );\n    }\n\n""",
    "pass8_allocation_is_disjoint_from_pass71_control_surface",
)

# ---------------------------------------------------------------------------
# Client reconnect compiler fixes and strict bounded quarantine semantics.
# ---------------------------------------------------------------------------
replace(
    "crates/seyal-client/src/local.rs",
    """        let mut stream = connect_stream(&socket_path)?;\n        let mut hello = hello(&mut stream, true, true)?;\n        send_control(&mut stream, MessageType::ListExecutions, &[])?;\n""",
    """        let mut stream = connect_stream(&socket_path)?;\n        let mut server_hello = hello(&mut stream, true, true)?;\n        send_control(&mut stream, MessageType::ListExecutions, &[])?;\n""",
)
replace(
    "crates/seyal-client/src/local.rs",
    """        if is_epoch_quarantined(hello.runtime_id, execution_id) {\n            drop(stream);\n            stream = connect_stream(&socket_path)?;\n            hello = hello(&mut stream, true, false)?;\n        }\n        let block_metadata_negotiated = hello.server_capabilities & CAP_BLOCK_METADATA != 0\n            && !is_epoch_quarantined(hello.runtime_id, execution_id);\n        Self::finish_attach(\n            stream,\n            execution_id,\n            Role::Controller,\n            hello.server_capabilities & CAP_COMMAND_BLOCKS != 0,\n            hello.runtime_id,\n            block_metadata_negotiated,\n        )\n""",
    """        if is_epoch_quarantined(server_hello.runtime_id, execution_id) {\n            drop(stream);\n            stream = connect_stream(&socket_path)?;\n            server_hello = hello(&mut stream, true, false)?;\n        }\n        let block_metadata_negotiated = server_hello.server_capabilities & CAP_BLOCK_METADATA != 0\n            && !is_epoch_quarantined(server_hello.runtime_id, execution_id);\n        Self::finish_attach(\n            stream,\n            execution_id,\n            Role::Controller,\n            server_hello.server_capabilities & CAP_COMMAND_BLOCKS != 0,\n            server_hello.runtime_id,\n            block_metadata_negotiated,\n        )\n""",
)
replace(
    "crates/seyal-client/src/local.rs",
    """        let mut stream = connect_stream(socket_path)?;\n        let mut hello = hello(&mut stream, role == Role::Controller, true)?;\n        if is_epoch_quarantined(hello.runtime_id, execution_id) {\n            drop(stream);\n            stream = connect_stream(socket_path)?;\n            hello = hello(&mut stream, role == Role::Controller, false)?;\n        }\n        let block_metadata_negotiated = hello.server_capabilities & CAP_BLOCK_METADATA != 0\n            && !is_epoch_quarantined(hello.runtime_id, execution_id);\n        Self::finish_attach(\n            stream,\n            execution_id,\n            role,\n            hello.server_capabilities & CAP_COMMAND_BLOCKS != 0,\n            hello.runtime_id,\n            block_metadata_negotiated,\n        )\n""",
    """        let mut stream = connect_stream(socket_path)?;\n        let mut server_hello = hello(&mut stream, role == Role::Controller, true)?;\n        if is_epoch_quarantined(server_hello.runtime_id, execution_id) {\n            drop(stream);\n            stream = connect_stream(socket_path)?;\n            server_hello = hello(&mut stream, role == Role::Controller, false)?;\n        }\n        let block_metadata_negotiated = server_hello.server_capabilities & CAP_BLOCK_METADATA != 0\n            && !is_epoch_quarantined(server_hello.runtime_id, execution_id);\n        Self::finish_attach(\n            stream,\n            execution_id,\n            role,\n            server_hello.server_capabilities & CAP_COMMAND_BLOCKS != 0,\n            server_hello.runtime_id,\n            block_metadata_negotiated,\n        )\n""",
)
replace(
    "crates/seyal-client/src/block.rs",
    """use std::{\n    collections::HashSet,\n    sync::{Mutex, OnceLock},\n};\n""",
    """use std::{\n    collections::{HashSet, VecDeque},\n    sync::{Mutex, OnceLock},\n};\n""",
)
replace(
    "crates/seyal-client/src/block.rs",
    """        if incoming.execution_id != expected_execution {\n            return self.conflict();\n        }\n\n        let Some(current) = self.accepted else {\n""",
    """        if incoming.execution_id != expected_execution {\n            return self.conflict();\n        }\n        if !matches!(\n            (incoming.state, incoming.revision),\n            (BlockLifecycle::Current, 1) | (BlockLifecycle::Completed, 2)\n        ) {\n            return self.conflict();\n        }\n\n        let Some(current) = self.accepted else {\n""",
)
replace(
    "crates/seyal-client/src/block.rs",
    """fn quarantine_set() -> &'static Mutex<HashSet<(u128, ExecutionId)>> {\n    static QUARANTINED: OnceLock<Mutex<HashSet<(u128, ExecutionId)>>> = OnceLock::new();\n    QUARANTINED.get_or_init(|| Mutex::new(HashSet::new()))\n}\n\npub(crate) fn quarantine_epoch(runtime_id: u128, execution_id: ExecutionId) {\n    if let Ok(mut values) = quarantine_set().lock() {\n        values.insert((runtime_id, execution_id));\n    }\n}\n\npub(crate) fn is_epoch_quarantined(runtime_id: u128, execution_id: ExecutionId) -> bool {\n    quarantine_set()\n        .lock()\n        .is_ok_and(|values| values.contains(&(runtime_id, execution_id)))\n}\n""",
    """const MAX_QUARANTINED_EPOCHS: usize = 1024;\n\n#[derive(Default)]\nstruct QuarantineRegistry {\n    values: HashSet<(u128, ExecutionId)>,\n    order: VecDeque<(u128, ExecutionId)>,\n}\n\nfn quarantine_registry() -> &'static Mutex<QuarantineRegistry> {\n    static QUARANTINED: OnceLock<Mutex<QuarantineRegistry>> = OnceLock::new();\n    QUARANTINED.get_or_init(|| Mutex::new(QuarantineRegistry::default()))\n}\n\npub(crate) fn quarantine_epoch(runtime_id: u128, execution_id: ExecutionId) {\n    if let Ok(mut registry) = quarantine_registry().lock() {\n        let epoch = (runtime_id, execution_id);\n        if registry.values.insert(epoch) {\n            registry.order.push_back(epoch);\n        }\n        while registry.order.len() > MAX_QUARANTINED_EPOCHS {\n            if let Some(retired) = registry.order.pop_front() {\n                registry.values.remove(&retired);\n            }\n        }\n    }\n}\n\npub(crate) fn is_epoch_quarantined(runtime_id: u128, execution_id: ExecutionId) -> bool {\n    quarantine_registry()\n        .lock()\n        .is_ok_and(|registry| registry.values.contains(&(runtime_id, execution_id)))\n}\n""",
)
replace(
    "crates/seyal-client/src/block.rs",
    """    #[test]\n    fn stale_metadata_does_not_replace_committed_cache() {\n        let mut cache = BlockCache::default();\n        cache.apply(execution(1), current()).unwrap();\n        cache.apply(execution(1), completed()).unwrap();\n        let mut stale = completed();\n        stale.revision = 1;\n        assert_eq!(cache.apply(execution(1), stale), Ok(BlockApply::Stale));\n        assert_eq!(cache.visible(), Some(completed()));\n    }\n""",
    """    #[test]\n    fn invalid_completed_revision_pair_quarantines_instead_of_looking_stale() {\n        let mut cache = BlockCache::default();\n        cache.apply(execution(1), current()).unwrap();\n        cache.apply(execution(1), completed()).unwrap();\n        let mut invalid = completed();\n        invalid.revision = 1;\n        assert_eq!(\n            cache.apply(execution(1), invalid),\n            Err(BlockCacheError::Conflict)\n        );\n        assert_eq!(cache.visible(), None);\n    }\n""",
)
replace(
    "crates/seyal-client/src/block.rs",
    """        let execution = execution(0xabc);\n        quarantine_epoch(10, execution);\n        assert!(is_epoch_quarantined(10, execution));\n        assert!(!is_epoch_quarantined(11, execution));\n        assert!(!is_epoch_quarantined(10, execution(0xdef)));\n""",
    """        let execution_id = execution(0xabc);\n        quarantine_epoch(10, execution_id);\n        assert!(is_epoch_quarantined(10, execution_id));\n        assert!(!is_epoch_quarantined(11, execution_id));\n        assert!(!is_epoch_quarantined(10, execution(0xdef)));\n""",
)
insert_before(
    "crates/seyal-client/src/block.rs",
    "}\n",
    """\n    #[test]\n    fn quarantine_registry_is_strictly_bounded() {\n        let runtime_id = 0xfeed_u128;\n        for ordinal in 1..=(MAX_QUARANTINED_EPOCHS + 1) {\n            quarantine_epoch(runtime_id, execution(ordinal as u128));\n        }\n        assert!(!is_epoch_quarantined(runtime_id, execution(1)));\n        assert!(is_epoch_quarantined(\n            runtime_id,\n            execution((MAX_QUARANTINED_EPOCHS + 1) as u128)\n        ));\n    }\n""",
    "quarantine_registry_is_strictly_bounded",
)

# ---------------------------------------------------------------------------
# Runtime failure injection is macOS-local and completion encode failure must
# be one global finalization decision, not consumed separately per client.
# ---------------------------------------------------------------------------
replace(
    "crates/seyal-runtime/src/block.rs",
    "#[cfg(feature = \"test-fault-injection\")]\nuse crate::test_fault::{self, FaultPoint};",
    "#[cfg(all(target_os = \"macos\", feature = \"test-fault-injection\"))]\nuse crate::test_fault::{self, FaultPoint};",
)
replace(
    "crates/seyal-runtime/src/block.rs",
    """    pub(crate) fn to_wire(self) -> WireBlockState {\n        #[cfg(feature = \"test-fault-injection\")]\n        let revision = if self.lifecycle == BlockLifecycle::Completed\n            && test_fault::take(FaultPoint::BlockCompletionEncode)\n        {\n            0\n        } else {\n            self.revision\n        };\n        #[cfg(not(feature = \"test-fault-injection\"))]\n        let revision = self.revision;\n\n        WireBlockState {\n            execution_id: self.execution_id,\n            block_id: self.id,\n            revision,\n""",
    """    pub(crate) fn to_wire(self) -> WireBlockState {\n        WireBlockState {\n            execution_id: self.execution_id,\n            block_id: self.id,\n            revision: self.revision,\n""",
)
replace(
    "crates/seyal-runtime/src/block.rs",
    """    InvalidTransition,\n    InjectedFailure,\n""",
    """    InvalidTransition,\n    #[cfg(all(target_os = \"macos\", feature = \"test-fault-injection\"))]\n    InjectedFailure,\n""",
)
replace(
    "crates/seyal-runtime/src/block.rs",
    "#[cfg(feature = \"test-fault-injection\")]\n        if test_fault::take(FaultPoint::BlockAdmission)",
    "#[cfg(all(target_os = \"macos\", feature = \"test-fault-injection\"))]\n        if test_fault::take(FaultPoint::BlockAdmission)",
)
replace(
    "crates/seyal-runtime/src/block.rs",
    "#[cfg(feature = \"test-fault-injection\")]\n        if test_fault::take(FaultPoint::BlockCompletionMutation)",
    "#[cfg(all(target_os = \"macos\", feature = \"test-fault-injection\"))]\n        if test_fault::take(FaultPoint::BlockCompletionMutation)",
)
insert_before(
    "crates/seyal-runtime/src/block.rs",
    "}\n",
    """\n    #[test]\n    fn ten_thousand_execution_churn_retires_every_record_and_never_reuses_identity() {\n        use std::collections::HashSet;\n\n        let workspace = WorkspaceId::m001_default();\n        let mut timeline = BlockTimeline::default();\n        let mut ids = HashSet::with_capacity(10_000);\n        for ordinal in 1..=10_000_u128 {\n            let execution_id = execution(ordinal);\n            let current = timeline\n                .admit(workspace, execution_id, ordinal as u64)\n                .unwrap();\n            assert!(ids.insert(current.id));\n            let completed = timeline.complete(workspace, execution_id).unwrap().unwrap();\n            assert_eq!(completed.id, current.id);\n            assert_eq!(timeline.retire(execution_id), Some(completed));\n            assert_eq!(timeline.len(), 0);\n        }\n        assert_eq!(ids.len(), 10_000);\n    }\n""",
    "ten_thousand_execution_churn_retires_every_record",
)

replace(
    "crates/seyal-runtime/src/runtime/local.rs",
    """        for (token, block_capable) in notifications {\n            if block_capable {\n                match block_completion {\n                    BlockCompletion::Completed(record) => {\n                        let Ok(frame) = encode_block_state_frame(&record.to_wire()) else {\n                            self.close_local_connection(token);\n                            continue;\n                        };\n                        #[cfg(feature = \"test-fault-injection\")]\n                        if test_fault::take(FaultPoint::BlockCompletionAdmission) {\n                            self.close_local_connection(token);\n                            continue;\n                        }\n                        if !self.send_after_display_frame(token, frame) {\n                            continue;\n                        }\n                    }\n                    BlockCompletion::Failed => {\n                        self.close_local_connection(token);\n                        continue;\n                    }\n                    BlockCompletion::None => {}\n                }\n            }\n\n            let message = framing::LifecycleMessage {\n""",
    """        let completion_frame = match block_completion {\n            BlockCompletion::Completed(record) => {\n                #[cfg(feature = \"test-fault-injection\")]\n                if test_fault::take(FaultPoint::BlockCompletionEncode) {\n                    Err(())\n                } else {\n                    encode_block_state_frame(&record.to_wire()).map(Some).map_err(|_| ())\n                }\n                #[cfg(not(feature = \"test-fault-injection\"))]\n                {\n                    encode_block_state_frame(&record.to_wire()).map(Some).map_err(|_| ())\n                }\n            }\n            BlockCompletion::Failed => Err(()),\n            BlockCompletion::None => Ok(None),\n        };\n\n        for (token, block_capable) in notifications {\n            if block_capable {\n                match &completion_frame {\n                    Ok(Some(frame)) => {\n                        #[cfg(feature = \"test-fault-injection\")]\n                        if test_fault::take(FaultPoint::BlockCompletionAdmission) {\n                            self.close_local_connection(token);\n                            continue;\n                        }\n                        if !self.send_after_display_frame(token, frame.clone()) {\n                            continue;\n                        }\n                    }\n                    Err(()) => {\n                        self.close_local_connection(token);\n                        continue;\n                    }\n                    Ok(None) => {}\n                }\n            }\n\n            let message = framing::LifecycleMessage {\n""",
)

# Make failure timing deterministic and prove a global encode failure closes
# every negotiated Block-capable client before Finalized.
replace(
    "crates/seyal-runtime/tests/pass8_block_failures.rs",
    'CommandSpec::new("/bin/sh").args(["-c", "sleep 0.05; printf TAIL"]),',
    'CommandSpec::new("/bin/sh").args(["-c", "sleep 0.25; printf TAIL"]),',
)
insert_before(
    "crates/seyal-runtime/tests/pass8_block_failures.rs",
    "#[test]\nfn completion_admission_failure_disconnects_before_finalized_and_retires_block()",
    """#[test]\nfn completion_encode_failure_is_global_across_all_block_capable_clients() {\n    let mut harness = Harness::new(\"encode-multi\");\n    let execution_id = harness.spawn();\n    let mut first = harness.connect();\n    let mut second = harness.connect();\n    harness.attach_until_current(&mut first, execution_id);\n    harness.attach_until_current(&mut second, execution_id);\n\n    test_fault::fail_next(FaultPoint::BlockCompletionEncode);\n    harness.assert_fails_closed_before_finalized(&mut first);\n    harness.assert_fails_closed_before_finalized(&mut second);\n}\n\n""",
    "completion_encode_failure_is_global_across_all_block_capable_clients",
)

# ---------------------------------------------------------------------------
# Fuzz registry validator and exact-head documentation authority.
# ---------------------------------------------------------------------------
replace(
    "scripts/test-harnesses.py",
    '        "display-binary-decode",\n    }',
    '        "display-binary-decode",\n        "block-state-decode",\n    }',
)
replace(
    "docs/specs/SPEC-007-M001-BLOCKS.md",
    "- **Status:** Proposed for M001 Pass 8; refinement only, production implementation blocked",
    "- **Status:** Accepted for M001 Pass 8; production implementation authorized by #715; wire allocation reconciled with merged Pass 7.1 in PR #721",
)
replace(
    "docs/specs/SPEC-007-M001-BLOCKS.md",
    "- **Issue:** #708",
    "- **Refinement issue:** #708\n- **Implementation issue:** #715",
)
replace(
    "docs/specs/SPEC-007-M001-BLOCKS.md",
    "- **Depends on:** SPEC-001, SPEC-003, SPEC-004, SPEC-005 and accepted SPEC-006; production implementation additionally requires Pass 7 implementation #706 / PR #707 to be merged and accepted",
    "- **Depends on:** SPEC-001, SPEC-003, SPEC-004, SPEC-005 and accepted SPEC-006; Pass 7/7.1 production behavior incorporated on master before PR #721",
)
replace(
    "docs/specs/SPEC-007-M001-BLOCKS.md",
    "Allocate capability bit 4:\n\n```text\nCAP_BLOCK_METADATA = 1 << 4\n```",
    "Allocate capability bit 5. Pass 7.1 owns bit 4 (`CAP_COMMAND_BLOCKS`), so the Pass 8 capability must remain independently negotiable and independently disableable during raw fallback:\n\n```text\nCAP_BLOCK_METADATA = 1 << 5\n```",
)
replace(
    "docs/specs/SPEC-007-M001-BLOCKS.md",
    "```text\n20  R→C  BlockState\n```",
    "```text\n26  R→C  BlockState\n```\n\nMessage type 26 is intentionally not a `MessageType` control-enum member. Pass 7.1 already owns client→Runtime type 20 (`ComposerCommand`), and a Pass 8 metadata frame must therefore remain directionally R→C only.",
)
replace(
    "docs/specs/SPEC-007-M001-BLOCKS.md",
    "Later trusted shell integration may introduce command Blocks through a later accepted contract.",
    "Merged Pass 7.1 may independently project trusted-shell per-command Blocks. Those command records do not create, replace, or amplify the one coarse Pass 8 execution metadata record defined here.",
)
replace(
    "docs/specs/README.md",
    "- [`SPEC-007-M001-BLOCKS.md`](SPEC-007-M001-BLOCKS.md) — **Proposed for M001 Pass 8 / Issue #708:** durable Workspace-owned `BlockId`, exact Workspace/Execution association, immutable `BeforeLine(LineId)` primary-history anchor, monotonic final-drain-driven `Current → Completed`, mandatory completed-record retirement, exact final-display → `BlockState::Completed` → `Lifecycle::Finalized` ordering, bounded read-only local projection, deterministic malformed/conflicting-metadata quarantine/raw-terminal recovery and strict no-hot-path/no-copied-transcript constraints. Production implementation remains `NOT_READY` until all SPEC-007 dependency gates pass.",
    "- [`SPEC-007-M001-BLOCKS.md`](SPEC-007-M001-BLOCKS.md) — **Accepted for M001 Pass 8 / implementation Issue #715:** durable Workspace-owned `BlockId`, exact Workspace/Execution association, immutable `BeforeLine(LineId)` primary-history anchor, monotonic final-drain-driven `Current → Completed`, mandatory completed-record retirement, exact final-display → `BlockState::Completed` → `Lifecycle::Finalized` ordering, bounded read-only local projection, deterministic malformed/conflicting-metadata quarantine/raw-terminal recovery and strict no-hot-path/no-copied-transcript constraints. PR #721 reconciles the read-only projection to capability bit 5 / R→C type 26 because merged Pass 7.1 owns bit 4 / client→Runtime type 20.",
)

# ---------------------------------------------------------------------------
# Native recovery/consumption and ABI evidence.
# ---------------------------------------------------------------------------
replace(
    "macos/Seyal/Sources/RustDisplayBridge.swift",
    """  func stop() {\n    reconnectRequested = false\n""",
    """  func stop(reconnect: Bool = false) {\n    reconnectRequested = reconnect\n""",
)
replace(
    "macos/Seyal/Sources/RustDisplayBridge.swift",
    """    if result == -3 || result == -10 || result == -18 {\n      onError(result)\n      stop()\n    }\n""",
    """    if result == -18 {\n      onError(result)\n      stop(reconnect: true)\n    } else if result == -3 || result == -10 {\n      onError(result)\n      stop()\n    }\n""",
)
replace(
    "macos/Seyal/Sources/RustDisplayBridge.swift",
    """      onError(result)\n      stop()\n      return\n""",
    """      onError(result)\n      stop(reconnect: result == -18)\n      return\n""",
)
replace(
    "macos/Seyal/Sources/MetalSurfaceView.swift",
    """  private(set) var lastBridgeError: Int32?\n  private(set) var lastRenderError: Error?\n""",
    """  private(set) var lastBridgeError: Int32?\n  private(set) var lastRenderError: Error?\n  private(set) var runtimeBlockMetadata: RuntimeBlockMetadata?\n""",
)
replace(
    "macos/Seyal/Sources/MetalSurfaceView.swift",
    """    onFrameChanged?(frame)\n    if lastAlternateScreen != frame.alternateScreen {\n""",
    """    runtimeBlockMetadata = bridge?.currentBlockMetadata()\n    onFrameChanged?(frame)\n    if lastAlternateScreen != frame.alternateScreen {\n""",
)
insert_before(
    "crates/seyal-client/src/ffi.rs",
    "#[derive(Clone, Copy, Debug)]\n#[repr(C)]\npub struct SeyalBlockRecord",
    """#[cfg(test)]\nmod pass8_execution_block_abi_tests {\n    use std::mem::{align_of, offset_of, size_of};\n\n    use super::SeyalExecutionBlockMetadata;\n\n    #[test]\n    fn execution_block_metadata_c_abi_is_exactly_40_bytes() {\n        assert_eq!(size_of::<SeyalExecutionBlockMetadata>(), 40);\n        assert_eq!(align_of::<SeyalExecutionBlockMetadata>(), 8);\n        assert_eq!(offset_of!(SeyalExecutionBlockMetadata, block_id_low), 0);\n        assert_eq!(offset_of!(SeyalExecutionBlockMetadata, block_id_high), 8);\n        assert_eq!(offset_of!(SeyalExecutionBlockMetadata, revision), 16);\n        assert_eq!(offset_of!(SeyalExecutionBlockMetadata, start_line_id), 24);\n        assert_eq!(offset_of!(SeyalExecutionBlockMetadata, state), 32);\n        assert_eq!(offset_of!(SeyalExecutionBlockMetadata, reserved), 33);\n    }\n}\n\n""",
    "execution_block_metadata_c_abi_is_exactly_40_bytes",
)
insert_before(
    "macos/Seyal/Tests/SeyalTests/SeyalShellComponentTests.swift",
    "  func testPaneQualifiedIdentitiesDoNotCollideAcrossPanes()",
    """  func testExecutionBlockMetadataCABIIsStable() {\n    XCTAssertEqual(MemoryLayout<SeyalExecutionBlockMetadata>.size, 40)\n    XCTAssertEqual(MemoryLayout<SeyalExecutionBlockMetadata>.stride, 40)\n    XCTAssertEqual(MemoryLayout<SeyalExecutionBlockMetadata>.alignment, 8)\n  }\n\n""",
    "testExecutionBlockMetadataCABIIsStable",
)

# ---------------------------------------------------------------------------
# Benchmark idle CPU is measured as a delta across the actual 500 ms idle
# interval rather than reporting process-lifetime CPU from ps.
# ---------------------------------------------------------------------------
replace(
    "crates/seyal-client/benches/pass8_block_metadata.rs",
    """    thread::sleep(Duration::from_millis(500));\n    let idle = process_metrics();\n""",
    """    let idle_cpu_start = process_cpu_seconds();\n    let idle_started = Instant::now();\n    thread::sleep(Duration::from_millis(500));\n    let idle_elapsed = idle_started.elapsed().as_secs_f64();\n    let idle_cpu_end = process_cpu_seconds();\n    let idle_cpu_percent = if idle_elapsed > 0.0 {\n        ((idle_cpu_end - idle_cpu_start).max(0.0) / idle_elapsed) * 100.0\n    } else {\n        0.0\n    };\n    let idle = process_metrics();\n""",
)
replace(
    "crates/seyal-client/benches/pass8_block_metadata.rs",
    """        idle.cpu_percent,\n""",
    """        idle_cpu_percent,\n""",
)
insert_before(
    "crates/seyal-client/benches/pass8_block_metadata.rs",
    "#[cfg(target_os = \"macos\")]\nfn print_host_metadata()",
    """#[cfg(target_os = \"macos\")]\nfn process_cpu_seconds() -> f64 {\n    let pid = process::id();\n    let output = Command::new(\"/bin/ps\")\n        .args([\"-o\", \"time=\", \"-p\", &pid.to_string()])\n        .output()\n        .expect(\"ps cpu time\");\n    parse_cpu_time(String::from_utf8_lossy(&output.stdout).trim())\n}\n\n#[cfg(target_os = \"macos\")]\nfn parse_cpu_time(value: &str) -> f64 {\n    let fields = value.split(':').collect::<Vec<_>>();\n    match fields.as_slice() {\n        [minutes, seconds] => {\n            minutes.parse::<f64>().unwrap_or(0.0) * 60.0\n                + seconds.parse::<f64>().unwrap_or(0.0)\n        }\n        [hours, minutes, seconds] => {\n            hours.parse::<f64>().unwrap_or(0.0) * 3600.0\n                + minutes.parse::<f64>().unwrap_or(0.0) * 60.0\n                + seconds.parse::<f64>().unwrap_or(0.0)\n        }\n        _ => 0.0,\n    }\n}\n\n""",
    "fn process_cpu_seconds()",
)
