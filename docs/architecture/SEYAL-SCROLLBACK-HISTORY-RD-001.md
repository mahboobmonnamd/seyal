# Seyal Scrollback / History R&D 001

- **Date:** 2026-09-06
- **Issue:** #685
- **Parent:** #664
- **Purpose:** evidence for the permanent M002 scrollback, retained-history and resize-reflow architecture
- **Prototype PR:** #803 (`spike/685-m002-scrollback-rd`) — isolated, non-mergeable, must be closed rather than merged
- **Evidence run:** GitHub Actions run `34000268655`, artifact digest `sha256:7bc9eed7672e3248da3a9a651fbcd9711aee192acdcb403c1776553fc5ee7a51`

## Question

Choose a retained-history representation and reflow model that extends the one authoritative `TerminalState` without introducing a second terminal/history authority, remains bounded for many detached executions, preserves durable content anchors through visual reflow, and leaves a clean asynchronous seam for later cold persistence.

This R&D does **not** choose Seyal's final grapheme/width payload. Issue #684 owns that question. The experiments use a compact synthetic atom containing scalar/style/width/combining metadata only so representation shape and scaling can be compared without pre-empting #684.

## Existing authority and current limitation

Existing architecture constrains the solution:

- `TerminalState` remains the single canonical terminal semantic owner (ADR-004).
- `LineId` is terminal-lifetime, monotonic and non-reused; viewport row number is not a durable history/Block anchor (ADR-004 / SPEC-001).
- completed scrollback is cold/evictable and must not stay hot merely because an execution is detached (ADR-007).
- persistence, indexing, agents, Blocks and presentation may not synchronously gate `PTY -> VT/parser -> TerminalState -> damage` (ADR-007).

The current M001 seam is intentionally temporary: primary history is a fixed 8,192-entry `VecDeque<(LineId, Vec<Cell>)>` of visual rows, and resize preserves/truncates the existing rectangular rows rather than performing production logical reflow.

## Candidates exercised

### A. Visual-row snapshots

A bounded deque of 80-column visual fragments. Logical identity is repeated across fragments and eviction removes all fragments belonging to the oldest retained logical line.

This approximates extending today's visual-row-oriented history representation.

### B. Per-logical-line ring

A bounded `VecDeque` with one variable-length allocation per logical line.

This is the simplest correct logical-history baseline.

### C. Immutable segmented logical history + mutable tail

A mutable append tail is periodically sealed into immutable segments containing compact line metadata plus a contiguous atom arena. Segments carry `LineId` ranges for indexing and are evicted as units.

The first experiment used 64 logical lines per segment to expose the allocation/indexing behavior. A second calibration replaced fixed line count with a target segment payload because terminal line lengths are workload-dependent and potentially very uneven.

## Correctness fixtures

The isolated harness exercised:

- logical lines spanning eight 80-column rows;
- wide-cell metadata and combining/grapheme-side metadata;
- non-contiguous `LineId`s simulating IDs consumed by alternate-screen lifetimes;
- repeated 40/48/64/80/96/132/160-column resize/reflow oscillation;
- search over retained logical content;
- selection anchors represented as `(LineId, atom offset)` across reflow;
- bounded whole-logical-line / whole-segment eviction;
- 10k, 100k and 1M retained-atom workloads;
- 1, 10, 50 and 100 hidden/detached execution populations;
- raw serialization plus a deliberately simple space-RLE persistence/compression seam.

The prototype is synthetic evidence, not conformance proof. Production implementation must repeat correctness against repository VT fixtures and the accepted #684 text-unit/width model.

## Core measurements

Runner: Linux x86_64 GitHub-hosted VM, Rust `1.98.0`. RSS is process delta from a fresh child per candidate/size. Absolute numbers are allocator/runner dependent; relative slopes and representation behavior are the useful evidence.

| candidate | target atoms | retained atoms | RSS KiB | append ms | representation allocation units | reflow p95 us | search p95 us | selection ns/op |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| visual rows | 10,000 | 9,880 | 316 | 0.24 | 175 | 16.06 | 7.62 | 32 |
| logical ring | 10,000 | 9,880 | 196 | 0.11 | 91 | 14.32 | 6.85 | 21 |
| segmented 64-line | 10,000 | 3,817 | 228 | 0.13 | 6 | 5.41 | 2.73 | 10 |
| visual rows | 100,000 | 99,936 | 2,244 | 2.29 | 1,683 | 175.46 | 93.00 | 322 |
| logical ring | 100,000 | 99,936 | 1,288 | 1.05 | 856 | 153.12 | 68.82 | 157 |
| segmented 64-line | 100,000 | 93,938 | 1,340 | 1.11 | 30 | 137.20 | 64.91 | 24 |
| visual rows | 1,000,000 | 999,940 | 20,196 | 22.12 | 16,636 | 2,818.35 | 1,769.21 | 3,892 |
| logical ring | 1,000,000 | 999,940 | 12,196 | 10.53 | 8,466 | 1,484.51 | 720.70 | 1,441 |
| segmented 64-line | 1,000,000 | 993,877 | 12,444 | 11.54 | 268 | 1,457.40 | 740.81 | 71 |

At the 1M scale, visual-row snapshots used about **66% more RSS** than the logical-line ring (`20,196` vs `12,196` KiB), took about **2.1x** the append time, and had materially worse reflow/search latency.

The logical ring and segmented representation had similar payload-level RSS and append/reflow/search behavior at large scale, but segmentation reduced representation allocation units from `8,466` to `268` in the 1M case and made `LineId`-range anchor lookup substantially cheaper. This is the main reason to prefer segmentation over a per-line heap/deque as the permanent shape.

