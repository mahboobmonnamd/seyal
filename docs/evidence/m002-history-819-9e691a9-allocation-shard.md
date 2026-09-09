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

## Width-160 shard

The final bounded width shard completed all four workloads at 1M lines, 10
executions and 160 columns. The per-case validator reported 4 cases passed;
each case had 10 append observations, available RSS, measured allocation
counters, and exact commit provenance `9e691a9`.

```text
workload=ascii            rss_delta_kib=748864  allocation_status=measured
workload=styled           rss_delta_kib=304400  allocation_status=measured
workload=cjk              rss_delta_kib=403904  allocation_status=measured
workload=emoji-combining  rss_delta_kib=211440  allocation_status=measured
```

The raw validator output is retained at
`/tmp/seyal-819-1m-10-width160-XXXXXX.log`. This remains comparative evidence
only and does not close the remaining execution populations or physical/manual
gates.

## 1M-line, one-execution cohort

The exact head also completed the full 28-case width/workload cohort at 1M
lines, 1 execution and `samples=10`. The validator reported 28 cases passed;
each case therefore has 10 append observations, available RSS, measured
allocation counters, and exact commit provenance `9e691a9`.

This remains comparative evidence (`performance_claim=false`) and does not
close the 10/50/100 execution populations, physical ARM64 comparison,
headed manual history/reflow evidence, or independent review.

The raw validator output is retained at
`/tmp/seyal-819-1m-1-samples10-full-XXXXXX.log`.

## 10k-line full scaling tier

The exact head completed the complete 10k-line tier across 1, 10, 50 and 100
executions, all seven widths, and all four workloads, using `samples=10`.
The validator reported 112 cases passed. Every case had at least 10 append
observations per execution, available RSS, measured allocation counters, and
exact commit provenance `9e691a9`.

This remains comparative evidence (`performance_claim=false`) and does not
establish product performance acceptance or replace the physical ARM64 lane.
The raw validator output is retained at
`/tmp/seyal-819-10k-full-samples10-XXXXXX.log`.

## 100k-line lower-population cohorts

The exact head completed the 100k-line cohorts for 1 and 10 executions across
all seven widths and four workloads, using `samples=10`. The validator
reported 56 cases passed. Every case had at least 10 append observations per
execution, available RSS, measured allocation counters, and exact commit
provenance `9e691a9`.

This remains comparative evidence (`performance_claim=false`) and does not
close the 50/100 execution matrix, physical ARM64 comparison, or headed
manual history/reflow gates. The raw validator output is retained at
`/tmp/seyal-819-100k-1-10-full-samples10-XXXXXX.log`.

## 100k-line scaling point

A bounded resource shard also completed at 100k lines, 50 executions and 40
columns for the ASCII workload. The per-case validator reported 1 case
passed with 50 append observations, available RSS, measured allocation
counters, and exact commit provenance `9e691a9`.

```text
rss_delta_kib=1861600
allocation_calls=485574564
allocated_bytes=30380428082
deallocated_bytes=28573663382
allocation_status=measured
performance_claim=false
```

The raw validator output is retained at
`/tmp/seyal-819-100k-50-ascii-width40-XXXXXX.log`. This is comparative
resource evidence only and does not establish the complete execution matrix
or product performance acceptance.

The corresponding bounded 100-execution point also completed for ASCII at
100k lines and 40 columns. The per-case validator reported 1 case passed with
100 append observations, available RSS, measured allocation counters, and
exact commit provenance `9e691a9`:

```text
rss_delta_kib=1784016
allocation_calls=971149114
allocated_bytes=60760856032
deallocated_bytes=57147326632
allocation_status=measured
performance_claim=false
```

The raw validator output is retained at
`/tmp/seyal-819-100k-100-ascii-width40-XXXXXX.log`. This is comparative
resource evidence only and does not establish product performance acceptance.

## 100k-line, 50-execution Unicode shard

A bounded high-population shard completed at 100k lines, 50 executions and 40
columns for all four workloads. The validator reported 4 cases passed; each
case had 50 append observations, available RSS, measured allocation counters,
and exact commit provenance `9e691a9`.

```text
workload=ascii            rss_delta_kib=1937296  allocation_status=measured
workload=styled           rss_delta_kib=125200   allocation_status=measured
workload=cjk              rss_delta_kib=0        allocation_status=measured
workload=emoji-combining  rss_delta_kib=1078704  allocation_status=measured
```

