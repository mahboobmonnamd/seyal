# M002 #819 exact-head history benchmark record

- **Issue:** #819
- **Authority:** ADR-010 and SPEC-010, with the frozen budgets from #818
- **Measured production code head:** `9cd8e3976a8d34d598d702deebe66c2b7694cd12`
- **Benchmark harness head:** `8e8b0e018722d80b1c90adc9cf4365e833bd3f52`
- **Recorded:** 2026-09-08
- **Host/build boundary:** local Apple Silicon macOS host, ARM64 Release Cargo benchmark for the retained legacy run; current harness smoke runs are comparative only
- **Claim status:** comparative evidence only (`performance_claim=false`)

This record retains the completed exact-head run for the `history_reflow` production
benchmark. It does not claim that the complete SPEC-010 acceptance matrix has run.

## Legacy benchmark command

```text
SEYAL_HISTORY_BENCH_LINES=10000,100000,1000000 \
SEYAL_HISTORY_BENCH_EXECUTIONS=1 \
cargo bench --bench history_reflow -- --quiet
```

The legacy benchmark used the production `TerminalState` history path, a 120x40
source geometry, 80 reflow columns, 32 nearest-rank reflow samples, and reported
aggregate resident bytes for the retained executions. It predates the matrix
harness and did not emit append, search or anchor metrics.

The ten-execution rows were captured with the same command and
`SEYAL_HISTORY_BENCH_EXECUTIONS=10`.

## Retained legacy output

| Retained source lines | Executions | Resident bytes | p50 (ns) | p95 (ns) | p99 (ns) |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 10,000 | 1 | 5,512,757 | 8,833 | 18,959 | 111,959 |
| 100,000 | 1 | 33,541,947 | 8,667 | 21,333 | 107,250 |
| 1,000,000 | 1 | 33,537,653 | 8,834 | 13,041 | 81,208 |
| 10,000 | 10 | 55,127,570 | 92,542 | 108,250 | 968,959 |
| 100,000 | 10 | 335,419,470 | 98,791 | 121,125 | 997,916 |
| 1,000,000 | 10 | 335,376,530 | 92,042 | 115,834 | 826,250 |
| 10,000 | 50 | 275,637,850 | 486,000 | 537,375 | 4,576,250 |
| 10,000 | 100 | 551,275,700 | 974,625 | 1,094,209 | 8,333,958 |

The output included `percentile_method=nearest-rank` and
`performance_claim=false` for every case. The resident values are benchmark
observations; they are not a release-level RSS attribution or a claim against
the physical-host latency gates.

## Current harness comparative smoke

The current exact harness was exercised with 1,000 retained lines, two
executions, columns 40 and 80, all four workload classes and four samples per
execution. `scripts/check-history-benchmark.py` accepted all eight emitted
cases. Each case now emits append, reflow, canonical-search and source-anchor
nearest-rank p50/p95/p99 fields, along with explicit observation counts and
`performance_claim=false`.

Append, search and anchor values are therefore **comparative measurements now
emitted by the harness**, while this smoke run remains too small and sandbox RSS
unavailable for any release or physical-host claim.

## Acceptance ledger

| Gate | Status | Evidence / remaining work |
| --- | --- | --- |
| Exact-head benchmark execution | **Recorded** | The command and output above ran against the measured code head. This documentation commit is evidence-only. |
| 10k/100k/1M single-execution reflow comparison | **Recorded** | Three completed cases above; comparative only. |
| Append latency | **Comparative emitted** | Current harness emits nearest-rank append p50/p95/p99 from up to 32 evenly sized feed chunks per execution; the retained legacy table above has no append values. Full physical acceptance remains unrun. |
| Search and anchor resolution | **Comparative emitted** | Current harness emits nearest-rank canonical-search and source-anchor p50/p95/p99; the retained legacy table above has no values. Full physical acceptance remains unrun. |
| Allocation churn | **Missing** | Allocation calls/bytes are explicitly `not-instrumented` under the benchmark target's `unsafe-code` prohibition. |
| Execution populations 1/10/50/100 | **Automation ready; evidence incomplete** | The 336-case selector includes all populations. Existing retained output covers 1/10 at all three scales and 50/100 only at 10k; the full matrix remains unrun. |
| Required column oscillation 40/48/64/80/96/132/160 | **Automation ready; evidence incomplete** | The selector includes all required widths. Retained legacy output covers only 80 columns; no full-width matrix is recorded. |
| ASCII, styled, CJK, emoji/combining workloads | **Automation ready; evidence incomplete** | The selector and smoke cover all four workload classes, but no normative 10k/100k/1M matrix is recorded. |
| Physical ARM64 p50/p95/p99 acceptance gates | **Unverified** | This run is comparative and does not provide the controlled release matrix or RSS attribution required by #818/#673. |
| Fuzz/property and focused regression tests | **Separate evidence** | See the issue/PR validation record; this file does not replace those results. |
| `make check` / Foundation gates | **Blocked on host** | The exact branch run reached the existing Pass 8 Runtime-to-Swift metadata singleton failure (`AlreadyRunning`). |
| Manual verification | **Unverified** | Headed manual evidence remains required for the user-visible history/reflow cases. |

This record intentionally leaves incomplete and unavailable gates explicit. It
must not be used as the sole basis for merging or closing #819.
