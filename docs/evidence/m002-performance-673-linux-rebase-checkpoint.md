# #673 Linux rebase checkpoint (contract PR #843)

## Identity

| Item | Value |
| --- | --- |
| Canonical branch | `issue/673-performance-contract` |
| Owning Issue | #673 |
| PR | #843 |
| Relationship | `Refs #673` — do **not** use `Closes #673` |

## What this checkpoint records

The performance-contract branch was **rebased onto current `origin/master`** and
Linux-only contract validators were re-run. A PLATFORM_LIMITED retention
negative/positive fixture pair was added so platform-limited records cannot
pass without a reason and are retained as `PLATFORM_LIMITED` when reasoned.
A proposed-gate evaluation negative was added so records cannot claim results
against gates still marked `status = "proposed"` (for example
`input_visible_proxy`).

This does **not** claim any release performance gate passed.

Hosted `native-macos-smoke` on exact head `3b40d233` succeeded. That is
Foundation Quality CI for this contract PR, not accepted physical ARM64
performance evidence and not a `Closes #673` gate.

## Linux verification

```text
python3 scripts/check-m002-performance-contract.py
# M002 performance contract shape passed.

python3 scripts/test-ci-validators.py
# controlled negative fixtures were rejected by every repository validator.
```

## Still open (blocks Closes #673 / performance pass)

- [x] Hosted `native-macos-smoke` on exact head `3b40d233` (CI; not a performance pass)
- [ ] Independent review GO on the current tip
- [ ] Accepted physical ARM64 evidence under this contract (C-gate)

## Mac note

Headed XCTest / physical ARM64 campaigns require macOS and an exclusive Runtime
owner. They are deferred while Mac is unavailable; do not weaken native tests.
