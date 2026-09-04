# Pass 9 production budget environment report

- **Status:** `EVIDENCE_RETAINED_AWAITING_MAINTAINER_SIGN_OFF`
- **Issue:** #736
- **Date:** 2026-09-04
- **Qualification / measured production head:** `05664dce493abeafa257dddc3c524b11ac74924a`
- **Branch tip after evidence commit:** may sit one evidence-only commit above that head; matrix/Track C/Pass 8/packaging claims bind to `05664dc` (includes production `seyal-client` poll extern + libc-drop and harness Pass 8 cohort parsing)
- **Artifact:** `docs/evidence/pass9-release-qualification-05664dce493a.json`
- **Calibration:** `docs/evidence/pass9-production-budget-calibration.md`
- **Security review:** `docs/evidence/pass9-security-review-745.md`
- **Packaging:** `docs/evidence/pass9-release-packaging-05664dce493a.md`
- **Track C:** `docs/evidence/pass9-input-accessibility-05664dce493a.json` (`overallPass=true`, schema `v2`)
- **Pass 8:** `docs/evidence/pass9-pass8-attribution-05664dce493a.log` (`pass8.cohorts=7`, `paired_delta_median_percent=2.96`)

## Open process gates

- Independent non-implementer sign-off / maintainer confirmation
- Issue checkbox updates after independent review
- Keep `Refs #736` until DoD is confirmed (then `Closes` only if every checkbox is evidenced)

## Measurement notes (quality bar)

- `native_ready_p99_us` = SPEC-009 §10 production interactive restore before Usable
- Resource exact-return is exact for attachment/controller/fd/thread/socket/renderer/allocator fields
- Reconnect-owned allocator proxies (not process-wide `malloc_zone_statistics`):
  - `runtime_allocator_in_use_kib` = `live_handles`
  - `client_allocator_in_use_kib` = dedicated Metal GPU KiB for the soak presenter
- `client_rss_delta_kib` remains a supporting `ps` RSS signal with a calibrated absolute gate
- Non-dry-run packaging is Release + Apple-issued Team identity; `codesign --verify --strict --deep` is fail-closed for Release
- Startup `WouldBlock` path: `poll(2)` readable/writable wait until attach deadline (local `extern "C"`; no portable `libc` Cargo dep)
- VoiceOver announcement: production `NSAccessibility.post(.announcementRequested)` on native-interaction restore; Track C asserts via qualification sink (`seyal.pass9.input-accessibility.v2`)
