# Issue #687 persistence spike (research-only)

This directory is an isolated, non-mergeable experiment. It is not a Runtime,
PTY, GUI, history, schema, or update implementation. The script compares the
four storage models named by Issue #687 using only Python's standard library:

* `sqlite`: typed metadata in SQLite/WAL with `synchronous=FULL`;
* `journal`: checksummed append records with replay that stops at a torn tail;
* `snapshot`: fsync + atomic replacement of one JSON snapshot;
* `hybrid`: SQLite metadata plus immutable, independently fsynced history files.

The generated workload performs create/update/reorder/close, history
redaction/truncation, detach, replacement execution creation, a simulated
crash window, recovery with Runtime absent, and schema migration. The output
also checks that recovery does not resurrect a PTY or retain redacted history.

## Reproduction

```sh
python3 experiments/issue-687/persistence_spike.py \
  --scale 100 --commits 3 --repetitions 20 \
  --output experiments/issue-687/results.json
```

The output includes environment metadata, nearest-rank p50/p95 commit latency
over 60 samples, recovery/migration latency over 20 samples, disk footprint,
schema-downgrade fencing, independent-reader committed-state fingerprints,
content-addressed segment fault/recovery assertions, and a synthetic
background-writer overlap measurement. It also scans comparator files for the
fixture's redacted secret: logical replay can pass while an append journal
still retains old bytes. The latter overlap measurement is a scheduler/I/O
diagnostic only; it is not PTY/VT latency evidence.

The accompanying `DECISION-DRAFT.md` proposes a hybrid metadata/immutable
history boundary and records the required #688 executable/schema/GUI-runtime
compatibility reconciliation gate. It is a proposal, not accepted authority.

The exact model cannot be selected from this experiment alone. Before M004
production implementation, the selected design needs an accepted ADR/spec,
macOS measurements, subprocess crash/restart and repeated fault-injection
tests, bounded queue/backpressure tests, authorization/security review,
backup/WAL/encryption deletion analysis, real Runtime/PTY detach-reconnect
evidence, and an independent implementation review. `results.json` is the
current machine-readable run; the harness remains research-only.
