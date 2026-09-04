# Pass 9 release-qualification findings (Issue #736)

**Status:** In progress — completing SPEC-009 §10/§16 gates without softened quality bars.

**PR relationship:** **`Refs #736`** until independent maintainer review confirms DoD; then `Closes #736` only if every Done checkbox is evidenced.

## Quality bar (no compromise)

- `native_ready` measures production interactive-surface restore (SPEC §10), not coordinator stage flips alone.
- Resource exact-return remains exact (not non-growth tolerance).
- Track C VoiceOver smoke covers discoverability, focus tracking, reconnect-style recovery, and no marked-text transcript leak.
- Non-dry-run packaging builds **Release** with Apple Development Team identity (`TeamIdentifier` required).
- Full 5×2×2 matrix + Pass 8 attribution required before Done.

## Production path changes

- `MetalSurfaceView` calls `restoreNativeInteractionAfterRendererReady()` before `.usable`.
- `InteractiveMetalSurfaceView` implements first-responder / AX focused / IME activate / empty marked text.

## Still required before Done

- Fresh exact-head 5×2×2 PASS under the restored gates
- Pass 8 attribution with threshold explanations
- Independent non-implementer maintainer confirmation
- Issue checkbox updates to match verified reality
