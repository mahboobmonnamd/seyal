# Issue #687 — scoped persistence decision draft

**Status:** proposed research output only; this is not an accepted ADR, schema,
implementation specification, or production authorization.

**Authority consulted:** ADR-007 (workspace identity/persistence classes),
ADR-010 (immutable retained-history segments), ADR-011 (canonical Unicode
payload), ADR-006 (hot-path isolation), Issue #687, and the M004 roadmap.

## Proposed decision

Adopt a **hybrid durability boundary** for M004, subject to a new accepted ADR
and implementation specification:

1. **Typed metadata:** a local SQLite database in WAL mode stores Workspace,
   presentation/layout, execution metadata, Block references, reconnect
   metadata, schema/version records, and explicit retention/redaction records.
   Every user-visible mutation (create/update/reorder/close) is one bounded
   transaction. Migration is versioned, transactional, resumable or fails
   closed before exposing the new schema.
2. **Cold history:** ADR-010 sealed immutable history segments are persisted as
   checksummed, versioned payload files referenced by a metadata manifest. The
   live `TerminalState` remains the only canonical history authority. Segment
   writes, compression, indexing and fsync are asynchronous consumers and can
   never gate `PTY -> VT -> TerminalState -> damage` progress.
3. **Snapshots/checkpoints:** atomic snapshots are recovery/export/checkpoint
   aids, not a second mutable authority. A full append journal is not the
   canonical workspace store; an optional bounded audit trail must have a
   separate retention and redaction contract.
4. **Honest recovery:** persisted execution records are last-known metadata.
   Runtime remains authoritative for live PTYs. If Runtime is absent or its
   identity does not match, recovery marks executions `not_claimed`/unknown,
   clears attachment state, and never restores a PTY claim. A replacement
   execution always receives a new non-reused `ExecutionId`; old records are
   finalized or explicitly unavailable, never resurrected.
5. **Redaction/deletion:** redaction is an explicit state transition that
   immediately hides the payload from all reads and creates a bounded cleanup
   obligation. Physical removal (or cryptographic key erasure when encrypted)
   must be verified before claiming at-rest deletion. Retention truncation is
   deterministic and anchor resolution returns evicted/unavailable rather than
   substituting another line.

This proposal is deliberately scoped to local workspace metadata, bounded cold
history and Runtime-absent recovery. It does not decide cloud sync, remote
multi-writer conflict resolution, agent event storage, or live PTY survival
across Runtime/process loss.

## Evidence from the isolated comparator

Command:

```sh
python3 experiments/issue-687/persistence_spike.py \
  --scale 100 --commits 3 --repetitions 20 \
  --output experiments/issue-687/results.json
```

Environment: Darwin 25.5.0, Python 3.14.6, SQLite 3.53.4. Workload is a
stdlib-only research comparator; the numbers are not product budgets and do
not measure real PTY/VT latency. Commit percentiles use nearest-rank over 60
samples (20 repetitions × 3 measured commits); recovery and migration
percentiles use 20 samples.

| model | commit p50/p95 ms | recovery p50/p95 ms | migration p50/p95 ms | disk bytes p50 | raw secret artifacts max | independent observations |
|---|---:|---:|---:|---:|---:|---:|
| SQLite/WAL typed comparator | 0.490 / 0.545 | 1.029 / 1.108 | 0.869 / 0.949 | 506,168 | 0 | passed |
| append journal + replay | 2.179 / 2.333 | 10.976 / 11.190 | 13.415 / 13.582 | 1,155,932 | **2** | passed |
| atomic full snapshot | 0.378 / 0.449 | 0.585 / 0.667 | 0.517 / 0.567 | 51,537 | 0 | passed |
| hybrid metadata + segments | 9.447 / 10.883 | 8.426 / 8.827 | 8.318 / 8.867 | 945,438 | 0 | passed |

All models passed independent-reader committed-state fingerprints, migration downgrade
refusal, no live PTY claim after Runtime absence, no duplicate execution
identity, and logical redaction checks. The journal still retained two raw
secret artifacts in old committed records, demonstrating why replay
correctness does not establish redaction/deletion correctness.