The raw validator output is retained at
`/tmp/seyal-819-100k-50-width40-samples10-XXXXXX.log`. The zero RSS delta is
reported as observed and is not treated as an inferred or corrected value.
This remains comparative evidence only.

## Physical ARM64 1M-line, one-execution width-40 cohort

The bounded 1M-line cohort completed under the physical Apple Silicon boundary
at width 40 for ASCII, styled, CJK, and emoji-combining workloads. The
validator reported 4 cases passed; each case had 10 append observations,
available RSS, measured allocation counters, and exact commit provenance
`9e691a9`.

```text
workload=ascii            rss_delta_kib=106864  allocation_status=measured
workload=styled           rss_delta_kib=1184    allocation_status=measured
workload=cjk              rss_delta_kib=0       allocation_status=measured
workload=emoji-combining  rss_delta_kib=75008   allocation_status=measured
```

The zero CJK RSS delta is reported as observed and is not treated as inferred
or corrected. The raw validator output is retained at
`/tmp/seyal-819-1m-1-width40-arm64-samples10-XXXXXX.log`. This is one
physical ARM64 comparative cohort and does not establish the complete
performance contract or product acceptance.

## Physical ARM64 100k-line, 100-execution width-48 shard

The same bounded cohort was rerun under the physical Apple Silicon boundary
(`arm64`, 24 GiB host memory) with RSS measurement available. The validator
reported 4 cases passed; each case had 1,000 append observations, measured
allocation counters, and exact commit provenance `9e691a9`.

```text
workload=ascii            rss_delta_kib=1968624  allocation_status=measured
workload=styled           rss_delta_kib=0        allocation_status=measured
workload=cjk              rss_delta_kib=238048   allocation_status=measured
workload=emoji-combining  rss_delta_kib=0        allocation_status=measured
```

The zero RSS deltas are reported as observed and are not treated as inferred
or corrected values. The raw validator output is retained at
`/tmp/seyal-819-100k-100-width48-arm64-samples10-XXXXXX.log`. This is one
physical ARM64 comparative cohort only; it does not establish the complete
performance contract or product acceptance.

## 100k-line, 50-execution width-48 shard

A bounded shard completed at 100k lines, 50 executions and 48 columns for all
four workloads. The validator reported 4 cases passed; each case had 500
append observations, measured allocation counters, and exact commit
provenance `9e691a9`. RSS was unavailable for this run (`rss_available=false`)
and is therefore not used as evidence.

```text
workload=ascii            allocation_calls=485807213  allocation_status=measured
workload=styled           allocation_calls=505848713  allocation_status=measured
workload=cjk              allocation_calls=365685613  allocation_status=measured
workload=emoji-combining  allocation_calls=475770113  allocation_status=measured
```

The raw validator output is retained at
`/tmp/seyal-819-100k-50-width48-samples10-XXXXXX.log`. This remains
comparative evidence only and does not replace physical ARM64 comparison or
the remaining headed and independent-review gates.

## 100k-line, 100-execution width-48 shard

A bounded high-population shard completed at 100k lines, 100 executions and 48
columns for all four workloads. The validator reported 4 cases passed; each
case had 1,000 append observations, measured allocation counters, and exact
commit provenance `9e691a9`. RSS was unavailable for this run
(`rss_available=false`) and is therefore not used as evidence.

```text
workload=ascii            allocation_calls=971614413   allocation_status=measured
workload=styled           allocation_calls=1011697413  allocation_status=measured
workload=cjk              allocation_calls=731371213   allocation_status=measured
workload=emoji-combining  allocation_calls=951540213   allocation_status=measured
```

The raw validator output is retained at
`/tmp/seyal-819-100k-100-width48-samples10-XXXXXX.log`. This remains
comparative evidence only; the observed Unicode latency tails are retained as
measurements and are not converted into an acceptance claim.

## 100k-line, 50-execution width-160 shard

A bounded shard completed at 100k lines, 50 executions and 160 columns for
all four workloads. The validator reported 4 cases passed; each case had 500
append observations, measured allocation counters, and exact commit
provenance `9e691a9`. RSS was unavailable for this run (`rss_available=false`)
and is therefore not used as evidence.

```text
workload=ascii            allocation_calls=485807213  allocation_status=measured
workload=styled           allocation_calls=505848713  allocation_status=measured
workload=cjk              allocation_calls=365685613  allocation_status=measured
workload=emoji-combining  allocation_calls=475770113  allocation_status=measured
```

