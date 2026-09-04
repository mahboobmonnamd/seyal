# Pass 9 production budget environment report

- **Status:** `RELEASE_QUALIFICATION_EVIDENCE_COMPLETE_ON_88e274bd36aa`
- **Issue:** #736
- **Date:** 2026-09-04
- **Calibration:** `docs/evidence/pass9-production-budget-calibration.md`
- **Exact production head:** `88e274bd36aae78ee6460758fa602692fe78dc38`
- **Matrix artifact:** `docs/evidence/pass9-release-qualification-88e274bd36aa.json` (validator PASS)
- **Input/accessibility:** `docs/evidence/pass9-input-accessibility-88e274bd36aa.json` (PASS)

## Scope

Physical controlled-host Pass 9 lifecycle/performance cohorts for release
qualification (`seyal.pass9.production-budget.v1`), distinct from merge-acceptance
(`seyal.pass9.merge-acceptance.v1`).

## Harness

```sh
bash scripts/pass9-release-qualification.sh
python3 scripts/check-pass9-production-budget.py \
  --expected-head 88e274bd36aae78ee6460758fa602692fe78dc38 \
  docs/evidence/pass9-release-qualification-88e274bd36aa.json
```

## Calibrated absolute timing gates

| Gate | Limit |
| --- | ---: |
| reconnect_p99 | ≤ 4000 µs |
| cleanup_p99 | ≤ 250 µs |
| prepared_surface_p99 | ≤ 1500 µs |
| native_ready_p99 | ≤ 2000 µs |
| client_rss_delta | ≤ 1536 KiB (noisy `ps`; logical exact-return is the leak contract) |

## Remaining non-evidence items

- Independent architecture/security/performance/accessibility reviews on the PR
- Explicit maintainer confirmation before merge
- Paid Apple Developer Team-identity Release signing when that identity is available
