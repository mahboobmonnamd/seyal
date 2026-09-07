# Issue #687 real Runtime/PTY evidence plan

This is a research coordination artifact, not an ADR, specification, or
production acceptance record. It does not change the Runtime or persistence
implementation. The companion `run-real-runtime-evidence.sh` invokes existing
macOS integration tests against the real Runtime, real PTY and real local UDS.

## Master-contract dependency review

Master `9365fa9` (#825) freezes SPEC-011's M002 Unicode production profile.
Persistence remains explicitly out of scope there, but #687 must preserve the
following canonical data rather than serializing renderer/client projections:

* Unicode semantic data version **17.0.0**;
* active canonical grapheme payload maximum **8,192 UTF-8 bytes**;
* live variable grapheme storage maximum **2 MiB per `TerminalState`**;
* canonical payload or overflow sentinel, terminal width, style and explicit
  lead/continuation semantics;
* hard-break/soft-wrap lineage and canonical text-unit anchors from ADR-010;
* exact DECAWM-reset behavior: an impossible new width-2 unit is ignored
  atomically, while late width-changing extension is rejected without deleting
  the committed width-1 prefix;
* grapheme display v2 (`CAP_GRAPHEME_DISPLAY`, messages 27/28, schema 2) is a
  presentation protocol, not a persistence schema or history authority.

The frozen profile therefore changes #687's dependency wording, not its storage
decision: persistence must consume canonical `TerminalState`/ADR-010 history
units after their production implementation is accepted, and must never persist
display sidecar bytes as the source history.

## What existing real evidence can establish

| #687 question | Existing real path | Evidence boundary |
|---|---|---|
| GUI detach/reattach while PTY remains live | `macos_runtime::headless_detach_preserves_execution_identity_and_terminal_state` | Proves same Runtime/PTY/TerminalState identity and state continuity; no durable restart. |
| UDS client detach, resync and reconnect | `pass8_resync_reattach::resync_and_detach_reattach_preserve_execution_block_identity_and_anchor` | Proves live Runtime reconnect/resync and immutable identity anchor; no persisted layout/history. |
| Concurrent client observations/fanout | Pass-5 Candidate-D `same_execution_fanout_is_consistent_at_4_8_and_16_viewers` and stalled-viewer recovery | Proves real PTY → VT → UDS/client projection behavior; not persistence-reader isolation. |
| Disconnect/failure cleanup | Pass-10 disconnect-during snapshot/input/resize cases | Proves live resource cleanup and reattach behavior; not crash-safe storage commit. |
| Real retained history source | existing `TerminalState::primary_history_range` tests | Proves current in-memory line identity/range semantics; no cold persistence, truncation/redaction or restart. |
| Large live execution population | `pass5_production_transport`/runtime scalability paths | Proves host-limited live PTY/runtime populations; not large retained-workspace restore or disk growth. |

## What cannot be established by current Runtime/PTY paths

The current Runtime has no production persistence worker/database, layout
store, schema migration, cold history store, redaction store, or Runtime-crash
keeper. Therefore these remain unavailable rather than inferred:

* crash during metadata/segment commit and subprocess restart recovery;
* schema migration/rollback and incompatible writer fencing on real storage;
* partial/corrupt metadata or segment recovery;
* physical redaction/truncation and WAL/backup/encryption remnants;
* Runtime-absent startup restore with explicit non-resurrection from persisted
  records;
* persistence queue saturation and measured background-write impact on real
  PTY/VT latency;
* large retained-workspace startup time/RSS/disk/write amplification.

The synthetic comparator remains useful only for these storage-shape questions.
It must not be promoted to real Runtime evidence.

## Reconciliation gate with #688

#687 owns canonical persistence semantics and Runtime-absent truth. #688 owns
signed/notarized executable delivery, staging, rollback and update UX. Before
either implementation becomes Ready, reconcile one compatibility matrix for:

* executable/protocol/schema versions and readable/writable metadata ranges;
* migration ownership when GUI and Runtime binaries differ;
* reconnect refusal versus live Runtime continuity during GUI update;
* rollback after a metadata migration has committed;
* SQLite/WAL/segment location, permissions, backup and cleanup under package
  replacement.

This plan intentionally freezes none of those #688 rules.