The raw validator output is retained at
`/tmp/seyal-819-100k-50-width160-samples10-XXXXXX.log`. This remains
comparative evidence only and does not replace physical ARM64 comparison or
the remaining headed and independent-review gates.

## 100k-line, 50-execution width-132 shard

A bounded shard completed at 100k lines, 50 executions and 132 columns for
all four workloads. The validator reported 4 cases passed; each case had 500
append observations, measured allocation counters, and exact commit
provenance `9e691a9`. RSS was unavailable for this run (`rss_available=false`)
and is therefore not used as evidence.

```text
workload=ascii            allocation_calls=485807213  allocation_status=measured
workload=styled           allocation_calls=505848713  allocation_status=measured
workload=cjk              allocation_calls=365685613  allocation_status=measured
workload=emoji-combining  allocation_calls=475770113  allocation_status=measured
```

The raw validator output is retained at
`/tmp/seyal-819-100k-50-width132-samples10-XXXXXX.log`. This remains
comparative evidence only and does not replace physical ARM64 comparison or
the remaining headed and independent-review gates.

## 100k-line, 50-execution width-96 shard

A bounded shard completed at 100k lines, 50 executions and 96 columns for all
four workloads. The validator reported 4 cases passed; each case had 500
append observations, measured allocation counters, and exact commit
provenance `9e691a9`. RSS was unavailable for this run (`rss_available=false`)
and is therefore not used as evidence.

```text
workload=ascii            allocation_calls=485807213  allocation_status=measured
workload=styled           allocation_calls=505848713  allocation_status=measured
workload=cjk              allocation_calls=365685613  allocation_status=measured
workload=emoji-combining  allocation_calls=475770113  allocation_status=measured
```

The raw validator output is retained at
`/tmp/seyal-819-100k-50-width96-samples10-XXXXXX.log`. This remains
comparative evidence only and does not replace physical ARM64 comparison or
the remaining headed and independent-review gates.

## 100k-line, 50-execution width-80 shard

A bounded shard completed at 100k lines, 50 executions and 80 columns for all
four workloads. The validator reported 4 cases passed; each case had 500
append observations, measured allocation counters, and exact commit
provenance `9e691a9`. RSS was unavailable for this run (`rss_available=false`)
and is therefore not used as evidence.

```text
workload=ascii            allocation_calls=485807213  allocation_status=measured
workload=styled           allocation_calls=505848713  allocation_status=measured
workload=cjk              allocation_calls=365685613  allocation_status=measured
workload=emoji-combining  allocation_calls=475770113  allocation_status=measured
```

The raw validator output is retained at
`/tmp/seyal-819-100k-50-width80-samples10-XXXXXX.log`. This remains
comparative evidence only and does not replace physical ARM64 comparison or
the remaining headed and independent-review gates.

## 100k-line, 50-execution width-64 shard

A bounded shard completed at 100k lines, 50 executions and 64 columns for all
four workloads. The validator reported 4 cases passed; each case had 500
append observations, measured allocation counters, and exact commit
provenance `9e691a9`. RSS was unavailable for this run (`rss_available=false`)
and is therefore not used as evidence.

```text
workload=ascii            allocation_calls=485807213  allocation_status=measured
workload=styled           allocation_calls=505848713  allocation_status=measured
workload=cjk              allocation_calls=365685613  allocation_status=measured
workload=emoji-combining  allocation_calls=475770113  allocation_status=measured
```

The raw validator output is retained at
`/tmp/seyal-819-100k-50-width64-samples10-XXXXXX.log`. This remains
comparative evidence only and does not replace physical ARM64 comparison or
the remaining headed and independent-review gates.

## 100k-line, 100-execution Unicode shard

A bounded high-population shard completed at 100k lines, 100 executions and 40
columns for all four workloads. The validator reported 4 cases passed; each
case had 1,000 append observations, available RSS, measured allocation
counters, and exact commit provenance `9e691a9`.

```text
workload=ascii            rss_delta_kib=2085504  allocation_status=measured
workload=styled           rss_delta_kib=0        allocation_status=measured
workload=cjk              rss_delta_kib=0        allocation_status=measured
workload=emoji-combining  rss_delta_kib=0        allocation_status=measured
```

The raw validator output is retained at
`/tmp/seyal-819-100k-100-width40-samples10-XXXXXX.log`. The zero RSS deltas
are reported as observed and are not treated as inferred or corrected values.
This remains comparative evidence only.
