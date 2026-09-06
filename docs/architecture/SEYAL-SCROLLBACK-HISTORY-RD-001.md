# Seyal Scrollback / History R&D 001

- **Date:** 2026-09-06
- **Issue:** #685
- **Parent:** #664
- **Purpose:** evidence for the permanent M002 scrollback, retained-history and resize-reflow architecture
- **Prototype PR:** #803 (`spike/685-m002-scrollback-rd`) — isolated, non-mergeable, close rather than merge
- **Final evidence run:** GitHub Actions `34012058371`
- **Final artifact digest:** `sha256:2157590d29c48d8f3830e4e29c29bf49ef4ffed2af7b19f0c72639e7f4a8d8db`

## Question

Choose a retained-history representation and reflow model that extends the one authoritative `TerminalState` without introducing a second terminal/history authority, remains bounded for many detached executions, preserves durable content anchors through visual reflow, and leaves a clean asynchronous seam for later cold persistence.

This R&D does **not** choose Seyal's final grapheme/width payload. Issue #684 owns that question. Synthetic experiments use a compact atom carrying scalar/style/width/combining metadata so representation shape can be compared without pre-empting #684.

## Existing authority and current limitation

Existing architecture constrains the answer:

- `TerminalState` remains the single canonical terminal semantic owner (ADR-004).
- `LineId` is terminal-lifetime, monotonic and non-reused; viewport row number is not a durable history/Block anchor (ADR-004 / SPEC-001).
- completed scrollback is cold/evictable and must not stay hot merely because an execution is detached (ADR-007).
- persistence, indexing, agents, Blocks and presentation may not synchronously gate `PTY -> VT/parser -> TerminalState -> damage` (ADR-007).

M001 intentionally keeps a temporary primary history: a fixed 8,192-entry `VecDeque<(LineId, Vec<Cell>)>` of visual rows. M001 resize is rectangular/non-reflowing and the current history projection does not expose hard-break versus soft-autowrap lineage. That representation therefore cannot be the permanent M002 reflow authority.

## Candidates exercised

### A. Visual-row snapshots

A bounded deque of width-derived visual fragments. This approximates extending M001's visual-row history.

### B. Per-logical-line ring

A bounded deque with one variable-length allocation per logical source line. This is the simple logical-history reference baseline.

### C. Compact immutable segmented source history + mutable tail

A mutable append tail seals into immutable segments containing compact source-line metadata and contiguous payload. Segment metadata preserves source `LineId` ranges/offsets for lookup and later persistence.

The first candidate used a fixed 64 logical lines per segment only to expose allocation/index behavior. Calibration proved that fixed line count is the wrong permanent boundary, so ADR-010 selects byte-targeted segments instead.

## Real retained-VT fixture replay

The final run replayed Seyal's existing retained M001 VT corpus through the real `seyal_terminal::TerminalState`, then reconstructed already-retained primary rows through ring and compact segmented candidate storage.

Corpus:

- `m001-basic`
- `m001-deferred-osc`
- `m001-ecma48-core`
- `m001-ecma48-erase-save`
- `m001-xterm-private`
- `m001-utf8`

Across 128 replay rounds:

- retained primary rows: `639`;
- retained cells: `5,112`;
- styled retained cells: `4,328`;
- non-ASCII retained cells: `127`;
- non-contiguous retained `LineId` gaps: `127`;
- 512-cell-target compact segments: `10`;
- ring reconstruction exactly matched canonical retained `(LineId, Cell[])` rows;
- segmented reconstruction exactly matched canonical retained `(LineId, Cell[])` rows;
- pre-existing retained payload survived resize oscillation unchanged;
- alternate-screen content remained excluded from primary history.

The replay exposed the key M002 schema requirement: M001 retained rows do **not** expose whether adjacent rows are connected by soft autowrap. Production M002 must record `HardBreak` versus `SoftWrap` in canonical terminal state at mutation time; reconstructing it later from row text or geometry is not correct.

## Synthetic correctness/stress coverage

The isolated representation harness additionally exercised behavior not yet expressible by M001's scalar/visual-row history API:

- one logical chain spanning eight 80-column rows;
- wide-cell and combining/grapheme-side metadata;
- non-contiguous `LineId`s simulating alternate-screen allocator consumption;
- repeated 40/48/64/80/96/132/160-column reflow oscillation;
- search across retained logical content;
- selection anchors as `(LineId, canonical-unit offset)` surviving reflow;
- bounded eviction;
- 10k/100k/1M retained-unit scales;
- 1/10/50/100 detached/hidden execution populations;
- raw serialization plus a simple space-RLE cold-compression seam.

