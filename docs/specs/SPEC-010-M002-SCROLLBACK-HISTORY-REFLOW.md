# SPEC-010 — M002 Scrollback, Retained History, and Resize Reflow

- **Status:** Active
- **Issue:** #685
- **Architecture:** `docs/architecture/ADR-010-SCROLLBACK-HISTORY-REFLOW.md`
- **Research:** `docs/architecture/SEYAL-SCROLLBACK-HISTORY-RD-001.md`
- **Dependency:** #684 owns the concrete canonical Unicode/grapheme/width text unit

## 1. Purpose and scope

Define the observable M002 contract for primary-screen retained history, hard/soft line lineage, resize reflow, history/search/selection anchors, bounded eviction, alternate-screen exclusion, and the future cold-history persistence seam.

This specification extends SPEC-001 without changing the one-authoritative-`TerminalState` rule. It does not select the concrete `CanonicalTextUnit`; production implementation consumes the representation accepted by #684.

## 2. Authority and ownership

For each `ExecutionId`:

- exactly one `TerminalState` owns canonical primary terminal state and retained primary history;
- history is not reparsed from output by a client, renderer, Block, search service, persistence service, or agent;
- display rows, reflow indexes, search indexes, persistence manifests and Block projections are derived/rebuildable views or backing representations only;
- PTY -> parser -> `TerminalState` mutation/history append -> damage never synchronously waits for persistence, compression, indexing, rendering, agents, cloud, telemetry or licensing.

Alternate-screen state remains owned by the same `TerminalState` but is not primary scrollback.

## 3. Canonical retained source model

Retained primary history preserves ordered source content, not width-specific visual rows.

Conceptually each retained source record contains:

```text
SourceRecord {
  LineId
  ordered CanonicalTextUnit payload
  boundary_to_next: HardBreak | SoftWrap
}
```

Requirements:

- existing ADR-004 `LineId` semantics remain unchanged: terminal-lifetime, non-reused and allocated only by the terminal authority;
- no second logical-line ID allocator is introduced merely for reflow;
- adjacent records connected by `SoftWrap` form one reflowable logical chain;
- `HardBreak` terminates that chain;
- `LineId`s may be non-contiguous because alternate-screen and other canonical row creation consume the same allocator;
- history order must never be inferred from numeric `LineId + 1` adjacency.

## 4. Hard-break and soft-wrap lineage

Lineage is recorded at canonical terminal mutation time and travels with the affected source rows/content.

For the currently accepted VT subset:

- autowrap/pending-wrap continuation creates `SoftWrap` lineage;
- explicit LF/VT/FF line termination creates `HardBreak` lineage unless a later accepted VT specification defines a more specific region/editing behavior;
- CR alone does not create a hard break;
- resize never invents hard/soft lineage;
- erase, cursor motion, style changes and rendering geometry must not be used to infer lineage after the fact.

Future editing/scroll-region sequences must define how they preserve, split, merge or invalidate lineage before they can be marked supported.

## 5. Retained storage and segmentation

Canonical retained payload is stored as a mutable append tail plus sealed immutable segments.

A sealed segment must carry enough metadata to:

- enumerate source records in terminal order;
- map each retained `LineId` to its canonical-unit range;
- preserve hard/soft lineage;
- validate payload offsets and lengths;
- identify first/last retained source ranges for coarse lookup;
- rebuild all derived reflow/search indexes without another source of truth.

Segment sealing is targeted by encoded/uncompressed payload bytes, not a fixed number of lines. The exact target is an implementation tuning value selected from measured production payloads and is not a public compatibility constant.

A single oversized line/logical chain may span multiple storage fragments at canonical text-unit boundaries. Storage fragmentation must not allocate new terminal `LineId`s or change logical break semantics.

## 6. Resident memory bounds and eviction

Production M002 must have:

- a hard per-execution resident-history byte bound; and
- a hard Runtime aggregate resident-history bound/policy across live and detached executions.

A user-visible line-count preference may additionally exist, but line count alone is not a safety bound.

Eviction behavior:

- oldest sealed content is evicted first;
- arbitrary middle-history eviction is forbidden;
- complete logical chains are preferred where compatible with the byte bound;
- a canonical text unit is never split;
- oversized chains may be trimmed only at explicit storage-fragment/source-unit boundaries;
- retained order and remaining anchors stay valid;
- an evicted anchor resolves to explicit `Evicted/Unavailable`, never another nearby line;
- if no durable cold copy exists, eviction permanently removes that payload from the available history surface.

Exact default byte values are intentionally not frozen by #685 because #684 has not yet fixed the production text-unit representation. The implementation Issue must establish measured defaults before it becomes Ready.

## 7. Resize and reflow semantics

Changing terminal columns must not rewrite canonical retained history merely to change presentation width.

For a requested width, reflow:

1. reads source records in retained order;
2. joins across `SoftWrap` boundaries;
3. stops/restarts at `HardBreak`;
4. consumes canonical units using #684 terminal-width semantics;
5. emits width-specific visual rows as source-span ranges;
6. never mutates retained payload, `LineId`s or break lineage;
7. may cache only derived row-boundary/index information.

A width-specific derived row is conceptually:

```text
VisualRow {
  source_spans: [(LineId, unit_range)]
}
```

A visual row may therefore span more than one source record in the same soft-wrapped chain.

The active/near-visible window may be materialized eagerly. Older retained segments may be reflowed lazily. A resize must not require work proportional to all retained history before the active surface can progress.

Dropping all reflow caches and rebuilding from canonical history must reproduce the same visual layout for the same terminal width and accepted #684 width policy.

## 8. Active primary screen on resize

The active primary screen and retained-history suffix must preserve canonical text order and hard/soft lineage across resize.

Observable requirements:

- narrowing/widening changes visual row boundaries, not historical content identity;
- resize alone does not allocate replacement durable history IDs for existing content;
- cursor/content mapping must follow the accepted VT resize semantics for the active primary state;
- rows displaced into retained history by resize become canonical retained source content with their existing identities/lineage;
- existing retained source payload is not truncated merely because the new viewport is narrower.

This replaces M001's deliberately non-reflowing top-left rectangular-copy limitation for the M002 production path.

## 9. Reflow cache behavior

Reflow/layout caches are bounded and rebuildable.

A cache key must include enough identity to prevent stale reuse, including at least:

- terminal/history generation or equivalent immutable segment identity;
- requested column width;
- retained-range/eviction generation where applicable.

Cache invalidation may discard work; it must never discard canonical source payload.

## 10. Durable history anchors

The durable content primitive is conceptually:

```text
HistoryAnchor {
  line_id: LineId
  unit_offset: integer
}
```

Requirements:

- `line_id` remains an existing canonical source-row identity;
- `unit_offset` identifies content within the source/logical coordinate represented by that anchor mapping;
- source metadata maps constituent `LineId`s to their logical-chain offsets so anchors survive reflow;
- viewport row/column is never the persisted history identity;
- resize/reflow leaves anchors unchanged;
- an anchor that has been evicted returns explicit unavailable state;
- integer overflow or malformed offsets fail explicitly rather than wrap/clamp to unrelated content.

Block anchors may continue to use existing `LineId` identity and evolve to an offset/range only where Block semantics require sub-line precision. ADR-010 does not create Block-owned output copies.

## 11. Selection semantics

Selection start/end are source anchors, plus affinity/direction metadata where required by presentation behavior.

- hit testing maps current visual source spans back to canonical anchors;
- reflow remaps anchors to new visual rows without changing the selected source content;
- selection may cross soft-wrap boundaries;
- hard breaks remain logical separators/newline semantics;
- eviction of either endpoint is surfaced explicitly according to the selection API; endpoints are not silently moved.

## 12. Search semantics

Search operates over canonical ordered source content, not current visual rows.

Therefore:

- changing terminal width does not change matches;
- a match may cross a `SoftWrap` source-record boundary;
- `HardBreak` acts as the logical line separator defined by the search contract;
- results are source-anchor ranges and are mapped to current visual rows only for presentation;
- search indexes, if present, are bounded/rebuildable and never become a copied transcript authority.

## 13. Alternate screen

