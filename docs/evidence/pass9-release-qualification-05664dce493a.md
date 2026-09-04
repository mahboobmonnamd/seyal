# Pass 9 release-qualification evidence

- **Issue:** #736
- **Exact production head under test:** `05664dce493abeafa257dddc3c524b11ac74924a`
- **Artifact:** `pass9-release-qualification-05664dce493a.json`
- **Packaging:** `pass9-release-packaging-05664dce493a.md`
- **Input/accessibility:** `pass9-input-accessibility-05664dce493a.json`
- **Pass 8:** `pass9-pass8-attribution-05664dce493a.log` (cohorts=7, paired_delta_median_percent=2.96)
- **Modes:** graceful_detach abrupt_socket_loss
- **Geometries:** 120x40 80x24
- **Cohorts:** 1 2 3 4 5
- **Cycles:** 100 each after 20 warmups
- **Topology:** Release `RustDisplayBridge` + `RuntimeLifecycleRecoveryCoordinator` + `MetalTerminalRenderer` prepare/release with production `InteractiveMetalSurfaceView` SPEC-009 §10 native interaction restore before Usable.
- **Issue relationship:** Refs #736 until independent maintainer review confirms DoD; packaging uses Team-identity Release.
- **Abrupt fault:** `socket_shutdown_owned_disconnect`
- **Fresh Runtime:** one Runtime helper process per cohort
- **Validator:** `python3 scripts/check-pass9-production-budget.py --expected-head 05664dce493abeafa257dddc3c524b11ac74924a docs/evidence/pass9-release-qualification-05664dce493a.json`
- **Dry run:** 0

Independent reviews remain required. This report does not self-certify release qualification.
