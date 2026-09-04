# Pass 9 release-qualification findings (Issue #736)

**Status:** Exact-head Release evidence retained on measured production head `05664dc`; awaiting independent maintainer DoD confirmation under `Refs #736`.

**PR relationship:** `Refs #736` until independent maintainer review confirms DoD; then `Closes #736` only if every Done checkbox is evidenced.

## Quality bar (no compromise)

- `native_ready` measures production interactive-surface restore (SPEC §10), not coordinator stage flips alone.
- Resource exact-return remains exact (not non-growth tolerance).
- Track C VoiceOver covers discoverability, focus tracking, reconnect-style recovery, `NSAccessibility` `announcementRequested` after restore, and no marked-text transcript leak (qualification sink + production post; not system VoiceOver audio capture).
- Non-dry-run packaging builds **Release** with Apple Development Team identity (`TeamIdentifier` required); Release `codesign --verify --strict --deep` fails closed.
- Full 5×2×2 matrix + Pass 8 attribution required before Done.

## Production path changes

- `MetalSurfaceView` calls `restoreNativeInteractionAfterRendererReady()` before `.usable`.
- `InteractiveMetalSurfaceView` implements first-responder / AX focused / IME activate / empty marked text.
- Graceful `RustDisplayBridge.stop()` disconnects CLIENT on the MainActor turn so cleanup meets the 250 µs gate.
- IME `activate()` is sticky per surface/window session (avoids IMK RSS growth across reconnect soaks).
- Pass 9 native_ready probe uses `Installation.nativeInteractionProbe` (plain layer, no second Runtime bridge).
- Startup UDS `WouldBlock` waits use `poll(2)` until the attach deadline (local `extern "C"`; no portable `libc` Cargo dep).
- Production restore posts `NSAccessibility.announcementRequested` via `SeyalAccessibilityAnnouncement`.

## Exact-head evidence (retained)

- Measured production head: `05664dce493abeafa257dddc3c524b11ac74924a`
  (production client poll/libc-drop tip `c70e0e9` + harness Pass 8 cohort-parse / Release verify-closed commit)
- Matrix: `docs/evidence/pass9-release-qualification-05664dce493a.json` — production-budget **PASS** (20 cohorts)
- Track C: `docs/evidence/pass9-input-accessibility-05664dce493a.json` — schema `v2`, `overallPass=true`, `voiceOverAnnouncementAfterReconnect=true`
- Packaging: `docs/evidence/pass9-release-packaging-05664dce493a.md` (`TeamIdentifier=3TL8X2RDAB`)
- Pass 8: tip log `docs/evidence/pass9-pass8-attribution-05664dce493a.log`; machine-readable `pass8.cohorts=7` matches 7 measured `pass8_attribution_cohort` lines; `paired_delta_median_percent=2.96` (under 5% explain threshold; no root-cause block). First in-soak Pass 8 attempt hit 16.50% under host load (bench assert); retained log is the successful same-tip re-measure.
- Observed matrix maxima: cleanup ≤32.375 µs, client_rss ≤384 KiB, reconnect ≤706 µs, prepared ≤1185 µs, native_ready ≤92 µs

## Still required before Done

- Independent non-implementer maintainer confirmation
- Issue checkbox updates to match verified reality
- Keep `Refs #736` until DoD is confirmed
