# Pass 10 independent final milestone review

**READY — #727 and #5 closed**

| Field | Value |
|---|---|
| Reviewer role | Independent final milestone review (did not implement) |
| Date (UTC) | 2026-09-05 |
| Production freeze | `d845c6ddbe86f20183186f1aa69f2293aa8356ba` (#776) |
| Harness tip | `e2b76024de2c85b3e9adb6dd5dcadb7b40881079` (#778) |
| Evidence tip consulted | `c1246d43869abda194bcc0c678ab8c916c581caf` (#779) / `M001-PASS10-EVIDENCE.md` |
| Machine RSS gate | **`CLIENT_RSS_KIB = 1536`** in `scripts/check-pass9-production-budget.py` (Pass 9 calibration at `1005bc4`; **unchanged by Pass 10**) |
| Pass 9 | Remains Done; not reopened |

## Correction (#784)

Earlier closeout wording asserted a “soft RSS gate of 768 KiB” and even claimed `CLIENT_RSS_KIB = 768`. That was **false**. The original merge-acceptance constant was `768`; Pass 9 release qualification raised the machine gate to **1536** (see `docs/evidence/pass9-production-budget-calibration.md`). Pass 10 did not change the constant. Measured freeze evidence (merge-acceptance 224/512; release-qual max 448) still satisfies the **1536** machine gate. This review is corrected under Issue #784; production freeze and Pass 9 remain Done.

## Mandatory gates

| Gate | Verdict | Evidence pointer |
|---|---|---|
| `make check` on freeze | PASS | `/tmp/pass10-evidence-3f7b2d9/make-check-d845c6d.exit` = `0`; log ends `MAKE_CHECK_EXIT:0` |
| §6.9 fuzz 8×600s controlled-campaign | PASS | `/tmp/pass10-evidence-3f7b2d9/fuzz/campaigns/summary.txt` `ALL_EXIT:0`; each target log `EXIT:0` @~600s; grade `controlled-campaign` |
| Pass9 merge soak RSS ≤1536 | PASS | `pass9-rss-fix/pass9-merge-acceptance-3f7b2d926dca.json`: graceful `client_rss_delta_kib=224`, abrupt `512`; validator OK; soak `result=ok` |
| Pass9 release-qual + budget | PASS | `docs/evidence/pass9-release-qualification-d845c6ddbe86.json` on freeze; max RSS delta 448 ≤1536; `python3 scripts/check-pass9-production-budget.py --expected-head d845c6d…` PASS; log line `ACCEPTED_ARTIFACT_BUDGET:PASS`; `pass9-budget-recheck.log` PASS |
| Pass9 IME/a11y | PASS | `pass9-input-accessibility-d845c6ddbe86.json` `overallPass: true` |
| §6.12 alt-screen + terminfo | PASS | `section-6.12-alt-terminfo-rerun.log`: `ALT_EXIT:0` / `PRIMARY_EXIT:0` / `INFOCMP_EXIT:0` |
| Headed §6.12 items 11–20 (aggregate) | PASS | Headed recovery UITest unblocked after #778 packaging: `section-6.12-headed-uitest-unblocked.log` — `testPass9ProductionRecoverySurvivesGracefulAndForcedGUIExit` passed (20.381s), `UITEST_EXIT:0`. Supporting aggregate: VT/alt-screen, `pass8_blocks`, terminate/reap registry-zero, IME/a11y JSON |
| Clean XCUI suite via #778 CI | PASS | PR #778 `native-macos-smoke` SUCCESS (run `33954532088`); master push on harness tip `e2b7602` Foundation Quality SUCCESS including `native-macos-smoke` (run `33956007221`) |
| Clean production demo aggregate | PASS | `clean-demo-d845c6d-recheck.log`: `CHECK_EXIT:0` / `DEMO_PIPELINE_DONE`. Host note: clean-demo recorded `TEST_EXIT:2` after SeyalTests 81/81 passed when `SeyalUITests-Runner` hit `Timed out while enabling automation mode` (host automation init, not a product assertion). Full XCUI proven by #778 CI on `e2b7602` + headed recovery `UITEST_EXIT:0` |
| Machine RSS gate unchanged by Pass 10 | PASS | `CLIENT_RSS_KIB = 1536` at freeze and harness tip; no change in #776/#778; raise provenance is Pass 9 `1005bc4`, not Pass 10 |
| Production freeze intact | PASS | #778 diff vs freeze is tests + `scripts/test-macos-ui.sh` + evidence docs only; freeze is ancestor of harness tip; no production crate/behavior change |
| Non-goals / no silent M002 | PASS | Evidence ledger §6.13 + layering; no commercial crates in OSS path |
| §6.10 security review | PASS | `docs/evidence/m001-pass10-security-review.md` Pass 10 focused review PASS; post-#776/#778 deltas are harness/validation only |

## Freeze / RSS integrity (explicit)

- Production freeze **must and does** remain `d845c6ddbe86f20183186f1aa69f2293aa8356ba` (#776).
- Machine RSS gate **must and does** remain **`CLIENT_RSS_KIB = 1536`**; measured merge-acceptance 224/512 and release-qual max 448 all ≤1536.
- Do not treat the historical pre-calibration merge-acceptance constant of 768 as the current machine gate.
- #778 is validation harness packaging only (`dev.seyal.Seyal.runtime` helper install after `build-for-testing`); production behavior freeze intact.
- Pass 9 stays Done.

## Close authority

Owning Issue **#727** and parent milestone **#5** are **closed**. Do not reopen Pass 9. Do not change `CLIENT_RSS_KIB` without a new calibration Issue. Do not treat #778 as a production-behavior change. Issue **#784** corrects closeout gate honesty only.
