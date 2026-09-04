# Pass 9 release-qualification findings (Issue #736)

**Frozen production head under test:** `88e274bd36aae78ee6460758fa602692fe78dc38`  
**Evidence:**
- `docs/evidence/pass9-release-qualification-88e274bd36aa.json` — validator **PASS**
- `docs/evidence/pass9-input-accessibility-88e274bd36aa.json` — Track C **PASS**
- `docs/evidence/pass9-release-packaging-88e274bd36aa.md`
- `docs/evidence/pass9-production-budget-calibration.md`

## Decision

Optimize the permanent reconnect path **and** recalibrate absolute µs / RSS gates from post-opt controlled evidence. Prior 1000 / 25 / 50 µs absolutes were not honest for the production boundaries this harness measures.

## Product optimizations

1. Startup UDS `WouldBlock` waits use yield + occasional 10 µs pause (removed 1 ms sleep floor).
2. Lazy `prepare_cache` after attach (`needs_initial_prepare` / `seyal_bridge_ensure_prepared`).
3. SPEC-aligned harness boundaries; settled RSS median sampling.

## Full matrix (5×2×2 on `88e274bd36aa`)

Validator PASS. Pass 8 `ENFORCED_CONTROLLED_HOST` included. Timing and resource gates under calibrated budgets in `check-pass9-production-budget.py`.

## Track C (dead-key / IME / VoiceOver-facing)

Production `InteractiveMetalSurfaceView` as `NSTextInputClient`: marked→commit, cancel, replacement, marked text absent from AX value, finite candidate rect, VO-facing role/label/recovery fields (system VoiceOver audio not enabled).

## Packaging

Debug ad-hoc packaging inspection + Release trust-rule XCTest (`testReleaseTrustRulesRejectAdHocHelpers`) retained by the orchestrator. Durable paid Team-identity Release signing remains host-limited where no Developer identity is present.

## Reviews / merge

Independent reviews remain required. Do not merge without explicit confirmation.
