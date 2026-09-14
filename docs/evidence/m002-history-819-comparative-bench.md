# M002 #819 comparative history_reflow rerun

- **Date:** 2026-09-11
- **Host:** macOS arm64 Release `cargo bench`
- **Recorded commit env:** `SEYAL_BENCH_COMMIT=2a5ebc535dc883dffb9e68113b9ae24cadc1b50f`
- **Does not prove current HEAD.** These rows are historical comparative
  direction at that parent SHA. They are not exact-head evidence for later
  commits and must not be read as SPEC-010 §18.1 acceptance.
- **Classification:** TerminalState-comparative only (`performance_claim=false`)

Command:

```sh
SEYAL_HISTORY_BENCH_LINES=10000 \
SEYAL_HISTORY_BENCH_EXECUTIONS=1 \
SEYAL_HISTORY_BENCH_COLUMNS=40,80,160 \
SEYAL_HISTORY_BENCH_WORKLOADS=ascii,cjk \
SEYAL_HISTORY_BENCH_SAMPLES=10 \
SEYAL_BENCH_COMMIT=$(git rev-parse HEAD) \
cargo bench -p seyal-terminal --bench history_reflow --features history-reflow-bench --locked -- --quiet
```

| workload | cols | append p50 ns | reflow p50 ns | reflow p95 ns | allocation_status |
| --- | ---: | ---: | ---: | ---: | --- |
| ascii | 40 | 6_376_958 | 48_333 | 91_250 | measured |
| ascii | 80 | 6_268_667 | 46_750 | 55_333 | measured |
| ascii | 160 | 6_268_208 | 48_583 | 65_250 | measured |
| cjk | 40 | 5_783_709 | 44_375 | 72_083 | measured |
| cjk | 80 | 5_731_916 | 44_750 | 49_916 | measured |
| cjk | 160 | 5_745_208 | 45_750 | 51_208 | measured |

RSS was unavailable in this run (`rss_available=false`). Do not treat these
rows as SPEC-010 §18.1 acceptance.
