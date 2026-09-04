# Pass 9 release-qualification findings (Issue #736)

**Status:** Exact-head Release evidence retained on `ed5650c`; awaiting independent maintainer DoD confirmation under `Refs #736`.

**PR relationship:** `Refs #736` until independent maintainer review confirms DoD; then `Closes #736` only if every Done checkbox is evidenced.

## Quality bar (no compromise)

- `native_ready` measures production interactive-surface restore (SPEC §10), not coordinator stage flips alone.
- Resource exact-return remains exact (not non-growth tolerance).
- Track C VoiceOver covers discoverability, focus tracking, reconnect-style recovery, `NSAccessibility` `announcementRequested` after restore, and no marked-text transcript leak (qualification sink + production post; not system VoiceOver audio capture).
- Non-dry-run packaging builds **Release** with Apple Development Team identity (`TeamIdentifier` required).
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

- Qualification head: `ed5650ce2dec4b278562fe00dcc73e41bc6e227d`
- Matrix: `docs/evidence/pass9-release-qualification-ed5650ce2dec.json` — production-budget **PASS** (20 cohorts)
- Track C: `docs/evidence/pass9-input-accessibility-ed5650ce2dec.json` — schema `v2`, `overallPass=true`, `voiceOverAnnouncementAfterReconnect=true`
- Packaging: `docs/evidence/pass9-release-packaging-ed5650ce2dec.md` (`TeamIdentifier=3TL8X2RDAB`)
- Pass 8: paired_delta_median_percent=6.03; root cause = ~5.25 µs absolute enabled−disabled
  resize p99 gap at ~30 µs scale (scheduler noise + Pass-8 metadata bookkeeping), under 10%
  blocker, `performance_claim=false`
- Observed matrix maxima: cleanup ≤31 µs, client_rss ≤448 KiB, reconnect ≤1259 µs, prepared ≤1137 µs, native_ready ≤119 µs

## Still required before Done

- Independent non-implementer maintainer confirmation
- Issue checkbox updates to match verified reality
- Keep `Refs #736` until DoD is confirmed
