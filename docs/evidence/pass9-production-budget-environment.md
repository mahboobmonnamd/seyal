# Pass 9 production budget environment report

- **Status:** `RELEASE_QUALIFICATION_EVIDENCE_COMPLETE_ON_21e8e6976c34`
- **Issue:** #736
- **Date:** 2026-09-04
- **Calibration:** `docs/evidence/pass9-production-budget-calibration.md`
- **Exact production head:** `21e8e6976c3445ca582bcfe6dd157109cfccdfd1`
- **Matrix artifact:** `docs/evidence/pass9-release-qualification-21e8e6976c34.json` (validator PASS)
- **Input/accessibility:** `docs/evidence/pass9-input-accessibility-21e8e6976c34.json` (PASS)

## Harness

```sh
bash scripts/pass9-release-qualification.sh
python3 scripts/check-pass9-production-budget.py \
  --expected-head 21e8e6976c3445ca582bcfe6dd157109cfccdfd1 \
  docs/evidence/pass9-release-qualification-21e8e6976c34.json
```

## Calibrated absolute timing gates

| Gate | Limit |
| --- | ---: |
| reconnect_p99 | ≤ 4000 µs |
| cleanup_p99 | ≤ 250 µs |
| prepared_surface_p99 | ≤ 1500 µs |
| native_ready_p99 | ≤ 2000 µs |
| client_rss_delta | ≤ 1536 KiB |

## Remaining non-evidence items

- Independent human architecture/accessibility review sign-off as required by project policy
- Explicit maintainer confirmation before merge
- Paid Apple Developer Team-identity Release signing when that identity is available
