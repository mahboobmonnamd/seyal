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

## Additional bounded shard

The same exact head also completed all four workloads at 1M lines, 10
executions and 40 columns. The per-case validator reported 4 cases passed;
all four cases had `append_observations=10`, `rss_available=true`, and
`allocation_status=measured`.

```text
workload=ascii            rss_delta_kib=748992  allocation_calls=971276244
workload=styled           rss_delta_kib=304384  allocation_calls=1011364484
workload=cjk              rss_delta_kib=404736  allocation_calls=730990644
workload=emoji-combining  rss_delta_kib=357472  allocation_calls=951165624
```

This shard is also comparative only (`performance_claim=false`) and does not
replace the remaining widths, execution populations, physical ARM64
comparison, or headed manual evidence.

## Width-48 shard

A second bounded shard completed all four workloads at 1M lines, 10
executions and 48 columns. The per-case validator reported 4 cases passed;
each case had 10 append observations, available RSS, measured allocation
counters, and exact commit provenance `9e691a9`.

```text
workload=ascii            rss_delta_kib=749632  allocation_status=measured
workload=styled           rss_delta_kib=293776  allocation_status=measured
workload=cjk              rss_delta_kib=403792  allocation_status=measured
workload=emoji-combining  rss_delta_kib=339408  allocation_status=measured
```

The raw validator output is retained at
`/tmp/seyal-819-1m-10-width48-XXXXXX.log`. This remains comparative evidence
only and does not close the remaining matrix or physical/manual gates.

## Width-64 shard

A third bounded shard completed all four workloads at 1M lines, 10 executions
and 64 columns. The per-case validator reported 4 cases passed; each case
had 10 append observations, available RSS, measured allocation counters, and
exact commit provenance `9e691a9`.

```text
workload=ascii            rss_delta_kib=749920  allocation_status=measured
workload=styled           rss_delta_kib=304720  allocation_status=measured
workload=cjk              rss_delta_kib=403424  allocation_status=measured
workload=emoji-combining  rss_delta_kib=94560   allocation_status=measured
```

The raw validator output is retained at
`/tmp/seyal-819-1m-10-width64-XXXXXX.log`. This remains comparative evidence
only and does not close the remaining matrix or physical/manual gates.

## Width-80 shard

A fourth bounded shard completed all four workloads at 1M lines, 10
executions and 80 columns. The per-case validator reported 4 cases passed;
each case had 10 append observations, available RSS, measured allocation
counters, and exact commit provenance `9e691a9`.

```text
workload=ascii            rss_delta_kib=748864  allocation_status=measured
workload=styled           rss_delta_kib=304368  allocation_status=measured
workload=cjk              rss_delta_kib=403792  allocation_status=measured
workload=emoji-combining  rss_delta_kib=339216  allocation_status=measured
```

The raw validator output is retained at
`/tmp/seyal-819-1m-10-width80-XXXXXX.log`. This remains comparative evidence
only and does not close the remaining matrix or physical/manual gates.

## Width-96 shard

A fifth bounded shard completed all four workloads at 1M lines, 10
executions and 96 columns. The per-case validator reported 4 cases passed;
each case had 10 append observations, available RSS, measured allocation
counters, and exact commit provenance `9e691a9`.

```text
workload=ascii            rss_delta_kib=748880  allocation_status=measured
workload=styled           rss_delta_kib=305696  allocation_status=measured
workload=cjk              rss_delta_kib=404576  allocation_status=measured
workload=emoji-combining  rss_delta_kib=328384  allocation_status=measured
```

The raw validator output is retained at
`/tmp/seyal-819-1m-10-width96-XXXXXX.log`. This remains comparative evidence
only and does not close the remaining matrix or physical/manual gates.

## Width-132 shard

A sixth bounded shard completed all four workloads at 1M lines, 10 executions
and 132 columns. The per-case validator reported 4 cases passed; each case
had 10 append observations, available RSS, measured allocation counters, and
exact commit provenance `9e691a9`.

```text
workload=ascii            rss_delta_kib=749056  allocation_status=measured
workload=styled           rss_delta_kib=304720  allocation_status=measured
workload=cjk              rss_delta_kib=374336  allocation_status=measured
workload=emoji-combining  rss_delta_kib=345392  allocation_status=measured
```

The raw validator output is retained at
`/tmp/seyal-819-1m-10-width132-XXXXXX.log`. This remains comparative evidence
only and does not close the remaining matrix or physical/manual gates.
