# SPEC-010 — M002 Scrollback, Retained History, and Resize Reflow

- **Status:** Active; M002 production history budgets frozen by #818
- **Issue:** #685 architecture R&D; #818 production-budget refinement
- **Architecture:** `docs/architecture/ADR-010-SCROLLBACK-HISTORY-REFLOW.md`
- **Research:** `docs/architecture/SEYAL-SCROLLBACK-HISTORY-RD-001.md`
- **Dependency:** SPEC-011 / #816 supply the concrete canonical Unicode/grapheme/width text unit
- **Production calibration:** [`../evidence/m002-scrollback-production-calibration.md`](../evidence/m002-scrollback-production-calibration.md)

## 1. Purpose and scope

Define the observable M002 contract for primary-screen retained history, hard/soft line lineage, resize reflow, history/search/selection anchors, bounded eviction, alternate-screen exclusion, and the future cold-history persistence seam.

This specification extends SPEC-001 without changing the one-authoritative-`TerminalState` rule. Production history implementation (#819) consumes the SPEC-011 canonical text unit accepted by #816. #818 freezes the production resource/latency budgets that ADR-010 / #685 deliberately left open pending that footprint; those budgets are normative here and the linked evidence records their rationale.

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

`CanonicalTextUnit` is the SPEC-011 / #816 grapheme-or-compatible terminal text unit (UTF-8 payload or overflow sentinel, terminal width, style). Renderer shaping results are not stored as canonical history.

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

### 5.1 Frozen segment target and mutable-tail policy (#818)

| Parameter | Normative value |
| --- | --- |
| Immutable segment payload target | **16,384 bytes** encoded/uncompressed canonical payload |
| Absolute mutable-tail unsealed payload ceiling | **32,768 bytes** |

Segment sealing is targeted by encoded/uncompressed payload bytes, not a fixed number of lines. The 16,384-byte target is an implementation constant selected from #685 retention evidence scaled to the #816 `Cell`/style footprint (see calibration). It is **not** a public on-wire compatibility constant; changing it does not alter `LineId`, lineage or anchor identity.

Mutable-tail policy:

1. Append into the mutable tail.
2. When tail payload reaches **16,384 bytes** at a permitted seal boundary, seal an immutable segment and start a new empty tail.
3. Oversized logical chains/source records that would exceed the segment target **must** fragment at canonical text-unit boundaries into sealed segments that retain the original `LineId` plus unit offset/range.
4. Unsealed payload must never exceed **32,768 bytes**.
5. Detach, resource-pressure preparation and future persistence handoff may force-seal a non-empty tail earlier than the target, still only at unit/source boundaries.
6. Storage fragmentation must not allocate new terminal `LineId`s or change logical break semantics.

## 6. Resident memory bounds and eviction

### 6.1 Frozen hard caps (#818)

| Cap | Normative value |
| --- | --- |
| Per-execution resident retained history | **33,554,432 bytes (32 MiB)** |
| Runtime aggregate resident retained history | **268,435,456 bytes (256 MiB)** |

A user-visible line-count preference may additionally exist, but line count alone is not a safety bound and must not raise these hard byte caps.

Counted toward the caps: sealed segment payload + metadata, mutable tail payload + metadata, and history-owned UTF-8 for multi-scalar units retained in segments/tail.

Not counted: active primary/alternate screen grids, the live SPEC-011 `GraphemeStore` (2 MiB), derived reflow/search indexes (see §9), non-resident cold copies, renderer/client caches.

### 6.2 Eviction behavior

- oldest sealed content is evicted first;
- under Runtime aggregate pressure, order candidates by **global seal/append age across executions** (age-fair; no attached/detached or focus privilege);
- arbitrary middle-history eviction is forbidden;
- complete logical chains are preferred where compatible with the byte bound;
- a canonical text unit is never split;
- oversized chains may be trimmed only at explicit storage-fragment/source-unit boundaries;
- retained order and remaining anchors stay valid;
- an evicted anchor resolves to explicit `Evicted`/`Unavailable`, never another nearby line;
- if no durable cold copy exists, eviction permanently removes that payload from the available history surface;
- eviction must not fabricate live PTY/process continuity, rewrite process liveness/exit state, or alter canonical visible primary/alternate screen contents incorrectly.

## 7. Resize and reflow semantics

Changing terminal columns must not rewrite canonical retained history merely to change presentation width.

For a requested width, reflow:

1. reads source records in retained order;
2. joins across `SoftWrap` boundaries;
3. stops/restarts at `HardBreak`;
4. consumes canonical units using SPEC-011 / #816 terminal-width semantics;
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

Dropping all reflow caches and rebuilding from canonical history must reproduce the same visual layout for the same terminal width and accepted SPEC-011 width policy.

## 8. Active primary screen on resize

The active primary screen and retained-history suffix must preserve canonical text order and hard/soft lineage across resize.

Observable requirements:

- narrowing/widening changes visual row boundaries, not historical content identity;
- resize alone does not allocate replacement durable history IDs for existing content;
- cursor/content mapping must follow the accepted VT resize semantics for the active primary state;
- rows displaced into retained history by resize become canonical retained source content with their existing identities/lineage;
- existing retained source payload is not truncated merely because the new viewport is narrower.

This replaces M001's deliberately non-reflowing top-left rectangular-copy limitation for the M002 production path.

## 9. Reflow cache behavior and derived-index budgets

Reflow/layout caches are bounded and rebuildable.

A cache key must include enough identity to prevent stale reuse, including at least:

- terminal/history generation or equivalent immutable segment identity;
- requested column width;
- retained-range/eviction generation where applicable.

Cache invalidation may discard work; it must never discard canonical source payload.

### 9.1 Frozen derived budgets (#818)

| Budget | Normative value |
| --- | --- |
| Per-execution reflow/layout derived indexes | **4,194,304 bytes (4 MiB)** |
| Per-execution search acceleration indexes | **4,194,304 bytes (4 MiB)** |
| Runtime aggregate derived history indexes/caches | **33,554,432 bytes (32 MiB)** |

On derived-budget pressure, drop least-recently-used or oldest derived caches first. Derived eviction must not delete canonical retained payload. Allocation failure for derived caches may drop/rebuild indexes without corrupting terminal state.

## 10. Durable history anchors

The durable content primitive is:

```text
HistoryAnchor {
  line_id: LineId
  unit_offset: u32
}
```

Requirements:

- `line_id` remains an existing canonical source-row identity;
- `unit_offset` is a **`u32`** identifying content within the source/logical coordinate represented by that anchor mapping;
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
- search indexes, if present, are bounded/rebuildable under §9.1 and never become a copied transcript authority.

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

- allocation/resource pressure must produce deterministic bounded eviction under §6 or an explicit terminal/history error; unbounded growth is forbidden;
- persistence queue saturation cannot block PTY/VT progress; it must degrade according to an explicit bounded policy;
- malformed persisted metadata/payload is rejected before it can become canonical retained history;
- search/reflow cache allocation failure may drop/rebuild derived caches without corrupting terminal state.

## 16. Security and privacy

Retained history may contain sensitive shell/process output.

- history is scoped to its owning execution/workspace authority;
- no new telemetry/cloud/licensing dependency is introduced;
- cold persistence, when implemented, must follow local-user isolation, retention/deletion and integrity rules before being enabled;
- search/index/cache derivatives must not outlive the source retention/policy boundary without explicit authority.

## 17. Required tests and retained fixtures

Production implementation (#819) must add, at minimum, the fixtures frozen by #818:

| ID | Obligation |
| --- | --- |
| `hist-m001-corpus` | Replay retained M001 VT corpus through canonical history |
| `hist-hard-soft-lineage` | Explicit hard-break vs soft-wrap cases (CR alone is not a hard break) |
| `hist-long-soft-chain` | Long soft-wrapped logical chains crossing segment boundaries |
| `hist-wide-grapheme` | SPEC-011 wide/grapheme/combining/emoji/overflow-sentinel retained units |
| `hist-resize-oscillation` | Exact-width and width-oscillation reflow at 40/48/64/80/96/132/160 columns |
| `hist-alt-screen-exclusion` | Alternate-screen exclusion from primary history; non-contiguous `LineId` gaps |
| `hist-selection-anchor-stable` | Selection anchors surviving resize/reflow |
| `hist-search-soft-span` | Search matches spanning soft wraps and remaining width-independent |
| `hist-evict-oldest` | Deterministic oldest-first eviction and explicit evicted-anchor resolution |
| `hist-oversize-fragment` | Oversized logical-chain storage fragmentation without new IDs |
| `hist-cache-rebuild-equiv` | Cache-drop/rebuild equivalence property tests |
| `hist-aggregate-fairness` | Multi-execution aggregate eviction fairness without fabricating PTY continuity |

Additionally required: fuzz/property coverage for extreme dimensions, metadata ranges and repeated resize/eviction.

## 18. Performance/resource acceptance

### 18.1 Frozen physical-host reflow gates (#818)

On controlled ARM64 macOS Release builds, exact-head evidence for #819 must satisfy:

| Workload | p50 | p95 | p99 |
| --- | ---: | ---: | ---: |
| Active/near-visible window reflow (viewport + ≤2 screenfuls slack) | ≤ **2 ms** | ≤ **4 ms** | ≤ **8 ms** |
| Lazy reflow of one sealed segment at the 16 KiB payload target | ≤ **1 ms** | ≤ **2 ms** | ≤ **4 ms** |

CI-host timings for the same harness shapes are comparative only and must not alone pass or fail these physical-host gates. Linux #685 spike numbers remain comparative R&D evidence, not release thresholds.

### 18.2 Required measurement matrix

#819 must record measured production values on the supported macOS representation for:

- segment payload target conformance (16,384-byte seal behavior);
- per-execution and Runtime aggregate resident-history caps;
- append throughput/latency;
- reflow p50/p95/p99 for active window and per-segment retained ranges;
- search and anchor-resolution cost;
- allocation churn;
- RSS at **10k / 100k / 1M** retained-content scales;
- **1 / 10 / 50 / 100** execution population RSS/CPU/resource behavior;
- detached/hidden idle behavior.

#673 remains the authority for versioned release-level ceilings and may further tighten (never silently weaken) these implementation gates.

## 19. Compatibility

- ADR-004 `LineId` lifetime/non-reuse guarantees remain valid;
- existing APIs that numerically iterate IDs or equate one visual row with one durable source identity must be refined rather than preserving an incorrect geometry assumption;
- projection/rendering protocols may transport derived visual rows, but clients gain no scrollback mutation authority;
- persistence format/versioning is deferred and must not expose raw in-memory Rust layout as a durable compatibility contract.

## 20. Explicit non-goals

This specification does not choose:

- an on-disk database/file format;
- a compression codec;
- remote history paging protocol;
- renderer shaping/cache representation;
- new Block ownership semantics;
- alternate-screen transcript capture.

The SPEC-011 canonical grapheme/width/text-unit representation and the #818 numeric history budgets are now frozen authority for #819. Those decisions may be revised only by focused specification review with evidence; they cannot violate this contract's one-authority, source-anchor, bounded-memory or asynchronous-persistence invariants.

## 21. Implementation readiness after #818

ADR-010 / #685 architecture remains unchanged. #818 closes the production-value gaps that previously blocked HistoryStore implementation:

- numeric segment target and mutable-tail policy;
- numeric per-execution and Runtime aggregate resident caps with age-fair eviction;
- numeric derived-index budgets;
- `u32` anchor offsets and selection/search behavior notes;
- physical-host vs CI reflow gates and the 10k/100k/1M × 1/10/50/100 benchmark matrix;
- exact retained fixture IDs including hard/soft wrap, wide/grapheme, alt-screen and resize oscillation.

After #818 is reviewed and merged, #819 may pass its separate exact-head development-readiness gate. This refinement does not implement `HistoryStore`.