Synthetic wide/grapheme metadata is representation stress only; #684 remains semantic authority for real Unicode/width behavior.

## Representation latency/allocation evidence

Runner: Linux x86_64 GitHub-hosted VM, Rust `1.98.0`. Numbers are comparative R&D evidence, not production release thresholds.

At approximately 1M retained synthetic atoms:

| candidate | retained atoms | append ms | representation allocation units | reflow p50/p95/p99 us | search p50/p95 us | selection ns/op |
|---|---:|---:|---:|---:|---:|---:|
| visual rows | 999,940 | 19.40 | 16,636 | 1543 / 1643 / 1817 | 977 / 996 | 3,084 |
| logical ring | 999,940 | 8.57 | 8,466 | 1360 / 1446 / 1448 | 850 / 917 | 1,724 |
| fixed-64 segmented prototype | 993,877 | 9.27 | 268 | 1321 / 1412 / 1417 | 798 / 823 | 78 |

Interpretation:

- visual-row snapshots have clearly worse append/allocation behavior and make layout geometry canonical;
- the logical ring is a strong simple baseline;
- compact segmentation preserves ring-like sequential performance while reducing representation object/allocation count dramatically and providing a natural immutable indexing/persistence unit;
- the fixed-64 result is **not** accepted as the segment policy because its small-history eviction granularity is incorrect.

## Independent actual-RSS calibration

A second harness measures each candidate in a fresh child process so allocator reuse from one candidate cannot contaminate another. The segmented candidate seals builder capacity into exact boxed immutable slices before RSS is read, matching the selected compact-segment architecture.

Baseline process VmRSS was `2,096 KiB`; table values below are baseline-subtracted deltas.

### Single execution

| candidate | retained units | 10k delta KiB | 100k delta KiB | 1M delta KiB | storage objects at 1M |
|---|---:|---:|---:|---:|---:|
| visual rows | 10k / 100k / 1M | 196 | 1,364 | 13,072 | 25,143 |
| logical ring | 10k / 100k / 1M | 132 | 1,336 | 12,780 | 22,349 |
| compact segmented | 10k / 100k / 1M | 208 | 1,316 | 12,388 | 2,235 |

At 1M retained units, compact segmentation used about `3.1%` less measured RSS than the ring and about `5.2%` less than visual rows in this isolated model, while reducing storage-object count by roughly `10x` versus the ring and `11x` versus visual rows.

The 10k result is too close to process/allocator granularity to treat as a memory winner; segment-size/retention calibration is more important at that scale.

### 1/10/50/100 execution population

Each execution retained 100,000 canonical synthetic units.

| candidate | 1 exec delta KiB | 10 exec delta KiB | 50 exec delta KiB | 100 exec delta KiB | 100-exec storage objects |
|---|---:|---:|---:|---:|---:|
| visual rows | 1,360 | 13,128 | 65,428 | 130,792 | 251,600 |
| logical ring | 1,340 | 12,784 | 63,816 | 127,556 | 223,600 |
| compact segmented | 1,312 | 12,440 | 61,752 | 123,464 | 22,400 |

At 100 executions, compact segmentation measured about `3.2%` lower RSS than the ring and `5.6%` lower than visual rows while retaining an order-of-magnitude fewer storage objects.

### RSS measurement correction

An intermediate RSS-only calibration mistakenly kept each segment as a live `Vec` with full 512-unit reserved capacity after sealing. That measured builder slack, not the selected immutable compact representation, and made segmented RSS appear roughly 15% worse. It was rejected as a harness-model defect. The final run seals payload/metadata into exact boxed slices before measuring and is the authoritative RSS calibration for this R&D.

This correction is why #685 does not treat a benchmark harness as authority merely because it produced a number.

## Segment-granularity calibration

A fixed number of logical lines is unsuitable because line lengths are workload-dependent and unbounded. Calibration instead sealed at source/logical boundaries using a target synthetic payload size as a proxy for future encoded bytes:

| target units/segment | history budget | retained | retention | sealed segments |
|---:|---:|---:|---:|---:|
| 512 | 10,000 | 9,880 | 98.80% | 23 |
| 1,024 | 10,000 | 9,545 | 95.45% | 11 |
| 2,048 | 10,000 | 8,552 | 85.52% | 6 |
| 4,096 | 10,000 | 6,490 | 64.90% | 3 |
| 512 | 100,000 | 99,525 | 99.52% | 220 |
| 1,024 | 100,000 | 99,666 | 99.67% | 106 |
| 512 | 1,000,000 | 999,940 | 99.99% | 2,181 |
| 1,024 | 1,000,000 | 999,605 | 99.96% | 1,047 |

For 100 executions with a 10k-unit history budget, a 512-target prototype retained 98.8% of requested content; larger targets increasingly traded away minimum-budget retention precision.

The architectural conclusion is **byte-targeted compact segmentation**, not “512 atoms.” The production byte target is a measured implementation constant selected after #684 fixes the real canonical payload and macOS benchmarks are available.

## LineId / anchor compatibility experiment

Reflow must not silently redefine ADR-004's existing `LineId` contract. The schema experiment therefore kept each retained source row's current `LineId`, grouped soft-connected source records into a logical chain, and mapped canonical-unit offsets through the group.

A chain captured at width 80 and reflowed at 40/120 preserved its original constituent row IDs. An anchor to the second constituent `LineId` remained resolvable after both reflows.

Decision consequence:

- no new competing logical-line ID allocator;
- retained logical chains own ordered source records carrying existing `LineId`s;
- durable content coordinates are `(ExecutionId, LineId, canonical-unit offset)`/ranges;
- visual rows are derived source spans and may contain more than one source `LineId`.

## Persistence/compression evidence

The representation harness serialized the same canonical synthetic payload from all candidates. Simple space-RLE produced roughly `0.84` of raw size in that synthetic workload and demonstrated that immutable ranges can be serialized/compressed independently.

This is **not** evidence to standardize RLE. It only validates the seam:

- seal immutable segment in terminal state;
- hand off bounded immutable data asynchronously;
- compression/write/fsync happen outside PTY mutation;
- persistence stores backing data for the same canonical segment rather than reparsing terminal output;
- cold history does not claim to restore a live PTY.

Codec, durable format, corruption recovery and paging remain later measured persistence decisions.

## Decision evidence

### Reject visual-row history as canonical retained history

- wrapping is presentation geometry, not content authority;
- current M001 projection lacks hard/soft lineage required for correct reflow;
- substantially more representation objects/allocations in stress measurements;
- resize/search/selection become simpler and stable when visual rows are derived source spans.

### Keep per-logical-line ring as a reference baseline, not the permanent shape

The ring is simple and performs well, but one allocation/object per source line provides no natural immutable range for coarse lookup, bounded paging/persistence or batch eviction. Final RSS also shows no memory advantage over compact exact-sealed segments at 100k/1M scales.

### Select canonical source history in compact byte-targeted immutable segments plus a mutable tail

This gives:

- one history authority inside `TerminalState`;
- hard/soft lineage recorded once at terminal mutation time;
- no full-history rewrite on resize;
- contiguous immutable payload storage;
- order-of-magnitude fewer representation objects in the measured model;
- stable source anchors across reflow;
- natural immutable units for asynchronous cold persistence/paging;
- explicit byte/resource bounds for detached populations.

Oversized logical chains may cross storage segments at canonical text-unit boundaries. Storage fragmentation never creates a new terminal identity or hard line break.

## Production gates after #685

Merging the #685 architecture/specification does **not** make production scrollback implementation automatically Ready.

Before implementation begins:

1. #684 must accept the canonical Unicode/grapheme/width text-unit authority, or an explicitly accepted compatible implementation slice must exist;
2. the implementation Issue must select measured segment byte target and per-execution/Runtime aggregate history limits using the real payload;
3. macOS production-path benchmarks must establish append/reflow/search/selection p50/p95/p99 and 1/10/50/100 execution RSS/resource gates;
4. tests must cover long wraps, hard/soft lineage, width/grapheme semantics, resize oscillation, alternate screen, source anchors, eviction, cache rebuild equivalence and fuzz/property cases;
5. persistence remains separate work unless explicitly in scope and may never gate terminal I/O.

## Conclusion

The #685 evidence supports **canonical retained source history + mutable tail + compact byte-targeted immutable segments + derived bounded reflow/search indexes**.

It rejects canonical visual-row scrollback, rejects fixed line-count segmentation, preserves existing `LineId` compatibility through source anchors, and keeps cold persistence/compression asynchronous and subordinate to the one `TerminalState` authority.