### Hidden/detached population slope

Each execution retained approximately 10k atoms.

| candidate | executions | retained atoms | RSS KiB | build ms |
|---|---:|---:|---:|---:|
| visual rows | 100 | 988,000 | 20,476 | 22.90 |
| logical ring | 100 | 988,000 | 12,360 | 11.00 |
| segmented 64-line | 100 | 381,700 | 6,720 | 9.16 |

The 64-line segmented row is **not** a like-for-like memory win at 10k per execution because it retained only 38.17% of the requested payload. That result exposed an eviction-granularity defect and must not be cited as proof that 64-line chunks are better for detached populations.

## Segment-granularity calibration

Fixed logical-line counts are unsuitable as the permanent segment boundary. A later calibration sealed at logical-line boundaries using a target payload size (synthetic atoms as a proxy for bytes):

| target atoms per segment | history budget | retained | retention | sealed segments |
|---:|---:|---:|---:|---:|
| 512 | 10,000 | 9,880 | 98.80% | 23 |
| 1,024 | 10,000 | 9,545 | 95.45% | 11 |
| 2,048 | 10,000 | 8,552 | 85.52% | 6 |
| 4,096 | 10,000 | 6,490 | 64.90% | 3 |
| 512 | 100,000 | 99,525 | 99.52% | 220 |
| 1,024 | 100,000 | 99,666 | 99.67% | 106 |
| 512 | 1,000,000 | 999,940 | 99.99% | 2,181 |
| 1,024 | 1,000,000 | 999,605 | 99.96% | 1,047 |

For 100 executions at a 10k-atom history budget, 512-target segments retained 98.80% of the requested content and used 14,404 KiB RSS; 1,024-target segments retained 95.45% and used 12,652 KiB. This exposes the expected trade-off between eviction precision and segment/allocation overhead.

The architecture therefore must specify a **byte-targeted bounded segment**, not a fixed number of lines and not the synthetic atom counts above. The exact production byte target remains an internal tuning constant selected against the final #684 payload and macOS benchmarks. It is not a user-visible compatibility contract.

## Decision evidence

### Reject visual-row history as canonical retained history

Reasons:

- visual wrapping is layout, not content authority;
- substantially worse memory and append scaling in the experiment;
- resize requires rewriting/reconstructing row snapshots instead of deriving a new layout;
- search/selection must reason across artificial row boundaries;
- it encourages viewport coordinates to leak into durable history/Block semantics.

### Keep a per-logical-line ring only as a reference baseline

It is simple and performed well, but one allocation per logical line creates avoidable allocation/indexing overhead, and it gives no natural immutable unit for cold persistence, compression or coarse eviction.

### Select logical retained content in size-targeted immutable segments plus a mutable tail

This gives:

- one canonical logical content stream inside `TerminalState`;
- no full-history rewrite on resize;
- compact contiguous payload storage;
- dramatically fewer representation allocations than per-line heap storage;
- cheap segment-level `LineId` range lookup;
- natural immutable units for asynchronous cold persistence/compression;
- bounded eviction without making a second history authority.

Segments must be allowed to split storage payload at canonical text-unit boundaries when a single logical line would otherwise violate the hard segment/memory bound. Storage fragmentation never creates a new terminal logical identity; the segment metadata carries the owning `LineId` and logical offset.

## Reflow direction

Reflow is a **derived layout operation** over canonical logical history:

1. primary terminal content records hard line termination versus soft autowrap continuation;
2. retained logical content is not rewritten when viewport width changes;
3. a width-specific derived layout/index maps logical `(LineId, text-unit offset)` ranges to visual rows;
4. resize invalidates only affected layout/index caches, not canonical history or durable anchors;
5. search operates on logical content, not visual wrapped rows;
6. selection/Block/history anchors remain content-relative, never viewport-row-relative;
7. alternate-screen content does not enter primary scrollback; alternate-screen resize remains screen-state behavior and may consume global `LineId`s without implying primary-history continuity.

A production implementation may eagerly materialize the active/near-visible window and lazily derive older visual rows by segment. Any lazy index is rebuildable/derived and cannot become a second canonical history.

## Persistence and compression direction

A sealed immutable segment is the handoff unit for future P3 cold history persistence (ADR-007), but persistence is not allowed to gate terminal feed/render progress.

- sealing a segment is an in-memory terminal-state operation;
- persistence/compression work is queued asynchronously after sealing;
- failure to persist cannot block or corrupt live terminal semantics;
- durable history recovery never claims to restore a live PTY;
- compression format is not chosen by this spike;
- the simple RLE experiment only proves a serialization/compression seam exists and is not evidence to standardize RLE.

## Remaining production gates

Before M002 scrollback/reflow production work can be Ready:

1. accept the architecture decision derived from this R&D;
2. accept #684's canonical Unicode/grapheme/width representation or restrict the first implementation slice to an already accepted compatible payload seam;
3. define the M002 terminal specification for hard/soft-wrap mutations, resize/reflow, anchor invalidation on eviction, history limits and alternate-screen behavior;
4. repeat benchmarks on macOS with the final representation, including 1/10/50/100 execution profiles;
5. add repository VT fixtures/property tests/fuzz cases for reflow, wide/grapheme content, resize oscillation, alternate screen, search/selection anchors and bounded eviction.

## Conclusion

The spike supports a permanent architecture of **canonical logical history + mutable tail + immutable byte-targeted segments + derived lazy/eager-windowed reflow indexes**. It rejects canonical visual-row scrollback, rejects fixed line-count segmentation, and keeps persistence/compression asynchronous and subordinate to the one `TerminalState` authority.
