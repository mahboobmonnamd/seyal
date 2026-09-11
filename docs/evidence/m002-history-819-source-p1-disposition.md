# #819 source P1 disposition after freeze remediation

## Scope

| Item | Value |
| --- | --- |
| Branch | `issue/819` |
| Auditor | implementing cloud agent (Linux x86_64) |
| Classification | **Implementer source audit** — not a substitute for independent review under ISSUE-PROTOCOL |

## Verdict

**Prior independent-review source P0–P1 findings: addressed on the portable HistoryStore / wire path** (including wrap-index miss ephemeral spine).

**NO-GO for `Closes #819`:** headed macOS steps 1–5, independent second-party review, and C-gates (#842/#673/#824) remain open. This document is not merge approval.

## Disposition table

| Prior finding | Disposition |
| --- | --- |
| Eager full-history resize | **ADDRESSED** — `eager_resize_suffix` + row budget |
| SoftWrap / aperiodic linear occupancy | **ADDRESSED** — HardBreak extend, cached spine, ephemeral spine on index miss |
| Multi-scalar Truncated zero-progress | **ADDRESSED** — sidecar-aware `admit_rows`, `shrink_for_encode`, `start_unit` tests |
| Retained/active soft-wrap one stream | **ADDRESSED** — combined `reflow_from` + regression |
| Resident / seal / derived accounting | **ADDRESSED** |
| Wrap miss returned 0 | **ADDRESSED** — `wrap_occupancy_from_canonical` |
| Blank tails / unbounded wrap index | **ADDRESSED** — seal + 4 MiB derived cap |

P2 headed Metal history-wide glyphs: **ENVIRONMENT_UNSUPPORTED** on Linux.

## Focused verification

```text
cargo test -p seyal-terminal --locked --lib history::
# 20 passed
cargo test -p seyal-terminal --locked --test history_store_regressions
# 20 passed
cargo test -p seyal-protocol --locked --lib -- pass7::
# 14 passed
```

## Requested next action

1. Independent reviewer (not the implementer): GO/NO-GO on this production tip.
2. macOS headed history/reflow steps 1–5 under exclusive Runtime.
3. Keep C-gates on #842/#673/#824.
