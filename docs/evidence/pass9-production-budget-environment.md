# Pass 9 production budget environment report

- **Status:** `EVIDENCE_RETAINED_AWAITING_MAINTAINER_SIGN_OFF`
- **Issue:** #736
- **Date:** 2026-09-04
- **Qualification head (matrix under test):** `5f8108ac6ea1464e5645a00770b163aa524ee6b2`
- **Branch tip:** docs/claim-accuracy commits only above that head (no production-code change since `5f8108a`; tip SHA moves with each docs fix)
- **Artifact:** `docs/evidence/pass9-release-qualification-5f8108ac6ea1.json`
- **Calibration:** `docs/evidence/pass9-production-budget-calibration.md`
- **Security review:** `docs/evidence/pass9-security-review-745.md`
- **Packaging:** `docs/evidence/pass9-release-packaging-5f8108ac6ea1.md` (`TeamIdentifier=3TL8X2RDAB`)
- **Track C:** `docs/evidence/pass9-input-accessibility-5f8108ac6ea1.json` (`overallPass=true`)

## Open process gates

- Independent non-implementer sign-off / maintainer confirmation
- Issue checkbox updates after independent review
- Keep `Refs #736` until DoD is confirmed (then `Closes` only if every checkbox is evidenced)
- Re-soak + Track C v2 announcement evidence on the tip that lands `poll(2)` startup waits and production `announcementRequested`

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