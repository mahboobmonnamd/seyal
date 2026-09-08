# M002 #819 exact-head history benchmark record

- **Issue:** #819
- **Authority:** ADR-010 and SPEC-010, with the frozen budgets from #818
- **Measured production code head:** `9cd8e3976a8d34d598d702deebe66c2b7694cd12`
- **Recorded:** 2026-09-08
- **Host/build boundary:** local Apple Silicon macOS host, ARM64 Release Cargo benchmark
- **Claim status:** comparative evidence only (`performance_claim=false`)

This record retains the completed exact-head run for the `history_reflow` production
benchmark. It does not claim that the complete SPEC-010 acceptance matrix has run.

## Command

```text
SEYAL_HISTORY_BENCH_LINES=10000,100000,1000000 \
SEYAL_HISTORY_BENCH_EXECUTIONS=1 \
cargo bench --bench history_reflow -- --quiet
```

The benchmark used the production `TerminalState` history path, a 120x40 source
geometry, 80 reflow columns, 32 nearest-rank samples, and reported aggregate
resident bytes for the retained executions. The run completed successfully.

## Retained output

| Retained source lines | Executions | Resident bytes | p50 (ns) | p95 (ns) | p99 (ns) |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 10,000 | 1 | 5,512,757 | 8,833 | 18,959 | 111,959 |
| 100,000 | 1 | 33,541,947 | 8,667 | 21,333 | 107,250 |
| 1,000,000 | 1 | 33,537,653 | 8,834 | 13,041 | 81,208 |

The output included `percentile_method=nearest-rank` and
`performance_claim=false` for every case. The resident values are benchmark
observations; they are not a release-level RSS attribution or a claim against
the physical-host latency gates.

## Acceptance ledger

| Gate | Status | Evidence / remaining work |
| --- | --- | --- |
| Exact-head benchmark execution | **Recorded** | The command and output above ran against the measured code head. This documentation commit is evidence-only. |
| 10k/100k/1M single-execution reflow comparison | **Recorded** | Three completed cases above; comparative only. |
| Append latency, search, anchor resolution, allocation churn | **Missing** | This benchmark does not emit those dimensions. |
| Execution populations 1/10/50/100 | **Incomplete** | Only the 1-execution cases are retained here. |
| Required column oscillation 40/48/64/80/96/132/160 | **Missing** | Current harness record covers only reflow at 80 columns. |
| ASCII, styled, CJK, emoji/combining workloads | **Missing** | Current workload is numbered ASCII history lines. |
| Physical ARM64 p50/p95/p99 acceptance gates | **Unverified** | This run is comparative and does not provide the controlled release matrix or RSS attribution required by #818/#673. |
| Fuzz/property and focused regression tests | **Separate evidence** | See the issue/PR validation record; this file does not replace those results. |
| `make check` / Foundation gates | **Blocked on host** | The exact branch run reached the existing Pass 8 Runtime-to-Swift metadata singleton failure (`AlreadyRunning`). |
| Manual verification | **Unverified** | Headed manual evidence remains required for the user-visible history/reflow cases. |

This record intentionally leaves incomplete and unavailable gates explicit. It
must not be used as the sole basis for merging or closing #819.
