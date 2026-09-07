# M002 scrollback / history production calibration

- **Issue:** #818
- **Parent:** #672
- **Architecture:** ADR-010
- **Behavioral authority:** SPEC-010
- **Depends on:** #816 (`1f2cc34` / PR #829) canonical grapheme/`Cell` footprint; #815 Unicode production caps
- **Comparative R&D:** `docs/architecture/SEYAL-SCROLLBACK-HISTORY-RD-001.md` (#685)
- **Purpose:** freeze segment, resident-memory, eviction, reflow, anchor, derived-cache and fixture contracts left open by ADR-010 / #685 before production `HistoryStore` work (#819)

## Decision summary

| Parameter | Frozen M002 value |
| --- | --- |
| Immutable segment payload target | **16 KiB (16,384 bytes)** encoded/uncompressed canonical payload |
| Mutable-tail policy | seal at segment target; absolute unsealed payload ceiling **32 KiB (32,768 bytes)** |
| Per-execution resident-history cap | **32 MiB (33,554,432 bytes)** |
| Runtime aggregate resident-history cap | **256 MiB (268,435,456 bytes)** |
| Aggregate eviction fairness | global oldest-sealed-first across executions; no attached-execution privilege; never fabricate PTY/process continuity |
| `HistoryAnchor.unit_offset` | **`u32`** (explicit overflow failure; never wrap/clamp) |
| Per-execution derived reflow-index budget | **4 MiB (4,194,304 bytes)** |
| Per-execution derived search-index budget | **4 MiB (4,194,304 bytes)** |
| Runtime aggregate derived-index/cache budget | **32 MiB (33,554,432 bytes)** |
| Active/near-visible reflow (physical ARM64 macOS) | p50 **≤ 2 ms**, p95 **≤ 4 ms**, p99 **≤ 8 ms** |
| Per sealed-segment lazy reflow (physical ARM64 macOS) | p50 **≤ 1 ms**, p95 **≤ 2 ms**, p99 **≤ 4 ms** |
| CI-host reflow/search numbers | comparative only; **not** release thresholds |

These values are M002 production contracts for #819. Changing them after implementation requires a specification review with resource/latency evidence; they are not opportunistic tuning knobs inside HistoryStore PRs. #673 remains the authority for versioned release-level terminal ceilings that may further tighten (never silently weaken) these implementation gates.

## #816 footprint baseline

Measured on the post-#816 production types in `seyal-terminal` (exact-head class `1f2cc34`):

| Type | `size_of` | `align_of` | Notes |
| --- | ---: | ---: | --- |
| `Cell` | **24** | 4 | lead/empty/continuation physical cell; includes `store_id` |
| `Style` | **11** | 1 | `Color` ×2 + bold/underline/inverse |
| `Color` | **4** | 1 | `Default` / `Indexed` / `Rgb` |
| `LineId` | **8** | 8 | ADR-004 identity |
| Display v2 physical-cell record | **16** | — | SPEC-011 projection; not history authority |
| Max active grapheme UTF-8 | **8,192** | — | SPEC-011 / #815 |
| Max live variable payload / `TerminalState` | **2 MiB** | — | active/grid only; **not** retained history |

Standalone layout probe (mirrors public `seyal-terminal` types) and path-dependent `cargo` probe against the crate both reported the same `Cell`/`Style`/`Color`/`LineId` sizes.

Interpretation for history encoding:

- Retained history stores **canonical text units + style + hard/soft lineage**, not renderer shaping and not a second live `GraphemeStore`.
- A dense styled unit whose in-memory authority resembles a `Cell` costs **24 bytes** before UTF-8 sidecar bytes for multi-scalar graphemes.
- A compact ASCII-heavy packing that stores `Style` + role/width/flags + 1-byte UTF-8 is about **13–16 bytes**/unit before segment metadata.
- Pathological units remain bounded by the **8,192-byte** active-grapheme cap; history never invents a larger canonical unit.

The #685 Linux segmented candidate measured **≈12.7 bytes/unit** RSS delta at 1M synthetic atoms (`12,388 KiB` / 1e6). Scaling that synthetic atom to the real **24-byte** `Cell` footprint implies roughly **≈23–24 MiB** for 1M dense units before grapheme UTF-8 sidecars — the primary calibration driver for the 32 MiB per-execution cap below.

## Segment target: 16 KiB

ADR-010 requires byte-targeted sealing, not fixed line counts. #685 showed:

| target units/segment | 10k-unit budget retention |
| ---: | ---: |
| 512 | 98.80% |
| 1,024 | 95.45% |
| 2,048 | 85.52% |
| 4,096 | 64.90% |

Mapping those unit targets onto the #816 footprint:

| Assumed encoded unit | Units in 16,384-byte segment |
| --- | ---: |
| 16-byte compact ASCII-styled unit | **1,024** |
| 24-byte `Cell`-equivalent dense unit | **682** |

Both land in the #685 high-retention band (512–1,024 units/segment). Larger power-of-two targets (64 KiB+) would recreate the coarse eviction granularity ADR-010 rejected.

**Frozen:** seal immutable segments when encoded/uncompressed **canonical payload** reaches **16,384 bytes** at a source/logical boundary (or earlier when fragmenting an oversized chain). The target is an implementation constant, not a public on-wire compatibility constant. Changing it does not change `LineId`, hard/soft lineage or anchor identity.

Segment metadata (per-source `LineId`, break kind, payload offset/length, first/last ranges, integrity) is counted toward the resident-history caps below but is **not** part of the 16 KiB payload seal trigger.

## Mutable-tail policy

```text
HistoryStore
  sealed Segment*
  mutable Tail   ← only unsealed append region
```

Normative policy:

1. Append into the mutable tail.
2. When tail **payload** reaches the **16,384-byte** segment target at a permitted seal boundary, seal into an immutable segment and start a new empty tail.
3. An oversized logical chain/source record that would exceed the segment target **must** fragment at canonical text-unit boundaries into sealed segments carrying the original `LineId` plus unit offset/range (ADR-010 §5). Fragmentation never allocates a new `LineId` or invents a hard break.
4. Absolute unsealed payload ceiling: **32,768 bytes** (two segment targets). This covers one sealing race (target-sized ready fragment + in-progress fragment) without allowing unbounded tail growth.
5. Detach, Resource-pressure eviction preparation, and future persistence handoff may force-seal a non-empty tail earlier than the target; force-seal still occurs only at unit/source boundaries.
6. The mutable tail is included in the per-execution resident-history byte cap.

## Per-execution resident-history cap: 32 MiB

**Frozen hard cap:** **33,554,432 bytes** of resident retained-history ownership per `ExecutionId`.

Counted toward the cap:

- sealed segment payload + segment metadata;
- mutable tail payload + tail metadata;
- history-owned UTF-8 for multi-scalar graphemes retained in segments/tail.

Not counted toward this cap (separate authorities):

- active primary/alternate `Screen` grid cells;
- live `GraphemeStore` (2 MiB SPEC-011 cap);
- derived reflow/search indexes (separate derived budgets below);
- cold/persisted copies after successful handoff that are no longer resident;
- renderer/client caches.

Rationale:

- #685 compact segmented RSS ≈ **12.1 MiB** at 1M synthetic units;
- scaling to the real 24-byte `Cell` class ≈ **23–24 MiB** at 1M dense units;
- **32 MiB** supplies ≈1.3–2.0M compact/dense units of headroom for real Style + UTF-8 while remaining a hard safety bound;
- adversarial all-max graphemes: `32 MiB / 8,192 ≈ 4,096` units — still finite and deterministic.

A user-visible line-count preference may exist later but **cannot** raise this hard byte cap.

## Runtime aggregate cap: 256 MiB + fairness

**Frozen hard aggregate:** **268,435,456 bytes** of resident retained history summed across all live and detached executions in one Runtime.

Implications:

- at most **8** executions may simultaneously sit at the full 32 MiB per-execution cap;
- 100 executions average at most **2.56 MiB** of resident history each if the aggregate is saturated;
- this matches the #685 observation that 100 × 100k-unit populations were on the order of **≈120 MiB** segmented RSS on the synthetic model, with headroom for the denser #816 unit.

### Eviction fairness (normative)

When either the per-execution or aggregate cap would be exceeded:

1. Evict **oldest sealed content first** (ADR-010). Prefer complete soft-wrapped logical chains when that still satisfies the byte need.
2. Under aggregate pressure, order candidates by **global seal/append age** across executions — **not** by attached vs detached, controller vs observer, or GUI focus. Fairness is age-based, not UX-priority-based.
3. Never split a canonical text unit. Oversized chains trim only at storage-fragment/unit boundaries.
4. Never evict arbitrary middle history.
5. Eviction **must not** fabricate live PTY/process continuity, resurrect child processes, rewrite exit status, or alter canonical **visible** primary/alternate screen contents.
6. Evicted anchors resolve to explicit `Evicted` / `Unavailable`; never silently clamp to a neighbor.
7. If no durable cold copy exists, eviction permanently removes that payload from the available history surface.
8. Derived indexes that reference evicted ranges are dropped or marked invalid; dropping derived data never counts as satisfying a canonical-history byte debt by itself.

## Resize / reflow latency gates

#685 Linux segmented reflow at ≈1M synthetic atoms: **p50 / p95 / p99 = 1,321 / 1,412 / 1,417 µs**. Those numbers are **comparative R&D only**.

### Physical-host implementation gates (ARM64 macOS, Release, exact head)

Measured by #819 (and release-checked under #673 where applicable) on a controlled physical host:

| Workload | p50 | p95 | p99 |
| --- | ---: | ---: | ---: |
| Active/near-visible window reflow (viewport + ≤2 screenfuls slack) | ≤ **2 ms** | ≤ **4 ms** | ≤ **8 ms** |
| Lazy reflow of one sealed segment at the 16 KiB target | ≤ **1 ms** | ≤ **2 ms** | ≤ **4 ms** |

Requirements:

- A resize **must not** require work proportional to all retained history before the active surface can progress (ADR-010).
- Large-history responsiveness is demonstrated by the active-window gate plus per-segment lazy cost, not by eagerly rewriting 1M units on every column change.
- Optional full-retained materialization benchmarks may be recorded for attribution but are **not** the resize acceptance path.

### CI-host evidence

CI VMs may record the same harness shapes for regression **comparison**. CI timings **must not** be labelled end-user key-to-photon latency and **must not** alone pass or fail the physical-host gates above.

### Benchmark matrix (#819 must retain)

| Dimension | Values |
| --- | --- |
| Retained canonical units | **10k / 100k / 1M** |
| Execution population | **1 / 10 / 50 / 100** |
| Column oscillation | **40 / 48 / 64 / 80 / 96 / 132 / 160** |
| Workloads | ASCII-dense; styled; width-2 CJK; family-emoji / combining (within SPEC-011 caps) |

Record append throughput/latency, reflow percentiles, search/anchor resolve cost, allocation churn and RSS attribution for each matrix cell that the implementation Issue claims.

## Search / selection anchors

Frozen durable primitive:

```text
HistoryAnchor {
  line_id: LineId      // u64
  unit_offset: u32     // canonical text-unit index within the source/logical mapping
}
```

Rationale for **`u32`**:

- geometric maxima are 512×256 physical cells, but a soft-wrapped logical chain may far exceed one viewport;
- `u32` addresses up to 4,294,967,295 units per mapped source identity — far above any resident 32 MiB budget (`32 MiB / 1 byte` still ≪ 2³²);
- overflow or malformed offsets **fail explicitly**; never wrap or clamp to unrelated content (SPEC-010).

Behavior notes for later UI (#820):

- selection start/end are source anchors plus affinity/direction metadata;
- hit testing maps current visual source spans → anchors;
- reflow remaps anchors to new visual rows without changing selected source content;
- selection may cross `SoftWrap` boundaries; `HardBreak` remains the logical newline separator;
- search operates on canonical ordered source content; width changes do not change matches;
- matches may cross `SoftWrap` source-record boundaries;
- results are source-anchor ranges, mapped to visual rows only for presentation;
- either endpoint evicted → explicit unavailable per selection/search API; endpoints are never silently moved.

## Derived-index / cache budgets

Derived reflow and search indexes are rebuildable and never canonical.

| Budget | Cap |
| --- | ---: |
| Per-execution reflow/layout derived indexes | **4 MiB** |
| Per-execution search acceleration indexes | **4 MiB** |
| Runtime aggregate of all derived history indexes/caches | **32 MiB** |

Rules:

- Cache keys include terminal/history generation (or immutable segment identity), column width, and eviction generation.
- On derived-budget pressure: drop least-recently-used / oldest derived caches first.
- Derived eviction **must not** delete canonical retained payload to free index bytes.
- Allocation failure for derived caches may drop/rebuild indexes without corrupting `TerminalState`.
- Dropping all derived caches and rebuilding from canonical history must reproduce the same visual layout for the same width and SPEC-011 width policy.

## Retained fixtures (exact list for #819)

Production HistoryStore/reflow work must retain deterministic fixtures for at least:

| ID | Fixture obligation |
| --- | --- |
| `hist-m001-corpus` | Replay retained M001 VT corpus (`m001-basic`, `m001-deferred-osc`, `m001-ecma48-core`, `m001-ecma48-erase-save`, `m001-xterm-private`, `m001-utf8`) through canonical history |
| `hist-hard-soft-lineage` | Explicit LF/VT/FF → `HardBreak`; autowrap/pending-wrap → `SoftWrap`; CR alone does not hard-break |
| `hist-long-soft-chain` | One logical chain spanning ≥8 wrapped rows and **crossing ≥1 segment boundary** |
| `hist-wide-grapheme` | SPEC-011 width-2 / combining / emoji / overflow-sentinel units in retained source |
| `hist-resize-oscillation` | Reflow oscillation across **40 / 48 / 64 / 80 / 96 / 132 / 160** columns; source payload and anchors unchanged |
| `hist-alt-screen-exclusion` | Enter/leave alternate screen; alternate frames **never** enter primary retained history; primary `LineId` gaps remain valid |
| `hist-selection-anchor-stable` | `(LineId, unit_offset)` selection endpoints survive resize/reflow |
| `hist-search-soft-span` | Search match crosses `SoftWrap`; results width-independent |
| `hist-evict-oldest` | Oldest-sealed-first eviction; evicted anchors → explicit unavailable |
| `hist-oversize-fragment` | Oversized logical chain fragments at unit boundaries without new `LineId`s |
| `hist-cache-rebuild-equiv` | Drop all reflow caches → rebuild equals prior visual projection |
| `hist-aggregate-fairness` | Multi-execution aggregate pressure evicts by global age without altering live screen/PTY liveness |

## Manual calibration verification

1. Confirm `size_of::<Cell>() == 24`, `Style == 11`, `Color == 4`, `LineId == 8` on the #816 production types.
2. Confirm 16 KiB / 16 B ≈ 1,024 units and 16 KiB / 24 B ≈ 682 units fall in the #685 512–1,024 retention band.
3. Confirm 1M × 24 B ≈ 22.9 MiB ≤ 32 MiB per-execution cap, and 256 MiB aggregate ⇒ ≤8 fully-capped executions.
4. Confirm `u32` unit offsets cannot wrap under the 32 MiB resident bound.
5. Confirm the fixture table above is copied into #819 acceptance without dropping alt-screen exclusion or resize oscillation.
6. Confirm this calibration PR contains **no** production `HistoryStore`/reflow implementation.

## Non-goals

This calibration does not implement `HistoryStore`, reflow algorithms, persistence backends, selection/search UI, renderer changes, or #673 release-ceiling amendments. Those remain separate Issues under #672/#673.
