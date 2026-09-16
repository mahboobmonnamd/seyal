# M002 #842 comparative HistoryStore StatsAlloc tip matrix — Linux x86_64

| Field | Value |
| --- | --- |
| Production SHA | `bd99e15620e2d75a039c0e42b6f26137962243f5` |
| Relationship | `Refs #842` — not `Closes` |
| Host | Linux x86_64 cloud agent |
| Evidence class | `SYNTHETIC` / TerminalState-comparative |
| `performance_claim` | `false` |

## Scope

Reproduce allocation-instrumented HistoryStore comparative rows on current
master tip after #947. This advances #842 gate 2 (allocation/resource
instrumentation on the tip) without claiming PHYSICAL_ARM64 release ceilings
or #673 five-cohort acceptance.

## Harness

```text
SEYAL_BENCH_COMMIT=<tip>
SEYAL_HISTORY_BENCH_SAMPLES=4
SEYAL_HISTORY_BENCH_LINES=2000
SEYAL_HISTORY_BENCH_EXECUTIONS=1
SEYAL_HISTORY_BENCH_COLUMNS=40,80
SEYAL_HISTORY_BENCH_WORKLOADS=ascii,cjk,emoji-combining
cargo bench -p seyal-terminal --bench history_reflow --features history-reflow-bench --locked
```

Raw log: `docs/evidence/m002-history-842-comparative-bd99e15.log`

Validator: `python3 scripts/check-history-benchmark.py docs/evidence/m002-history-842-comparative-bd99e15.log` → pass.

## Results (summary)

All six cases report `allocation_status=measured`, `performance_claim=false`,
and `evidence_scope=TerminalState-comparative`. RSS is available on this Linux
host; values are comparative only.

| Workload | Cols | resident_history_bytes | allocation_calls | allocated_bytes | reflow_p50_ns |
| --- | --- | --- | --- | --- | --- |
| ascii | 40 | 1053104 | 217501 | 13715022 | 281946 |
| ascii | 80 | 1053104 | 217501 | 13715022 | 304606 |
| cjk | 40 | 815182 | 164209 | 11357908 | 217022 |
| cjk | 80 | 815182 | 164209 | 11357908 | 223689 |
| emoji-combining | 40 | 946022 | 210019 | 12752100 | 261257 |
| emoji-combining | 80 | 946022 | 210019 | 12752100 | 243765 |

## Explicitly not claimed

- PHYSICAL_ARM64 / Apple Silicon Release ceilings vs #818/#673
- Five-cohort raw TOML release gate
- Full 10k/100k/1M × 1/10/50/100 × full column ladder
- Independent closing review of #819/#842

Keep #819 and #842 open.