The hybrid comparator now writes immutable content-addressed/versioned segment
objects, publishes typed manifest references only after object durability, and
validates checksum, source identity and line range on recovery. Missing or
corrupt objects become explicit `unavailable` history entries. Fault evidence
passed for before-object, after-object-before-manifest and after-manifest
publication; existing retained objects were not overwritten. These are
comparator semantics, not production acceptance.

## Why the alternatives are not the primary decision

* **SQLite alone:** good transaction/recovery surface for metadata, but placing
  large retained history in ordinary mutable rows risks write amplification and
  keeps cold content coupled to metadata. It remains the metadata authority in
  the proposal.
* **Append journal + materialized views:** excellent audit/replay lineage, but
  replay grows with history and old records retain redacted bytes unless the
  format has authenticated encryption/key erasure plus compaction. It can be an
  optional bounded audit stream, not the sole redaction-capable store.
* **Full snapshots:** simplest atomic recovery and small fixture footprint, but
  every layout/metadata update rewrites the retained graph/history and offers no
  natural immutable segment handoff. Keep as checkpoint/export tooling.
* **Hybrid:** aligns with ADR-007's persistence classes and ADR-010's sealed
  segment boundary, provided metadata transactions, immutable content-addressed
  objects, explicit unavailable disposition, cleanup, and queue saturation
  behavior are specified before implementation.

## Required acceptance gates before production implementation

* accepted ADR/spec with exact schema/version ownership and migration authority;
* real Runtime integration proving GUI detach/reconnect does not terminate a
  live PTY and Runtime-absent recovery never claims one;
* subprocess crash/failure injection at every metadata and segment commit
  boundary, including repeated/N-times failure and restart;
* concurrent reader/writer tests with bounded queues and no partial graph
  observations;
* malformed/truncated/checksum-invalid metadata and segment fixtures;
* retention/truncation/redaction tests that inspect both logical recovery and
  physical/encrypted at-rest bytes;
* migration fencing tests proving an older writer cannot downgrade a committed
  schema and rollback/restart behavior is explicit;
* macOS startup restore, commit, migration, reflow/history paging, RSS/disk
  growth and write-amplification measurements at representative workspace and
  history scales;
* security review covering file permissions, secrets, crash remnants, backup/
  WAL handling, deletion, authorization and path confinement;
* independent review; no production code from this spike is promotable.

The repaired harness still does not provide subprocess-level crash/restart
proof, real Runtime/PTY behavior, GUI detach/reconnect behavior, or a production
queue/backpressure measurement. Those remain mandatory external gates.

## #688 reconciliation gate (do not freeze here)

Issue #688 owns signed/notarized executable delivery, staged replacement,
rollback and update UX. Before either issue's implementation becomes Ready, the
owners must reconcile a shared compatibility matrix covering:

* executable build/schema/protocol versions and minimum readable/writable
  metadata versions;
* migration ownership and whether a GUI update may run while the Runtime/PTY
  authority remains on the old executable;
* GUI reconnect behavior after update, including incompatible-version refusal,
  stale-socket handling and no false live-PTY handoff claim;
* crash/rollback behavior when a metadata migration has committed but the new
  GUI or Runtime executable is rolled back;
* location/permissions/backup handling for the SQLite database, WAL and cold
  segments under signed app replacement.

This is a **reconciliation gate**, not an overlapping #688 contract. #687
decides persistence semantics and recovery truth; #688 decides package/update
trust and atomic delivery. Neither issue may silently invent the other's
compatibility or executable lifecycle rules.

## Unknowns intentionally left open

The exact SQLite schema, segment serialization/codec, encryption/key-erasure
scheme, compaction cadence, per-workspace/aggregate byte limits, migration
rollback policy, backup policy, and macOS release thresholds require a follow-up
architecture/spec issue and real production-path measurements. No number in
`results.json` is an accepted limit.
