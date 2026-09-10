# #819 feature-gate freeze — ready for independent review

## Freeze identity

| Item | Value |
| --- | --- |
| Branch | `issue/819` |
| Exact head | `177541f` (production tip; freeze docs commit is branch tip) |
| Base | `origin/master` @ `485bdf5` |
| Relationship | `Refs #819` only — do **not** use `Closes #819` |
| Environment | Linux x86_64 cloud agent — no headed/macOS/ARM64 claim |

## Why freeze now

M002 recovery A-gates for HistoryStore source defects addressed in this
candidate are ready for an **independent** closing review. This ledger is
implementer evidence of focused verification only. It is **not** a merge GO.

## A-gate changes in scope (most recent)

1. `86f241e` — aperiodic SoftWrap resize: extend-to-HardBreak within budget;
   cols-dependent occupancy spine for mid-chain cuts (SPEC-010 §7).
2. `177541f` — wire Truncated/`start_unit`: `admit_rows` reserves sidecar
   bytes; `shrink_for_encode` avoids zero-lead / CapacityExceeded collapse;
   continuation regression tests.

Earlier HistoryStore production work on this branch remains in the range;
prior independent reviews recorded NO-GO with P1s that the commits above
target. Reviewers must re-check those P1 dispositions on **this** SHA.

## Focused verification (exact head)

```text
cargo test -p seyal-protocol --locked --lib -- pass7::
# 14 passed

cargo test -p seyal-terminal --locked --lib history::
# 20 passed

cargo test -p seyal-terminal --locked --test history_store_regressions
# 20 passed
```

`git diff --check origin/master...HEAD` should be clean on the freeze push.

## Explicitly open (blocks Closes #819)

- [ ] Independent review GO with no open P0–P2 on this exact head
- [ ] Headed history/reflow manual steps 1–5 on macOS (exclusive Runtime)
- [ ] C-gates (#842 / #673 / #824): full scaling matrix / physical ARM64 —
      **not** required to re-run on this Issue before independent source review

## Operating rules preserved

- No full-matrix / physical ARM64 campaigns on intermediate commits
- One Runtime owner for native/XCTest/Pass8
- Draft PR should use `Refs #819` until closing evidence exists

## Requested next action

Independent reviewer: run source review against ADR-010 / SPEC-010 / #819 on
exact head `177541f` (or successor freeze SHA if docs-only amend). Record
GO/NO-GO with P0–P2 table. Do not self-approve.
