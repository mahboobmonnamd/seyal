# Pass 9 release-qualification findings (Issue #736)

**Status:** Exact-head Release evidence retained on `5f8108a`; awaiting independent maintainer DoD confirmation under `Refs #736`.

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

- Startup UDS `WouldBlock` waits use `poll(2)` until the attach deadline.
- Production restore posts `NSAccessibility.announcementRequested` via `SeyalAccessibilityAnnouncement`.

## Exact-head evidence (retained)

- Prior matrix head: `5f8108ac6ea1464e5645a00770b163aa524ee6b2` (pre-poll / pre-announcement)
- Re-run matrix + Track C required on the tip that lands poll + announcement before Done claim update

## Still required before Done

- Re-soak Release 5×2×2 + Track C v2 (`vo_announcement_after_reconnect`) on the poll+announcement tip
- Independent non-implementer maintainer confirmation
- Issue checkbox updates to match verified reality
- Keep `Refs #736` until DoD is confirmed