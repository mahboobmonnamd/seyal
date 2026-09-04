# Pass 9 production budget environment report

- **Status:** `EVIDENCE_RETAINED_AWAITING_MAINTAINER_SIGN_OFF`
- **Issue:** #736
- **Date:** 2026-09-04
- **Qualification head (matrix under test):** `ed5650ce2dec4b278562fe00dcc73e41bc6e227d`
- **Branch tip:** same as qualification head (poll startup waits + VoiceOver announcement)
- **Artifact:** `docs/evidence/pass9-release-qualification-ed5650ce2dec.json`
- **Calibration:** `docs/evidence/pass9-production-budget-calibration.md`
- **Security review:** `docs/evidence/pass9-security-review-745.md`
- **Packaging:** `docs/evidence/pass9-release-packaging-ed5650ce2dec.md`
- **Track C:** `docs/evidence/pass9-input-accessibility-ed5650ce2dec.json` (`overallPass=true`, schema `v2`)

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
- Non-dry-run packaging is Release + Apple-issued Team identity
- Startup `WouldBlock` path: `poll(2)` readable/writable wait until attach deadline (see calibration)
- VoiceOver announcement: production `NSAccessibility.post(.announcementRequested)` on native-interaction restore; Track C asserts via qualification sink (`seyal.pass9.input-accessibility.v2`)
