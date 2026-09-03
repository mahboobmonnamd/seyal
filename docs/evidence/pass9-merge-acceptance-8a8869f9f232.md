# Pass 9 merge-acceptance evidence

- **Issue:** #735
- **Implementation PR:** #734
- **Exact production head under test:** `8a8869f9f2325a0abb4cc0813af189ba2d5ae770`
- **Artifact:** `pass9-merge-acceptance-8a8869f9f232.json`
- **Schema:** `seyal.pass9.merge-acceptance.v1`
- **Modes:** `graceful_detach`, `abrupt_socket_loss`
- **Cycles:** 100 each after 5 warmups
- **Geometry:** `120x40`
- **Topology:** bundled Debug `Seyal.app` + `Contents/Helpers/seyal-runtime` (production coordinator/bridge path)
- **Host:** Apple Silicon, local controlled run
- **Validator:**

```sh
python3 scripts/check-pass9-merge-acceptance.py \
  --expected-head 8a8869f9f2325a0abb4cc0813af189ba2d5ae770 \
  docs/evidence/pass9-merge-acceptance-8a8869f9f232.json
```

## Cohort summary

| Mode | Cycles | reconnect p99 (µs) | runtime RSS Δ KiB | client RSS Δ KiB | exact resource return |
| --- | ---: | ---: | ---: | ---: | --- |
| graceful_detach | 100 | 24565 | 144 | 304 | yes (attachments/controllers/handles/sockets/renderer/retry timers) |
| abrupt_socket_loss | 100 | 11546 | 0 | 112 | yes |

Continuity: same `runtime_id` / `execution_id` across both cohorts; fresh unique `attachment_id` every measured cycle.

## Additional merge-critical proofs in this PR

- Release trust rules reject ad-hoc helpers (`testReleaseTrustRulesRejectAdHocHelpers`)
- Native stale-handle adopt fails closed (`testNativeStaleHandleAdoptionIsRejected`)
- Existing Rust adversarial suite retains protocol stale/malformed coverage; XCUITest Pass 9 recovery smoke remains green

## Not claimed here

- Full five-cohort × two-geometry production-budget matrix (#736)
- Independent implementation/architecture/security/performance/accessibility review certification
- VoiceOver / exhaustive IME qualification

Independent reviews remain required before merge of #734. This report is evidence only; it does not self-certify those gates.
