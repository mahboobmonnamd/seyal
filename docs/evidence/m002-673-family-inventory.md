# M002 #673 finite family inventory

Refs #673 only. This is not #673 Done and is not a `PHYSICAL_ARM64`
`VALID` pass.

Machine-readable copy: [`m002-673-family-inventory.toml`](m002-673-family-inventory.toml).
Authority: [`M002-PERFORMANCE-CONTRACT-V1.md`](M002-PERFORMANCE-CONTRACT-V1.md)
and SPEC-010 §18.1 / #818 for the two accepted HistoryStore ceilings.

## Host this session

- **Class:** `uncontrolled-developer-host`
- **Machine:** Apple M5 Pro MacBook Pro (`Mac17,9`), 15 cores, 24 GiB, macOS 27.0
- **Power:** battery, discharging — not a controlled lab slot
- **`PHYSICAL_ARM64` `VALID`:** forbidden on this host
- **#837:** not started; ceilings are not accepted

## Decision rule (unchanged)

Five fresh-process cohorts × 20 warmups × 100 samples. Nearest-rank
percentiles. No best-run substitution. Proposed gates have no accepted
numeric ceilings and cannot be evaluated as release PASS/FAIL. Missing
metrics stay `unknown` or `not-instrumented`.

## Inventory

| Gate | Status | Target | Baseline SHA | Harness | Evidence | Environment | Numeric |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `history_active_reflow_ms` | accepted | 2/4/8 ms | `f105364` | ready | retained `20260916T171837Z` | `PLATFORM_LIMITED` | **FAIL** |
| `history_sealed_segment_reflow_ms` | accepted | 1/2/4 ms | `f105364` | ready | retained `20260916T171837Z` | `PLATFORM_LIMITED` | PASS |
| `pty_to_terminal_state` | proposed | none | none | ready (PTY→`TerminalState`) | none | `PLATFORM_LIMITED` | not-instrumented |
| `damage_to_client_cache` | proposed | none | none | not-instrumented | none | `PLATFORM_LIMITED` | not-instrumented |
| `renderer_prepare_submission` | proposed | none | none | not-instrumented | none | `PLATFORM_LIMITED` | not-instrumented |
| `input_visible_proxy` | proposed | none | none | not-instrumented | none | `PLATFORM_LIMITED` | not-instrumented |
| `high_output_responsiveness` | proposed | none | none | not-instrumented | none | `PLATFORM_LIMITED` | not-instrumented |
| `resource_scaling_rss` | proposed | none | none | not-instrumented | none | `PLATFORM_LIMITED` | not-instrumented |
| `resource_scaling_fds` | proposed | none | none | not-instrumented | none | `PLATFORM_LIMITED` | not-instrumented |
| `resource_scaling_threads` | proposed | none | none | not-instrumented | none | `PLATFORM_LIMITED` | not-instrumented |
| `startup` | proposed | none | none | not-instrumented | none | `PLATFORM_LIMITED` | not-instrumented |
| `idle_cpu` | proposed | none | none | not-instrumented | none | `PLATFORM_LIMITED` | not-instrumented |
| `teardown_recovery` | proposed | none | none | not-instrumented | none | `PLATFORM_LIMITED` | not-instrumented |

The `f105364` HistoryStore row stays StatsAlloc-era `PLATFORM_LIMITED`.
The active-reflow relative p95/p99 FAIL is retained. Do not remasure it
to hide host noise.

## Runner

```sh
python3 scripts/run-m002-performance-contract.py --inventory
python3 scripts/run-m002-performance-contract.py --self-test
# Opt-in collection for a harness-ready proposed family. Always PLATFORM_LIMITED
# on an uncontrolled host. Cannot evaluate a proposed gate as PASS/FAIL.
python3 scripts/run-m002-performance-contract.py --gate pty_to_terminal_state
```

HistoryStore remasure stays on the landed opt-in runner and is not the
default path:

```sh
python3 scripts/run-m002-history-reflow-contract.py
```
