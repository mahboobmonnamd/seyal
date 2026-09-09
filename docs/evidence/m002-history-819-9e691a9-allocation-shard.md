# M002 #819 exact-head allocation shard

- **Production head:** `9e691a9`
- **Host:** local Apple Silicon macOS
- **Scope:** `TerminalState` comparative benchmark only
- **Claim:** `performance_claim=false`

## Command

```text
SEYAL_HISTORY_BENCH_FULL=1 \
SEYAL_HISTORY_BENCH_LINES=1000000 \
SEYAL_HISTORY_BENCH_EXECUTIONS=50 \
SEYAL_HISTORY_BENCH_WORKLOADS=ascii \
SEYAL_HISTORY_BENCH_SAMPLES=1 \
SEYAL_BENCH_COMMIT=9e691a9 \
cargo bench -p seyal-terminal --bench history_reflow --locked -- --quiet
```

## Retained result

The first two width cases completed and passed the per-case validator. The
first completed case (`columns=40`) reported:

```text
append_observations=50
rss_available=true
rss_delta_kib=1687584
allocation_calls=4856381164
allocated_bytes=304950602482
deallocated_bytes=301472966582
allocation_status=measured
performance_claim=false
```

The shard was terminated at the host-resource boundary after resident memory
grew to approximately 3 GiB while the next width was still computing. The
complete seven-width/workload matrix was not produced by this shard, so this
record is partial evidence only and does not satisfy the #819 acceptance
matrix, physical ARM64 comparison, or performance-claim gate.

Validation of the retained output:

```text
python3 scripts/check-history-benchmark.py /tmp/seyal-819-1m-50-ascii-XXXXXX.log
[seyal history benchmark contract] 2 case(s) passed.
```

Remaining gates are recorded on #819: complete retained evidence at the
instrumented head, accepted ARM64 comparison under #673, headed
history/reflow verification, and independent exact-head review.
