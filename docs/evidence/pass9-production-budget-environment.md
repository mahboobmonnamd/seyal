# Pass 9 production budget environment report

- **Status:** `EVIDENCE_RETAINED_AWAITING_MAINTAINER_SIGN_OFF`
- **Issue:** #736
- **Date:** 2026-09-04
- **Qualification head:** `5f8108ac6ea1464e5645a00770b163aa524ee6b2`
- **PR tip (evidence commit):** `8e72abc7f73aaff674d5889a7c28b1a33c71e680`
- **Artifact:** `docs/evidence/pass9-release-qualification-5f8108ac6ea1.json`
- **Calibration:** `docs/evidence/pass9-production-budget-calibration.md`
- **Security review:** `docs/evidence/pass9-security-review-745.md`
- **Packaging:** `docs/evidence/pass9-release-packaging-5f8108ac6ea1.md` (`TeamIdentifier=3TL8X2RDAB`)
- **Track C:** `docs/evidence/pass9-input-accessibility-5f8108ac6ea1.json` (`overallPass=true`)

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
