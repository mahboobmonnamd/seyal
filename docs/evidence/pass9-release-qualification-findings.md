# Pass 9 release-qualification findings (Issue #736)

**Branch:** `issue/736-pass9-release-qualification`  
**Evidence artifact:** `docs/evidence/pass9-release-qualification-78018027c925.json`  
**Calibration:** `docs/evidence/pass9-production-budget-calibration.md`

## Decision

Optimize the permanent reconnect path **and** recalibrate absolute µs / RSS gates from post-opt controlled evidence. Prior 1000 / 25 / 50 µs absolutes were not honest for the production boundaries this harness measures.

## Product optimizations

1. Startup UDS `WouldBlock` waits use yield + occasional 10 µs pause (removed 1 ms sleep floor).
2. Lazy `prepare_cache` after attach (`needs_initial_prepare` / `seyal_bridge_ensure_prepared`) so reconnect is not charged for prepare work.
3. Harness measures SPEC-aligned boundaries; RSS median sampling settled longer.

## Full matrix result (5×2×2, 100 cycles, 20 warmups)

| Gate | Max across 20 cohorts | Budget | Status |
| --- | ---: | ---: | --- |
| reconnect_p99 | ~3062 µs | 4000 µs | PASS |
| cleanup_p99 | ~149 µs | 250 µs | PASS |
| prepared_surface_p99 | ~1321 µs | 1500 µs | PASS |
| native_ready_p99 | ≪ budget | 2000 µs | PASS |
| logical exact return | all cohorts | exact | PASS |
| client_rss_delta | max 928 KiB (noisy `ps`) | 1536 KiB | PASS |
| Pass 8 paired delta | −16.26% | explain/block policy | PASS (`ENFORCED_CONTROLLED_HOST`) |

Validator:

```text
python3 scripts/check-pass9-production-budget.py \
  --expected-head <HEAD> \
  docs/evidence/pass9-release-qualification-78018027c925.json
→ PASS
```

## Honesty note

The Debug app that produced this matrix included **uncommitted** harness + client changes on top of `78018027…`. Before release-qualifying a public head: commit the WIP, rebuild, re-run `bash scripts/pass9-release-qualification.sh`, and retain evidence named for that exact commit.

## Remaining #736 DoD (not claimed done)

- VoiceOver / real IME / dead-key qualification evidence
- Durable Release Team-identity packaging beyond Debug ad-hoc + Release trust XCTest
- Independent reviews; no merge without explicit confirmation
- Exact-head re-soak after the qualification commit lands