- alternate-screen content is never appended to primary retained history by default;
- entering alternate screen preserves primary retained history;
- leaving alternate screen reveals the primary state/history subject only to normal independent resource-policy eviction;
- alternate-screen `LineId` allocation may create gaps in primary-history IDs;
- search/reflow/history code must not assume contiguous IDs;
- TUI/full-screen resize remains alternate-screen behavior and does not manufacture primary scrollback snapshots.

## 14. Persistence boundary

A sealed immutable segment is the future P3 cold-history persistence handoff unit.

Persistence requirements when implemented:

- handoff is asynchronous and bounded;
- terminal progress never waits for write/fsync/compression/replication;
- persisted payload represents the same immutable canonical segment, not reparsed terminal output;
- manifests preserve ordering, source-range metadata, version and integrity information;
- rehydration validates segment identity/ranges/integrity before exposure;
- corruption or missing cold data is reported as unavailable/corrupt, never replaced with fabricated history;
- historical persistence does not claim to restore a live PTY after Runtime/process loss.

Compression is optional cold-storage policy and not terminal semantics.

## 15. Failure and backpressure behavior

- allocation/resource pressure must produce deterministic bounded eviction or an explicit terminal/history error according to the implementation specification; unbounded growth is forbidden;
- persistence queue saturation cannot block PTY/VT progress; it must degrade according to an explicit bounded policy;
- malformed persisted metadata/payload is rejected before it can become canonical retained history;
- search/reflow cache allocation failure may drop/rebuild derived caches without corrupting terminal state.

## 16. Security and privacy

Retained history may contain sensitive shell/process output.

- history is scoped to its owning execution/workspace authority;
- no new telemetry/cloud/licensing dependency is introduced;
- cold persistence, when implemented, must follow local-user isolation, retention/deletion and integrity rules before being enabled;
- search/index/cache derivatives must not outlive the source retention/policy boundary without explicit authority.

## 17. Required tests

Production implementation must add, at minimum:

- retained M001 VT fixture replay;
- long soft-wrapped logical chains crossing segment boundaries;
- explicit hard-break vs soft-wrap cases;
- exact-width and width-oscillation reflow;
- #684 wide/grapheme/combining/emoji cases once accepted;
- non-contiguous `LineId` cases caused by alternate-screen churn;
- alternate-screen exclusion from primary history;
- selection anchors surviving resize/reflow;
- search matches spanning soft wraps and remaining width-independent;
- deterministic oldest-first eviction and explicit evicted-anchor resolution;
- oversized logical-chain storage fragmentation without new IDs;
- cache-drop/rebuild equivalence property tests;
- fuzz/property coverage for extreme dimensions, metadata ranges and repeated resize/eviction.

## 18. Performance/resource acceptance

Before M002 production history/reflow implementation is Ready, its owning Issue must record measured production values for:

- segment payload target;
- per-execution resident-history cap;
- Runtime aggregate resident-history cap/policy;
- append throughput/latency;
- reflow p50/p95/p99 for active window and retained ranges;
- search and anchor-resolution cost;
- allocation churn;
- RSS at 10k/100k/1M retained-content scales;
- 1/10/50/100 execution population RSS/CPU/resource behavior;
- detached/hidden idle behavior.

Measurements must be repeated on the supported macOS production representation after #684. Linux spike numbers are comparative R&D evidence, not release thresholds.

## 19. Compatibility

- ADR-004 `LineId` lifetime/non-reuse guarantees remain valid;
- existing APIs that numerically iterate IDs or equate one visual row with one durable source identity must be refined rather than preserving an incorrect geometry assumption;
- projection/rendering protocols may transport derived visual rows, but clients gain no scrollback mutation authority;
- persistence format/versioning is deferred and must not expose raw in-memory Rust layout as a durable compatibility contract.

## 20. Explicit non-goals

This specification does not choose:

- the #684 canonical grapheme/width/text-unit representation;
- exact default history byte limits;
- an on-disk database/file format;
- a compression codec;
- remote history paging protocol;
- renderer shaping/cache representation;
- new Block ownership semantics;
- alternate-screen transcript capture.

Those decisions may extend this contract but cannot violate its one-authority, source-anchor, bounded-memory or asynchronous-persistence invariants.
