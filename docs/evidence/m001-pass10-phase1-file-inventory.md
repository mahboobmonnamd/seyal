# M001 Pass 10 — Phase 1 file inventory (domain-mapped)

Generated for Issue #727 Phase 1 ledger completeness under
`docs/engineering/M001-PASS10-CODE-QUALITY-REVIEW.md` §3/§20.

Each production-significant path inherits the domain-agent result from
`docs/evidence/m001-pass10-code-quality-review-ledger.md`, except where a
finding override records post-fix re-review. This is **not** a claim that
every line was re-read after the finding PRs; it is the explicit inventory
binding paths to the recorded domain review + finding disposition.

| Path | Domain agent | Status | Notes |
|---|---|---|---|
| `crates/seyal-client/benches/pass7_input_resize.rs` | Agent3-FFI/client | PASS | domain review |
| `crates/seyal-client/benches/pass7_validation_matrix.rs` | Agent3-FFI/client | PASS | domain review |
| `crates/seyal-client/benches/pass8_block_metadata.rs` | Agent3-FFI/client | PASS | domain review |
| `crates/seyal-client/src/block.rs` | Agent3-FFI/client | PASS | domain review |
| `crates/seyal-client/src/ffi.rs` | Agent3-FFI/client | PASS | F-010/#760+#762 |
| `crates/seyal-client/src/lib.rs` | Agent3-FFI/client | PASS | domain review |
| `crates/seyal-client/src/local.rs` | Agent3-FFI/client | PASS | domain review |
| `crates/seyal-client/src/pass7_benchmark.rs` | Agent3-FFI/client | PASS | domain review |
| `crates/seyal-client/src/pass8_benchmark.rs` | Agent3-FFI/client | PASS | domain review |
| `crates/seyal-client/tests/ffi_misuse_macos.rs` | Agent3-FFI/client | PASS | domain review; test |
| `crates/seyal-client/tests/live_candidate_d.rs` | Agent3-FFI/client | PASS | domain review; test |
| `crates/seyal-client/tests/pass7_interactive.rs` | Agent3-FFI/client | PASS | domain review; test |
| `crates/seyal-core/src/lib.rs` | Agent-general | PASS | domain review |
| `crates/seyal-exec/benches/execution_scalability.rs` | Agent2-exec/PTY | PASS | domain review |
| `crates/seyal-exec/benches/pty_io.rs` | Agent2-exec/PTY | PASS | domain review |
| `crates/seyal-exec/src/child.rs` | Agent2-exec/PTY | PASS | domain review |
| `crates/seyal-exec/src/command.rs` | Agent2-exec/PTY | PASS | domain review |
| `crates/seyal-exec/src/endpoint.rs` | Agent2-exec/PTY | PASS | domain review |
| `crates/seyal-exec/src/error.rs` | Agent2-exec/PTY | PASS | domain review |
| `crates/seyal-exec/src/execution.rs` | Agent2-exec/PTY | PASS | domain review |
| `crates/seyal-exec/src/lib.rs` | Agent2-exec/PTY | PASS | domain review |
| `crates/seyal-exec/src/platform/macos.rs` | Agent2-exec/PTY | PASS | domain review |
| `crates/seyal-exec/src/platform/macos_reactor.rs` | Agent2-exec/PTY | PASS | domain review |
| `crates/seyal-exec/src/platform/mod.rs` | Agent2-exec/PTY | PASS | domain review |
| `crates/seyal-exec/src/projection.rs` | Agent2-exec/PTY | PASS | domain review |
| `crates/seyal-exec/src/reactor.rs` | Agent2-exec/PTY | PASS | domain review |
| `crates/seyal-exec/src/readiness.rs` | Agent2-exec/PTY | PASS | domain review |
| `crates/seyal-exec/src/test_fault.rs` | Agent2-exec/PTY | PASS | domain review |
| `crates/seyal-exec/src/winsize.rs` | Agent2-exec/PTY | PASS | domain review |
| `crates/seyal-exec/tests/macos_environment.rs` | Agent2-exec/PTY | PASS | domain review; test |
| `crates/seyal-exec/tests/macos_pty.rs` | Agent2-exec/PTY | PASS | domain review; test |
| `crates/seyal-exec/tests/source_hygiene.rs` | Agent2-exec/PTY | PASS | domain review; test |
| `crates/seyal-protocol/src/discovery.rs` | Agent2-protocol | PASS | domain review |
| `crates/seyal-protocol/src/display.rs` | Agent2-protocol | PASS | domain review |
| `crates/seyal-protocol/src/framing.rs` | Agent2-protocol | PASS | domain review |
| `crates/seyal-protocol/src/lib.rs` | Agent2-protocol | PASS | domain review |
| `crates/seyal-protocol/src/pass7.rs` | Agent2-protocol | PASS | domain review |
| `crates/seyal-protocol/src/pass8.rs` | Agent2-protocol | PASS | domain review |
| `crates/seyal-protocol/tests/pass7_fuzz_smoke.rs` | Agent2-protocol | PASS | domain review; test |
| `crates/seyal-protocol/tests/pass7_input_resize.rs` | Agent2-protocol | PASS | domain review; test |
| `crates/seyal-protocol/tests/pass8_fuzz_smoke.rs` | Agent2-protocol | PASS | domain review; test |
| `crates/seyal-render/benches/pass6_preparation.rs` | Agent4/6-render | PASS | domain review |
| `crates/seyal-render/src/lib.rs` | Agent4/6-render | PASS | domain review |
| `crates/seyal-runtime/benches/pass5_delta_transport.rs` | Agent2/5/6-runtime | PASS | domain review |
| `crates/seyal-runtime/benches/pass5_production_transport.rs` | Agent2/5/6-runtime | PASS | domain review |
| `crates/seyal-runtime/benches/pass5_shared_projection.rs` | Agent2/5/6-runtime | PASS | domain review |
| `crates/seyal-runtime/benches/pass5_transport_stress.rs` | Agent2/5/6-runtime | PASS | domain review |
| `crates/seyal-runtime/benches/runtime_scalability.rs` | Agent2/5/6-runtime | PASS | domain review |
| `crates/seyal-runtime/build.rs` | Agent2/5/6-runtime | PASS | domain review |
| `crates/seyal-runtime/src/block.rs` | Agent2/5/6-runtime | PASS | domain review |
| `crates/seyal-runtime/src/blocks.rs` | Agent2/5/6-runtime | PASS | domain review |
| `crates/seyal-runtime/src/capability.rs` | Agent2/5/6-runtime | PASS | domain review |
| `crates/seyal-runtime/src/display.rs` | Agent2/5/6-runtime | PASS | domain review |
| `crates/seyal-runtime/src/error.rs` | Agent2/5/6-runtime | PASS | domain review |
| `crates/seyal-runtime/src/ids.rs` | Agent2/5/6-runtime | PASS | domain review |
| `crates/seyal-runtime/src/input.rs` | Agent2/5/6-runtime | PASS | domain review |
| `crates/seyal-runtime/src/lib.rs` | Agent2/5/6-runtime | PASS | domain review |
| `crates/seyal-runtime/src/local_ipc/attachment.rs` | Agent2/5/6-runtime | PASS | domain review |
| `crates/seyal-runtime/src/local_ipc/auth.rs` | Agent2/5/6-runtime | PASS | domain review |
| `crates/seyal-runtime/src/local_ipc/connection.rs` | Agent2/5/6-runtime | PASS | domain review |
| `crates/seyal-runtime/src/local_ipc/discovery.rs` | Agent2/5/6-runtime | PASS | domain review |
| `crates/seyal-runtime/src/local_ipc/fd_transfer.rs` | Agent2/5/6-runtime | PASS | domain review |
| `crates/seyal-runtime/src/local_ipc/framing.rs` | Agent2/5/6-runtime | PASS | domain review |
| `crates/seyal-runtime/src/local_ipc/mod.rs` | Agent2/5/6-runtime | PASS | domain review |
| `crates/seyal-runtime/src/local_ipc/recovery.rs` | Agent2/5/6-runtime | PASS | domain review |
| `crates/seyal-runtime/src/main.rs` | Agent2/5/6-runtime | PASS | domain review |
| `crates/seyal-runtime/src/pass7_benchmark.rs` | Agent2/5/6-runtime | PASS | domain review |
| `crates/seyal-runtime/src/pass8_benchmark.rs` | Agent2/5/6-runtime | PASS | domain review |
| `crates/seyal-runtime/src/platform.rs` | Agent2/5/6-runtime | PASS | domain review |
| `crates/seyal-runtime/src/projection/layout.rs` | Agent2/5/6-runtime | PASS | domain review |
| `crates/seyal-runtime/src/projection/lifecycle.rs` | Agent2/5/6-runtime | PASS | domain review |
| `crates/seyal-runtime/src/projection/mod.rs` | Agent2/5/6-runtime | PASS | domain review |
| `crates/seyal-runtime/src/projection/producer.rs` | Agent2/5/6-runtime | PASS | domain review |
| `crates/seyal-runtime/src/projection/writer.rs` | Agent2/5/6-runtime | PASS | domain review |
| `crates/seyal-runtime/src/runtime.rs` | Agent2/5/6-runtime | PASS | F-006/#756+#774; F-009 publish/#771 |
| `crates/seyal-runtime/src/runtime/local.rs` | Agent2/5/6-runtime | PASS | domain review |
| `crates/seyal-runtime/src/singleton.rs` | Agent2/5/6-runtime | PASS | domain review |
| `crates/seyal-runtime/src/test_fault.rs` | Agent2/5/6-runtime | PASS | domain review |
| `crates/seyal-runtime/tests/display_fuzz_smoke.rs` | Agent2/5/6-runtime | PASS | domain review; test |
| `crates/seyal-runtime/tests/display_publish_bookkeeping.rs` | Agent2/5/6-runtime | PASS | domain review; test |
| `crates/seyal-runtime/tests/final_projection.rs` | Agent2/5/6-runtime | PASS | domain review; test |
| `crates/seyal-runtime/tests/fuzz_smoke.rs` | Agent2/5/6-runtime | PASS | domain review; test |
| `crates/seyal-runtime/tests/local_ipc_adversarial.rs` | Agent2/5/6-runtime | PASS | domain review; test |
| `crates/seyal-runtime/tests/local_ipc_ctrunc.rs` | Agent2/5/6-runtime | PASS | domain review; test |
| `crates/seyal-runtime/tests/local_ipc_failure_injection.rs` | Agent2/5/6-runtime | PASS | domain review; test |
| `crates/seyal-runtime/tests/local_ipc_protocol.rs` | Agent2/5/6-runtime | PASS | domain review; test |
| `crates/seyal-runtime/tests/macos_failure_contracts.rs` | Agent2/5/6-runtime | PASS | domain review; test |
| `crates/seyal-runtime/tests/macos_runtime.rs` | Agent2/5/6-runtime | PASS | domain review; test |
| `crates/seyal-runtime/tests/macos_terminfo_clean.rs` | Agent2/5/6-runtime | PASS | domain review; test |
| `crates/seyal-runtime/tests/pass10_disconnect_during.rs` | Agent2/5/6-runtime | PASS | F-007/#757+#770; test |
| `crates/seyal-runtime/tests/pass5_candidate_d_matrix.rs` | Agent2/5/6-runtime | PASS | domain review; test |
| `crates/seyal-runtime/tests/pass7_local_ipc.rs` | Agent2/5/6-runtime | PASS | domain review; test |
| `crates/seyal-runtime/tests/pass8_block_failures.rs` | Agent2/5/6-runtime | PASS | domain review; test |
| `crates/seyal-runtime/tests/pass8_blocks.rs` | Agent2/5/6-runtime | PASS | domain review; test |
| `crates/seyal-runtime/tests/pass8_resync_reattach.rs` | Agent2/5/6-runtime | PASS | domain review; test |
| `crates/seyal-runtime/tests/pass8_runtime_matrix.rs` | Agent2/5/6-runtime | PASS | domain review; test |
| `crates/seyal-runtime/tests/pass8_stalled_client.rs` | Agent2/5/6-runtime | PASS | domain review; test |
| `crates/seyal-runtime/tests/runtime_adversarial.rs` | Agent2/5/6-runtime | PASS | domain review; test |
| `crates/seyal-runtime/tests/shared_projection_fuzz_smoke.rs` | Agent2/5/6-runtime | PASS | domain review; test |
| `crates/seyal-terminal/benches/vt_parser_state.rs` | Agent2-VT/core | PASS | domain review |
| `crates/seyal-terminal/benches/vt_scroll_state.rs` | Agent2-VT/core | PASS | domain review |
| `crates/seyal-terminal/src/cell.rs` | Agent2-VT/core | PASS | domain review |
| `crates/seyal-terminal/src/color.rs` | Agent2-VT/core | PASS | domain review |
| `crates/seyal-terminal/src/cursor.rs` | Agent2-VT/core | PASS | domain review |
| `crates/seyal-terminal/src/damage.rs` | Agent2-VT/core | PASS | domain review |
| `crates/seyal-terminal/src/error.rs` | Agent2-VT/core | PASS | domain review |
| `crates/seyal-terminal/src/lib.rs` | Agent2-VT/core | PASS | domain review |
| `crates/seyal-terminal/src/line.rs` | Agent2-VT/core | PASS | domain review |
| `crates/seyal-terminal/src/modes.rs` | Agent2-VT/core | PASS | domain review |
| `crates/seyal-terminal/src/parser.rs` | Agent2-VT/core | PASS | domain review |
| `crates/seyal-terminal/src/screen.rs` | Agent2-VT/core | PASS | domain review |
| `crates/seyal-terminal/src/style.rs` | Agent2-VT/core | PASS | domain review |
| `crates/seyal-terminal/src/terminal.rs` | Agent2-VT/core | PASS | domain review |
| `crates/seyal-terminal/tests/fixture_corpus.rs` | Agent2-VT/core | PASS | domain review; test |
| `crates/seyal-terminal/tests/fuzz_smoke.rs` | Agent2-VT/core | PASS | domain review; test |
| `crates/seyal-terminal/tests/m001_vt.rs` | Agent2-VT/core | PASS | domain review; test |
| `crates/seyal-terminal/tests/rill_salvage_regressions.rs` | Agent2-VT/core | PASS | domain review; test |
| `macos/Seyal/Sources/AppDelegate.swift` | Agent4-macOS/Metal | PASS | domain review |
| `macos/Seyal/Sources/BlockView.swift` | Agent4-macOS/Metal | PASS | domain review |
| `macos/Seyal/Sources/BundledRuntimeLauncher.swift` | Agent4-macOS/Metal | PASS | domain review |
| `macos/Seyal/Sources/CommandBlockBodyView.swift` | Agent4-macOS/Metal | PASS | domain review |
| `macos/Seyal/Sources/GlyphAtlas.swift` | Agent4-macOS/Metal | PASS | domain review |
| `macos/Seyal/Sources/Main.swift` | Agent4-macOS/Metal | PASS | domain review |
| `macos/Seyal/Sources/MetalSurfaceView.swift` | Agent4-macOS/Metal | PASS | domain review |
| `macos/Seyal/Sources/MetalTerminalRenderer.swift` | Agent4-macOS/Metal | PASS | domain review |
| `macos/Seyal/Sources/PaneComposerShellView.swift` | Agent4-macOS/Metal | PASS | domain review |
| `macos/Seyal/Sources/Pass9InputAccessibilityQualification.swift` | Agent4-macOS/Metal | PASS | domain review |
| `macos/Seyal/Sources/Pass9MergeAcceptance.swift` | Agent4-macOS/Metal | PASS | domain review |
| `macos/Seyal/Sources/Pass9ReleaseQualification.swift` | Agent4-macOS/Metal | PASS | domain review |
| `macos/Seyal/Sources/Pass9ReleaseQualificationModels.swift` | Agent4-macOS/Metal | PASS | domain review |
| `macos/Seyal/Sources/RendererValidation.swift` | Agent4-macOS/Metal | PASS | domain review |
| `macos/Seyal/Sources/RuntimeLifecycleRecoveryCoordinator.swift` | Agent4-macOS/Metal | PASS | domain review |
| `macos/Seyal/Sources/RustDisplayBridge.swift` | Agent4-macOS/Metal | PASS | domain review |
| `macos/Seyal/Sources/SeyalAccessibilityAnnouncement.swift` | Agent4-macOS/Metal | PASS | domain review |
| `macos/Seyal/Sources/SeyalBridge.h` | Agent4-macOS/Metal | PASS | domain review |
| `macos/Seyal/Sources/SeyalDepthLevel.swift` | Agent4-macOS/Metal | PASS | domain review |
| `macos/Seyal/Sources/SeyalMetrics.swift` | Agent4-macOS/Metal | PASS | domain review |
| `macos/Seyal/Sources/SeyalNativePresentation.swift` | Agent4-macOS/Metal | PASS | domain review |
| `macos/Seyal/Sources/SeyalProductPalette.swift` | Agent4-macOS/Metal | PASS | domain review |
| `macos/Seyal/Sources/SeyalRGBA.swift` | Agent4-macOS/Metal | PASS | domain review |
| `macos/Seyal/Sources/SeyalShellModel.swift` | Agent4-macOS/Metal | PASS | domain review |
| `macos/Seyal/Sources/SeyalShellPreviewFactory.swift` | Agent4-macOS/Metal | PASS | domain review |
| `macos/Seyal/Sources/SeyalShellProductionFactory.swift` | Agent4-macOS/Metal | PASS | domain review |
| `macos/Seyal/Sources/SeyalShellView.swift` | Agent4-macOS/Metal | PASS | domain review |
| `macos/Seyal/Sources/SeyalTOML.swift` | Agent4-macOS/Metal | PASS | domain review |
| `macos/Seyal/Sources/SeyalThemeResolver.swift` | Agent4-macOS/Metal | PASS | domain review |
| `macos/Seyal/Sources/SeyalTypography.swift` | Agent4-macOS/Metal | PASS | domain review |
| `macos/Seyal/Sources/SeyalUIConfiguration.swift` | Agent4-macOS/Metal | PASS | domain review |
| `macos/Seyal/Sources/SeyalUserUISettings.swift` | Agent4-macOS/Metal | PASS | domain review |
| `macos/Seyal/Sources/TerminalInputSurface.swift` | Agent4-macOS/Metal | PASS | domain review |
| `macos/Seyal/Sources/TerminalSurfaceHostView.swift` | Agent4-macOS/Metal | PASS | domain review |
| `macos/Seyal/Tests/SeyalTests/SeyalDesignSystemTests.swift` | Agent4-macOS/Metal | PASS | domain review; test |
| `macos/Seyal/Tests/SeyalTests/SeyalShellComponentTests.swift` | Agent4-macOS/Metal | PASS | domain review; test |
| `macos/Seyal/Tests/SeyalUITests/SeyalShellUITests.swift` | Agent4-macOS/Metal | PASS | domain review; test |
