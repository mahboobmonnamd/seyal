# Pass 9 merge-acceptance evidence

- **Issue:** #735
- **Implementation PR:** #734
- **Exact production head under test:** `8a8869f9f2325a0abb4cc0813af189ba2d5ae770`
- **Artifact:** `pass9-merge-acceptance-8a8869f9f232.json`
- **Modes:** graceful_detach, abrupt_socket_loss (`socket_shutdown_owned_disconnect` fault injection; not GUI-process death)
- **Cycles:** 100 each after 5 warmups
- **Geometry:** 120x40 applied via `proposeGeometry` + observed frame shape
- **Topology:** `RustDisplayBridge` + `RuntimeLifecycleRecoveryCoordinator` + `MetalTerminalRenderer` prepare/release (same boundary as `MetalSurfaceView.consumeBridgeFrame`; not full AppKit window/CAMetalLayer present)
- **Renderer proof:** non-vacuous `renderer_*_peak_connected >= 1` with quiescent exact return to 0
- **Client RSS soft gate:** Debug Metal shared-atlas reclaim noise (48 MiB); logical renderer fields remain hard exact-return
- **Validator:** `python3 scripts/check-pass9-merge-acceptance.py --expected-head 8a8869f9f2325a0abb4cc0813af189ba2d5ae770 docs/evidence/pass9-merge-acceptance-8a8869f9f232.json`

Independent reviews remain required before merge of #734. This report is evidence only; merging #743 must not auto-merge #734.
