# Pass 9 release-qualification findings (Issue #736)

**Status:** Exact-head Release evidence retained on `5f8108a`; awaiting independent maintainer DoD confirmation under `Refs #736`.

**PR relationship:** `Refs #736` until independent maintainer review confirms DoD; then `Closes #736` only if every Done checkbox is evidenced.

## Quality bar (no compromise)

- `native_ready` measures production interactive-surface restore (SPEC §10), not coordinator stage flips alone.
- Resource exact-return remains exact (not non-growth tolerance).
- Track C VoiceOver smoke covers discoverability, focus tracking, reconnect-style recovery, and no marked-text transcript leak (AX smoke, not system VoiceOver audio).
- Non-dry-run packaging builds **Release** with Apple Development Team identity (`TeamIdentifier` required).
- Full 5×2×2 matrix + Pass 8 attribution required before Done.

## Production path changes

- `MetalSurfaceView` calls `restoreNativeInteractionAfterRendererReady()` before `.usable`.
- `InteractiveMetalSurfaceView` implements first-responder / AX focused / IME activate / empty marked text.
- Graceful `RustDisplayBridge.stop()` disconnects CLIENT on the MainActor turn so cleanup meets the 250 µs gate.
- IME `activate()` is sticky per surface/window session (avoids IMK RSS growth across reconnect soaks).
- Pass 9 native_ready probe uses `Installation.nativeInteractionProbe` (plain layer, no second Runtime bridge).

## Exact-head evidence (retained)

- Qualification head: `5f8108ac6ea1464e5645a00770b163aa524ee6b2`
- Matrix: `docs/evidence/pass9-release-qualification-5f8108ac6ea1.json` — production-budget **PASS** (20 cohorts)
- Track C: `docs/evidence/pass9-input-accessibility-5f8108ac6ea1.json` — `overallPass=true`
- Packaging: `TeamIdentifier=3TL8X2RDAB`
- Observed matrix maxima: cleanup ≤28 µs, client_rss ≤352 KiB, reconnect ≤890 µs, prepared ≤969 µs, native_ready ≤95 µs

## Still required before Done

- Independent non-implementer maintainer confirmation
- Issue checkbox updates to match verified reality
- Explicit DoD decision on Issue “VoiceOver announcement”: either narrow #736 to SPEC §10 AX smoke (current Track C) or add a system VoiceOver audio/announcement gate — Track C does **not** claim announcement today
