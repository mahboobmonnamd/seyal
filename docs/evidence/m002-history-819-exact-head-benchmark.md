# M002 #819 exact-head history benchmark record

- **Issue:** #819
- **Authority:** ADR-010 and SPEC-010, with the frozen budgets from #818
- **Measured production code head:** `9cd8e3976a8d34d598d702deebe66c2b7694cd12`
- **Benchmark harness head:** `e9aa0a40e8aa458c83983f5ca9eaff63e708f312` (canonical branch exact head)
- **Issue branch exact head:** `b783230d2b9cc8f355e0d3e069ebbaa7b08fd5ba`
- **Recorded:** 2026-09-08
- **Host/build boundary:** local Apple Silicon macOS host, ARM64 Release Cargo benchmark for the retained legacy run; current harness smoke runs are comparative only
- **Claim status:** comparative evidence only (`performance_claim=false`); merge gates remain incomplete

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

## Current 10k matrix slice

The exact `c9030fa` harness also ran the complete 10k-line slice across all four
execution populations, seven required column widths and four workload classes:
112 cases, one execution per population, ten append samples per execution.
The validator accepted every emitted case. The raw machine output is retained
in `m002-history-819-10k-comparative.log`; RSS was unavailable and every case
keeps `performance_claim=false`.

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
| `make check` / Foundation gates | **Passed** | Exact-head `make check` exited 0, including ARM64 build, native shell smoke, Runtime-to-Swift metadata, and live renderer checks. |
| Manual verification | **Unverified** | Headed manual evidence remains required for the user-visible history/reflow cases. |

This record intentionally leaves incomplete and unavailable gates explicit. It
must not be used as the sole basis for merging or closing #819.

## Matrix harness automation

The exact tip also retains a reproducible matrix harness in
`crates/seyal-terminal/benches/history_reflow.rs`. `SEYAL_HISTORY_BENCH_FULL=1`
enumerates the complete SPEC-010 shape: 10k/100k/1M retained lines,
1/10/50/100 `TerminalState` populations, 40/48/64/80/96/132/160 columns and
ASCII/styled/CJK/emoji-combining workloads. Each case reports nearest-rank
append/reflow/search/anchor p50/p95/p99, resident and derived-cache bytes, and
best-effort process RSS. `scripts/check-history-benchmark.py
--require-full-matrix` verifies that all 336 case keys are present and rejects
missing required metrics or a `performance_claim=true` claim.

The automation does not close the missing gates by itself. Its evidence scope is
explicitly `TerminalState-comparative`; Runtime aggregate eviction/resource
behavior, physical ARM64 Release latency/RSS attribution, and manual headed
verification remain separate. Allocation counters are reported as
`not-instrumented` because the benchmark inherits the repository's
`unsafe-code` prohibition, so allocation churn remains missing until a safe,
accepted instrumentation boundary exists.

Append percentile sampling is now explicit: each execution feeds its exact
requested line count in up to `SEYAL_HISTORY_BENCH_SAMPLES` evenly sized chunks,
and the case records `append_observations` plus
`append_samples_per_execution`. The full-matrix validator requires at least ten
append observations per execution before accepting the matrix shape. These are
chunk append latencies, rather than repeated full-history construction times;
the ledger still contains no physical performance claim.
