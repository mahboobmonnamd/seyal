# Pass 9 production budget environment report

- **Status:** `FULL_MATRIX_VALIDATOR_PASS_PENDING_COMMIT_AND_VO_IME`
- **Issue:** #736
- **Date:** 2026-09-04
- **Calibration:** `docs/evidence/pass9-production-budget-calibration.md`
- **Latest matrix:** `docs/evidence/pass9-release-qualification-78018027c925.json` (validator PASS; rebuild on committed head before release claim)

## Scope

Physical controlled-host Pass 9 lifecycle/performance cohorts for release
qualification (`seyal.pass9.production-budget.v1`), distinct from merge-acceptance
(`seyal.pass9.merge-acceptance.v1`).

## Harness

The repository provides a reusable generator that shares the merge-acceptance
production recovery topology:

```sh
# Full SPEC-009 §16 matrix (5 cohorts × 2 modes × 2 geometries, 20 warmups)
bash scripts/pass9-release-qualification.sh

# Tooling dry-run (short cycles by default; skips budget validator / trust XCTest)
SEYAL_PASS9_DRY_RUN=1 bash scripts/pass9-release-qualification.sh

# Meaningful single-cohort smoke (overrides dry-run defaults)
SEYAL_PASS9_DRY_RUN=1 SEYAL_PASS9_CYCLES=100 SEYAL_PASS9_WARMUP=20 \
  SEYAL_PASS9_COHORTS=1 SEYAL_PASS9_GEOMETRIES=120x40 \
  SEYAL_PASS9_MODES=graceful_detach \
  bash scripts/pass9-release-qualification.sh
python3 scripts/check-pass9-release-smoke.py \
  docs/evidence/pass9-release-partials-<shortsha>/graceful_detach-120x40-c1.json
```

Validator (exact-head evidence only):

```sh
python3 scripts/check-pass9-production-budget.py \
  --expected-head <full-40-character-production-head> \
  docs/evidence/pass9-release-qualification-<shortsha>.json
```

## Accepted host preconditions

- Apple Silicon Mac; otherwise-idle / exclusive host for retained RSS and
  detached-CPU evidence
- Release-qualification evidence names the exact tested commit SHA
- Fresh Runtime helper process per independent cohort
- Geometries `120x40` and `80x24`; modes `graceful_detach` and
  `abrupt_socket_loss` (`socket_shutdown_owned_disconnect`)
- Topology disclosure: Metal prepare/release equivalent to
  `MetalSurfaceView.consumeBridgeFrame` (not full AppKit window present), same
  honesty bar as merge-acceptance

## Calibrated absolute timing gates

| Gate | Limit |
| --- | ---: |
| reconnect_p99 | ≤ 4000 µs |
| cleanup_p99 | ≤ 250 µs |
| prepared_surface_p99 | ≤ 1500 µs |
| native_ready_p99 | ≤ 2000 µs |

See calibration note for derivation. Resource exact-return and Pass 8 paired
attribution policy are unchanged.

## Still pending on controlled host

- Full five-cohort exact-head artifact that passes the production budget
  validator on the retained production head
- Pass 8 paired attribution with `pass8.gate=ENFORCED_CONTROLLED_HOST`
- VoiceOver / real IME / dead-key qualification evidence
- Durable Release Team-identity packaging beyond Debug ad-hoc inspection +
  Release trust-rule XCTest

Older pre-calibration absolute µs values and validator self-tests alone are not
exact-head production evidence.
