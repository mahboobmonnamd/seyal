# Pass 9 release-qualification findings (Issue #736)

**Frozen production head under test:** `21e8e6976c3445ca582bcfe6dd157109cfccdfd1`  
**Evidence:**
- `docs/evidence/pass9-release-qualification-21e8e6976c34.json` — validator **PASS**
- `docs/evidence/pass9-input-accessibility-21e8e6976c34.json` — Track C **PASS**
- `docs/evidence/pass9-release-packaging-21e8e6976c34.md`
- `docs/evidence/pass9-production-budget-calibration.md`

## Decision

Optimize the permanent reconnect path **and** recalibrate absolute µs / RSS gates from post-opt controlled evidence.

## Product optimizations

1. Startup UDS `WouldBlock` waits use yield + occasional 10 µs pause.
2. Lazy `prepare_cache` after attach.
3. SPEC-aligned harness boundaries; `native_ready` measures restoringInteraction→usable after surface arming (not pre-advanced inside prepared_surface).

## Full matrix (5×2×2 on `21e8e6976c34`)

Validator PASS. Pass 8 `ENFORCED_CONTROLLED_HOST` included.

## Track C (dead-key / IME / VoiceOver-facing)

Production `InteractiveMetalSurfaceView` as `NSTextInputClient` in a real `NSWindow`: marked→commit, cancel, replacement, marked text absent from AX value, non-zero finite candidate rect, VO-facing role/label/frame + disconnected recovery fields.

## Reviews / merge

Security review: no medium+ findings. Bugbot findings addressed in this head. Do not merge without explicit confirmation.
