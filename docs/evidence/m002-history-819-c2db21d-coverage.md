# #819 exact-head coverage attempt

- **Issue:** #819 / #842
- **Production branch:** `issue/819`
- **Exact head:** `c2db21d`
- **Host:** Apple Silicon macOS, ARM64 Release benchmark
- **Evidence class:** `PHYSICAL_ARM64` execution attempt, comparative only
- **Claim status:** incomplete; no release gate is claimed

## Commands

The complete selector was split into four independent execution-population
shards, each using one sample per case to establish coverage without claiming
percentile acceptance:

```sh
SEYAL_HISTORY_BENCH_FULL=1 SEYAL_HISTORY_BENCH_SAMPLES=1 \
SEYAL_HISTORY_BENCH_EXECUTIONS=<1|10|50|100> \
TMPDIR=/tmp cargo bench -p seyal-terminal --bench history_reflow --locked -- --quiet
```

## Observed result

The 1-execution shard completed with 84 emitted cases. The validator correctly
reported that the full matrix was incomplete and that one sample per execution
cannot satisfy the append-observation requirement. The 10-execution shard
advanced through 1M-line cases but was stopped after sustained host pressure
and no progress. The 50- and 100-execution shards were stopped at the same
boundary after emitting partial rows. Their logs reported RSS unavailable and
resident-history targets of approximately 1.68 GiB and 3.35 GiB at the
observed 1M/50 and 100k/100 cases respectively.

The raw logs remain outside the repository at:

- `/tmp/seyal-history-819-c2db21d-one.log`
- `/tmp/seyal-history-819-c2db21d-ten.log`
- `/tmp/seyal-history-819-c2db21d-fifty.log`
- `/tmp/seyal-history-819-c2db21d-hundred.log`

Every emitted record includes `percentile_method=nearest-rank`,
`performance_claim=false`, and `evidence_scope=TerminalState-comparative`.

## Gate disposition

This attempt proves the exact-head harness executes and exposes the resource
boundary, but it does **not** prove the 336-case matrix, repeated physical
cohorts, allocation instrumentation, or the #818/#673 release ceilings. The
missing rows and unavailable RSS remain open in #842; no result is estimated,
backfilled, or reclassified as a product pass.
