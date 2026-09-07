# SPEC-011 — M002 Unicode, Grapheme, Width, Projection, and IME Contract

- **Status:** Active; M002 production profile frozen by #815
- **Issue:** #684 architecture R&D; #815 production-profile refinement
- **Parent milestone:** #664
- **Architecture:** ADR-011
- **Coordinates with:** accepted ADR-010 / #685 retained-history/reflow contract
- **Production calibration:** [`../evidence/m002-unicode-production-calibration.md`](../evidence/m002-unicode-production-calibration.md)

## Purpose

Define the observable M002 contract for Unicode decoding, grapheme clustering, terminal-cell width, wide-cell occupation, projection transport, renderer shaping ownership and macOS IME behavior.

This specification does not implement the contract. It is the behavioral authority beneath ADR-011 and above M002 implementation work. #815 freezes the production values that #684 deliberately left for calibration; those values are normative here and the linked evidence records their rationale/provenance.

## Scope

In scope:

- streaming UTF-8 decode and malformed-input recovery;
- Unicode-core versus legacy compatibility semantics;
- extended grapheme grouping and active-cluster mutation;
- terminal occupation width and continuation-cell semantics;
- East Asian Ambiguous width policy;
- late VS15/VS16/ZWJ/RI width changes while a grapheme is active;
- bounded grapheme storage and overflow behavior;
- canonical history text-unit requirements consumed by #685;
- versioned local display projection for multi-scalar text;
- renderer shaping/font-fallback/cache ownership;
- native macOS marked-text/commit/cancel/candidate-coordinate behavior;
- required tests, fuzzing and performance/resource gates.

Out of scope:

- exact production slab/arena allocator implementation beyond the bounds and observable behavior fixed here;
- graphics/image protocols;
- BiDi terminal semantics;
- persistence backend design;
- agent/workspace semantic processing.

## Frozen M002 production profile

The following values are normative for #816/#817 and MUST NOT be changed opportunistically inside an implementation PR:

| Parameter | M002 value |
| --- | --- |
| Unicode semantic data | Unicode **17.0.0** |
| Maximum active canonical-grapheme payload | **8,192 UTF-8 bytes** |
| Maximum live variable grapheme payload per `TerminalState` active/grid storage | **2 MiB (2,097,152 bytes)** |
| Grapheme display capability | `CAP_GRAPHEME_DISPLAY = 1 << 6` |
| Grapheme display message types | snapshot `27`, delta `28` |
| Grapheme display schema | `2` |
| Fixed projected physical-cell record | **16 bytes** |
| Per-chunk grapheme sidecar | **65,536 bytes** maximum |
| Display transport-batch maximum | existing **4 MiB** per presentation batch |
| V2 chunk cell addressing | whole-row **or** contiguous partial-row cell span via `first_col` |

Changing one of these values requires a focused specification review with compatibility/resource evidence. #673 remains the authority for release-level performance ceilings.

Projection completeness invariant: every legal Unicode-core `TerminalState` within the frozen grapheme/live-store caps MUST be losslessly representable by grapheme display v2 (possibly using partial-row chunks and multiple 4 MiB transport batches for one logical generation update). Runtime MUST NOT require weakening Unicode caps to make projection succeed.

## Terms

### Unicode-core mode

DEC private mode 2027 set. Extended grapheme clusters are the terminal text unit.

### Legacy mode

DEC private mode 2027 reset. Non-zero-width printable scalars retain scalar/wcwidth-style placement compatibility; zero-width combining scalars may decorate the preceding eligible cell.

### Canonical grapheme

A `TerminalState`-owned semantic text unit containing canonical UTF-8 payload (or an explicit overflow sentinel), terminal occupation width and style.

### Lead cell

The canonical grid cell that owns a grapheme's text/style identity.

### Continuation cell

The second physical cell occupied by a width-2 grapheme. It has no independent text identity.

### Active grapheme anchor

Bounded terminal mutation state proving that future compatible scalars may still extend a specific lead cell.

## Invariants

1. `TerminalState` is the only terminal Unicode/grid authority.
2. PTY bytes are never synchronously gated on shaping, font fallback, persistence, search, agents, cloud or licensing.
3. Renderer/client text is derived and disposable.
4. IME marked/preedit text is not terminal state.
5. A width-2 canonical grapheme is always one lead plus one continuation; the continuation owns no text.
6. Rebuilding all client/renderer/history indexes from canonical terminal state must reproduce the same semantic text and cell occupation.
7. Host locale/font metrics cannot change terminal width semantics for the same pinned policy/version.
8. Resource pressure cannot silently create dangling text references, orphan continuation cells or fabricated history.

## 1. UTF-8 decode contract

### 1.1 Arbitrary chunking

For any valid UTF-8 byte sequence, feeding the same bytes in any PTY-read partitioning MUST emit the same Unicode scalar/control event sequence.

### 1.2 Partial sequence

Ending one PTY read in the middle of a valid codepoint MUST retain bounded decoder state and MUST NOT emit U+FFFD solely because the read ended.

### 1.3 Truncated stream

When a byte stream is explicitly finished/terminated with an incomplete UTF-8 sequence, the decoder MUST emit the deterministic replacement behavior selected by the parser contract.

### 1.4 Malformed input

Malformed UTF-8 MUST:

- produce U+FFFD according to the decoder recovery rule;
- preserve subsequent valid bytes through reprocessing where required;
- remain bounded under hostile byte streams;
- never panic or buffer unbounded input.

### 1.5 Controls

C0/C1/escape/parser controls MUST remain distinct terminal events. The UTF-8 decoder MUST NOT decide whether a control terminates grapheme append eligibility.

## 2. Unicode data contract

### 2.1 Pinned semantic data

M002 production MUST use **Unicode 17.0.0** semantic data for:

- extended grapheme breaking;
- East Asian width;
- emoji/variation-selector-sensitive width behavior used by the terminal core.

The build MUST expose/test the pinned semantic version. It MUST NOT derive semantic width/grapheme behavior from CoreText, AppKit, locale or installed fonts.

A library such as `unicode-segmentation` may provide reviewed implementation data/algorithms only when its exposed Unicode version is exactly compatible with this profile. A dependency does not become architecture authority.

### 2.2 Upgrade discipline

Changing the Unicode semantic data version MUST regenerate retained Unicode fixtures and produce a reviewable compatibility diff for changed grapheme boundaries/widths before release. A Unicode-version bump is a specification-affecting change, not an incidental dependency update.

## 3. Mode 2027 contract

### 3.1 Default

Private mode 2027 MUST default to **set** for a fresh terminal state.

### 3.2 DECSET

`CSI ? 2027 h` MUST enable Unicode-core mode for future input.

### 3.3 DECRST

`CSI ? 2027 l` MUST enable legacy mode for future input.

### 3.4 DECRQM

The terminal MUST report mode 2027 as recognized and changeable, with the returned state matching the actual mode.

### 3.5 Mode transition

Changing mode 2027 MUST:

- invalidate the active grapheme anchor before future printable input;
- leave already committed cells/history unchanged;
- not resegment prior text;
- not rewrite prior width occupation merely because the mode changed.

## 4. Ambiguous-width contract

### 4.1 Default

East Asian Ambiguous characters MUST occupy one cell by default.

### 4.2 Explicit compatibility profile

A CJK ambiguous-width=2 profile MAY be supported. If changed at runtime, the terminal MUST invalidate active grapheme append eligibility before subsequent text and MUST NOT retroactively rewrite existing content.

### 4.3 No font inference

Ambiguous-width policy MUST NOT change because a fallback font renders a glyph wider/narrower.

## 5. Canonical grapheme storage contract

### 5.1 Lead representation

A lead cell MUST provide semantic access to:

- canonical text payload or explicit overflow sentinel;
- terminal occupation width;
- style;
- role=`Lead`.

The implementation MAY encode a single scalar inline.

### 5.2 Variable payload

Multi-scalar text MUST be owned by bounded, reclaimable `TerminalState` storage. References MUST be generation/bounds safe.

Live variable payload for active/grid canonical text MUST NOT exceed **2,097,152 bytes per `TerminalState`**. The limit applies to live payload, not allocator capacity and not inline scalar cells. Dead payload MUST be reclaimable and the implementation MUST NOT eagerly preallocate the full cap per execution.

### 5.3 Continuation representation

A continuation cell MUST:

- identify itself as continuation;
- resolve to its lead for overwrite/hit-test purposes;
- contain no independent text payload or style authority.

### 5.4 Reclamation

Erasing/overwriting/replacing canonical text MUST make dead variable payload reclaimable. Append-only dead-payload retention is non-conforming.

### 5.5 Aggregate live-store fallback

If admitting/replacing a grapheme would exceed the 2 MiB live-store cap after bounded reclamation, the affected grapheme MUST use the same unavailable-payload sentinel/fallback family defined in §9. The terminal MUST NOT evict unrelated live visible canonical text merely to make the new grapheme fit, and PTY progress MUST remain nonblocking.

## 6. Active grapheme contract

### 6.1 Append eligibility

In Unicode-core mode, printable scalars MAY extend the current active grapheme while its anchor remains valid under extended grapheme rules.

### 6.2 Required invalidation

The active anchor MUST be invalidated by:

- explicit positional cursor movement;
- erase affecting the anchored lead/continuation;
- insert/delete cell/character operations affecting the anchor;
- structural scroll/row movement affecting identity;
- primary/alternate screen switch;
- reset;
- mode 2027 change;
- ambiguous-width policy change;
- replacement of the anchored canonical unit.

Returning the cursor to the same coordinate after invalidation MUST NOT resurrect the old anchor.

### 6.3 Non-positional actions

Pure non-positional actions MAY preserve the active anchor. SGR/style changes MUST NOT retroactively restyle the existing active grapheme; newly created graphemes use the current style.

### 6.4 Boundary ownership

Anchor invalidation is a terminal mutation decision. It MUST NOT be hidden in the UTF-8 decoder or renderer.

## 7. Width and cell-occupation contract

### 7.1 Width domain

A canonical terminal text unit MUST resolve to terminal occupation 0, 1 or 2 according to the active semantic policy.

### 7.2 Wide unit

Width 2 MUST occupy adjacent lead+continuation cells on one row. A canonical wide unit MUST NOT be split across rows.

### 7.3 Overwrite

Writing/erasing over either the lead or continuation of an existing width-2 unit MUST remove/replace the entire prior canonical unit so no orphan half remains.

### 7.4 Renderer independence

The renderer MAY produce one or more glyphs for a canonical grapheme but MUST render inside the terminal-provided cell rectangle and MUST NOT change semantic width.

## 8. Late width-change contract

### 8.1 Active-only mutation

VS15/VS16, modifiers, ZWJ/RI completion or another scalar MAY change width only while the same canonical grapheme remains active.

After anchor invalidation, later input MUST NOT retroactively move unrelated committed cells.

### 8.2 Late widening with DECAWM set

When an active width-1 grapheme widens to width 2 and cannot fit on the current row:

- the complete active grapheme MUST be atomically re-placed on the next row;
- it MUST remain one canonical unit;
- the previous row MUST record soft-wrap lineage compatible with #685;
- cursor/pending-wrap state MUST reflect placement of the final width-2 unit;
- no half-glyph or independent continuation text may remain on the previous row.

### 8.3 Late narrowing

A still-active width-2 grapheme MAY narrow and release its continuation. Narrowing MUST NOT pull later committed content backward.

### 8.4 DECAWM reset edge — frozen by #815

The M002 compatibility rule is now exact:

1. **New width-2 unit at final column:** when DECAWM is reset and a newly completed width-2 canonical unit begins at the final column, that complete unit is ignored atomically. Grid, cursor, pending-wrap state, hard/soft lineage and active-grapheme anchor remain unchanged. No lead/continuation half is created.
2. **Late widening at final column:** when an already committed active width-1 grapheme occupies the final column and a later scalar would widen it to width 2 while DECAWM is reset, the width-changing extension is rejected. The already committed prefix/payload, width-1 occupation, cursor and lineage remain unchanged; the rejected extension MUST NOT enter canonical payload.

This policy follows retained cross-implementation evidence recorded in the #815 calibration and preserves the stronger invariant that impossible wide occupation can neither split a unit nor retroactively delete already committed text.

## 9. Pathological grapheme/resource contract

### 9.1 Frozen bounds and counters

M002 production limits are:

- maximum retained payload for one active canonical grapheme: **8,192 UTF-8 bytes**;
- maximum live variable text bytes per `TerminalState` active/grid storage: **2,097,152 bytes**;
- `grapheme_payload_overflow_count`: saturating `u64`, aggregate/non-content;
- `grapheme_store_capacity_fallback_count`: saturating `u64`, aggregate/non-content.

The retained `base + 4096 combining marks` probe is 8,193 UTF-8 bytes, so it crosses the single-grapheme bound deterministically.

### 9.2 Overflow behavior

If one active grapheme exceeds 8,192 bytes:

- already committed terminal width/occupation MUST remain stable;
- additional payload for that grapheme MUST stop growing resident memory;
- copy/search/selection MUST return U+FFFD for the unavailable exceptional payload;
- exactly bounded aggregate diagnostic state MAY record the overflow, but terminal contents MUST NOT be logged;
- input MUST recover normally at the next grapheme boundary.

The same unavailable-payload sentinel family applies to a live-store capacity fallback under §5.5. The two counters distinguish per-grapheme overflow from aggregate live-store pressure without recording text.

### 9.3 No quadratic full rebuild

The production hot path MUST NOT require reconstruction/resegmentation of the complete accumulated grapheme payload for every appended scalar. The implementation must use incremental/bounded state or prove equivalent bounded cost.

## 10. History/reflow contract

### 10.1 Canonical history unit

The canonical history text unit consumed by #685 MUST expose:

```text
utf8 payload or overflow sentinel
terminal width
style
```

Renderer glyph runs/font identities are not part of canonical history.

### 10.2 Soft/hard lineage

Unicode text mutation MUST preserve/produce explicit hard-break versus soft-wrap lineage required for later history reflow. Late widening that wraps under DECAWM MUST produce soft-wrap lineage, not a fabricated hard newline.

### 10.3 Anchors

Search/selection/history anchors operate on canonical text-unit offsets/ranges, not renderer glyphs or current visual-row columns.

## 11. Projection transport contract

### 11.1 Version/capability transition

M002 grapheme display MUST use:

```text
CAP_GRAPHEME_DISPLAY = 1 << 6
DisplaySnapshotV2    = message type 27 (R→C)
DisplayDeltaV2       = message type 28 (R→C)
display schema       = 2
```

Capability bit 5 and message type 26 remain owned by Pass 8 Block metadata. Existing M001 display types 12/13 retain their scalar-only meanings. The SPEC-004 envelope remains protocol major/minor 1.0 because the v2 display is explicitly capability-gated with distinct message IDs; no old record layout is reinterpreted.

A production M002 graphical client MUST advertise `CAP_GRAPHEME_DISPLAY`. Runtime MUST NOT send types 27/28 to a peer lacking the capability.

A peer without bit 6 MAY receive legacy types 12/13 only when the complete projected batch is losslessly representable by the M001 scalar schema. If not, Runtime MUST use the existing bounded `DisplayUnavailable` error family rather than silently dropping/modifying grapheme state.

### 11.2 V2 chunk layout

A v2 display payload consists of:

```text
48-byte chunk header
N × 16-byte physical-cell records
sidecar_len bytes chunk-local UTF-8 sidecar
```

Header bytes 0..40 retain the existing Candidate-D chunk fields/order (`generation`, `base_generation`, geometry, cursor, modes, `first_row`, `row_count`, `chunk_index`, `chunk_count`, `cell_count`). Bytes 40..48 are:

```text
40..44  sidecar_len : u32 little-endian
44..46  schema      : u16 little-endian = 2
46..48  first_col   : u16 little-endian
```

`first_col` replaces the previously unused reserved `u16`. The 48-byte header size is unchanged.

Decoder validation MUST perform checked arithmetic before allocation/use and require:

```text
48 + cell_count * 16 + sidecar_len == payload_len
sidecar_len <= 65,536
payload_len <= MAX_FRAME_PAYLOAD (262,144)
first_col < columns
cell_count >= 1
```

The existing `MAX_DISPLAY_BATCH_BYTES = 4 MiB` remains the maximum size of one presentation transport batch. It is not a limit on the total encoded size of one logical generation update (see §11.8).

### 11.3 V2 cell-span addressing

M001 Candidate-D chunks encode whole rows only. Grapheme display v2 MUST additionally allow contiguous **partial-row** cell spans so a legal row whose variable payloads exceed one 65,536-byte sidecar remains representable without splitting a grapheme across chunks.

Exact addressing rules:

1. **Whole-row span (permitted):** `first_col == 0`, `row_count >= 1`, and `cell_count == row_count * columns`. The chunk carries rows `[first_row, first_row + row_count)` in full, identical to the M001 rectangular packing shape.
2. **Partial-row span (required when needed for sidecar/frame fit):** `row_count == 1`, `first_col >= 0`, and `first_col + cell_count <= columns`. The chunk carries exactly the physical cells at columns `[first_col, first_col + cell_count)` of `first_row`.
3. No other `(first_col, row_count, cell_count)` combinations are valid.
4. A width-2 lead and its continuation MUST occupy adjacent physical cells and MUST be co-encoded in the same chunk. A chunk boundary MUST NOT fall between a lead and its continuation.
5. Sidecar references remain chunk-local. A grapheme payload MUST NOT be split across chunks; the encoder places the entire referenced UTF-8 payload in the chunk that contains that lead cell.

Producer obligation: when packing damaged/snapshot cells, the encoder MUST emit the smallest number of valid spans that keep each chunk inside `sidecar_len <= 65,536` and `payload_len <= MAX_FRAME_PAYLOAD`. A single dense row MAY therefore become multiple `row_count == 1` partial-row chunks.

Proof obligation retained by this rule: nine legal 8,192-byte width-1 graphemes on one row require `9 × 8192 = 73,728` sidecar bytes, which exceeds one sidecar. Partial-row spans of at most eight such graphemes per chunk (`8 × 8192 = 65,536`) keep the row representable without changing Unicode caps or the 48-byte header size.

### 11.4 V2 16-byte physical-cell record

```text
0..4    text_ref   : u32 little-endian
4..8    foreground : existing DisplayColor encoding
8..12   background : existing DisplayColor encoding
12..16  meta       : u32 little-endian
```

`meta` is:

```text
bits  0..2   bold / underline / inverse (existing meanings)
bits  3..4   role: 0 Empty, 1 Lead, 2 Continuation, 3 invalid
bits  5..6   terminal width: 0, 1 or 2; 3 invalid
bit      7   sidecar-reference flag
bits  8..20  sidecar_len_minus_1 (13 bits; 1..8192 when sidecar=1)
bits 21..31  reserved = 0
```

Role requirements:

- `Empty`: `text_ref=0`, width=0, sidecar=0, length bits=0. Color/attribute metadata MAY paint an empty cell.
- `Lead`, inline scalar: width=1 or 2, sidecar=0, length bits=0 and `text_ref` MUST be one valid Unicode scalar.
- `Lead`, variable/multi-scalar payload: width=1 or 2, sidecar=1, `text_ref` is a byte offset from this chunk's sidecar start, and payload length is `sidecar_len_minus_1 + 1` in 1..8192.
- `Continuation`: `text_ref=0`, width=0, sidecar=0, length bits=0. It carries no independent text authority. Any projected colors/attributes are presentation metadata derived from its lead, never terminal semantic authority.

The producer MUST serialize sidecar payloads in physical-cell order within the chunk's cell span with no gaps/overlap. The decoder MUST validate that canonical order, UTF-8, bounds, role/width consistency and lead/continuation adjacency before exposing a chunk to the disposable client cache.

### 11.5 Inline scalar

A single-scalar lead SHOULD use the inline record when representable. It MUST NOT allocate sidecar bytes merely for convenience.

### 11.6 Sidecar bounds

Each v2 chunk sidecar is limited to **65,536 bytes** and each referenced grapheme to **8,192 bytes**. All offset+length calculations MUST be overflow checked. A sidecar reference MUST be wholly contained in the current chunk's sidecar and MUST NOT point into another frame/chunk/transport-batch.

### 11.7 Logical update identity

One logical display update is identified by:

```text
(kind, generation, base_generation, rows, columns, chunk_count)
```

`chunk_index` values `0 .. chunk_count-1` enumerate every chunk of that logical update exactly once, covering the required snapshot rows or delta damage cell spans without gaps/overlap in row-major order. All chunks of one logical update repeat identical generation/base/dimensions/cursor/mode values.

### 11.8 Multi-batch transport transaction

Proof obligation: a maximum-geometry fixed-cell payload is `512 × 256 × 16 = 2,097,152` bytes. Together with the maximum live variable payload of `2,097,152` bytes, content alone reaches the existing 4 MiB presentation-batch ceiling before frame/chunk header overhead. Therefore a legal `TerminalState` can require more than one 4 MiB transport batch for one logical generation update.

Normative rules:

1. `MAX_DISPLAY_BATCH_BYTES = 4 MiB` continues to bound one presentation **transport batch** (encoded frames delivered as one replaceable presentation unit under SPEC-004 backpressure).
2. One logical v2 snapshot/delta MAY be fragmented across multiple transport batches. Each batch carries a contiguous subsequence of the logical update's chunks.
3. The client MUST assemble/validate all `chunk_count` chunks for the logical update before atomically committing the disposable display cache for that generation. Partial transport progress MUST NOT become authoritative client display state.
4. Existing SPEC-004 supersession remains in force: a newer logical update replaces not-yet-committed pending presentation work. An incomplete multi-batch assembly that is superseded MUST be discarded.
5. Malformed/missing/overlapping/gapped chunks for a logical update invalidate that update and force the existing bounded resync path. They MUST NOT partially commit.
6. Runtime MUST NOT fail projection of a legal Unicode-core `TerminalState` solely because the encoded logical update exceeds 4 MiB; it MUST fragment transport batches instead. `DisplayUnavailable` remains reserved for capability/negotiation/lossy-legacy cases, not for representable v2 size fragmentation.

### 11.9 Atomic commit/resync

Malformed v2 projection data invalidates the complete logical update (all of its transport batches/chunks). A client MUST NOT partially commit malformed role/width/sidecar/cell-span state and continue. It MUST use the existing bounded resync path.

### 11.10 Reconstructability

Dropping all client display state and rebuilding from the authoritative source MUST recover the same canonical text roles/width/style. No v2 client cache becomes canonical text authority.

## 12. Renderer contract

### 12.1 Input

Renderer preparation receives derived grapheme UTF-8 + style + terminal cell rectangle/role.

### 12.2 Ownership

Renderer owns presentation-only:

- font fallback;
- shaping;
- raster/glyph atlas;
- bounded shape cache;
- scale/font/theme invalidation.

### 12.3 Width prohibition

Renderer typographic metrics MUST NOT alter terminal cursor/grid/history width decisions.

### 12.4 Cache correctness

Shape-cache keys MUST include all inputs necessary for correct presentation, including at minimum grapheme payload, font/style identity, scale and fallback-generation context.

### 12.5 Hot-path separation

A cache miss MAY require shaping during renderer preparation, but PTY parsing/`TerminalState` mutation MUST NOT synchronously wait for CoreText/AppKit.

## 13. macOS IME contract

### 13.1 Preedit ownership

Marked/preedit text MUST remain native `NSTextInputClient` state and MUST NOT be written into `TerminalState`, history or Blocks.

### 13.2 Commit

Committed text MUST be converted to UTF-8 and admitted atomically through the existing native-input/Runtime path. Multi-scalar commits MUST preserve byte ordering as one committed input action even though the remote application may process bytes incrementally.

### 13.3 Cancel/abandon

Cancelling/abandoning composition MUST remove preedit without emitting terminal input.

### 13.4 Replacement range

Replacement ranges apply only to the native marked-text document owned by the input surface; they MUST NOT directly edit historical terminal cells.

### 13.5 Candidate coordinates

Candidate-window coordinates MUST derive from current authoritative cursor/layout projection and remain valid under scale/window movement.

### 13.6 Lifecycle

Detach, reconnect or input-surface destruction MUST discard stale marked text. IME preedit is not persisted across attachment lifetime.

## 14. Security/failure behavior

- malformed UTF-8 and projection sidecars are untrusted input;
- integer/range arithmetic MUST be checked;
- hostile combining storms MUST be bounded by the 8,192-byte grapheme and 2 MiB live-store limits;
- renderer/font errors MUST fall back to presentation failure/tofu without changing canonical terminal text/width;
- no diagnostic artifact may include arbitrary terminal contents by default;
- no Unicode/IME feature may add a cloud/commercial dependency to OSS terminal progress.

## 15. Required deterministic fixtures

At minimum:

1. ASCII baseline;
2. base + combining mark;
3. isolated combining mark behavior;
4. CJK width-2;
5. East Asian Ambiguous default=1 and explicit CJK=2;
6. heart + VS15;
7. heart + VS16;
8. emoji modifier;
9. ZWJ sequence;
10. family emoji;
11. regional flag;
12. keycap;
13. Tamil/Indic combining sample;
14. Arabic combining sample;
15. supplementary-plane scalar;
16. byte-at-a-time UTF-8 feed;
17. every split of a 4-byte scalar;
18. malformed sequence followed by valid ASCII;
19. truncated UTF-8 on explicit finish;
20. control interleaved with combining sequence;
21. mode 2027 set/reset/query;
22. mode switch invalidates active anchor;
23. cursor move invalidates anchor even after returning;
24. SGR change does not retroactively split/restyle active grapheme;
25. overwrite wide lead;
26. overwrite wide continuation;
27. right-edge late widen with DECAWM set;
28. DECAWM reset direct width-2-at-final-column is ignored, plus late-widen extension rejection preserving the committed width-1 prefix;
29. width-2 alternate-screen behavior;
30. `base + 4096 combining marks` crosses 8,192 bytes, yields bounded overflow/sentinel exactly once for the grapheme and recovers on the next grapheme;
31. projection v2 single-scalar inline round-trip;
32. projection v2 multi-scalar sidecar round-trip;
33. projection v2 malformed/out-of-bounds/overlapping/gapped/invalid-role/invalid-width/invalid-reserved sidecar rejection and resync;
34. v2 capability/message negotiation does not reinterpret legacy types 12/13 or BlockState type 26;
35. one row with nine 8,192-byte width-1 graphemes encodes via partial-row spans without loss or Unicode-cap weakening;
36. max-geometry snapshot whose fixed cells plus live variable payload exceed 4 MiB fragments across multiple transport batches and applies atomically only when complete;
37. IME marked text -> commit;
38. IME cancel/abandon;
39. IME replacement commit;
40. IME candidate-coordinate validity;
41. detach/reconnect discards stale preedit.

## 16. Property/fuzz requirements

Production work MUST add or extend fuzz/property coverage for:

- arbitrary PTY chunk partitions;
- malformed UTF-8 byte streams;
- long combining/ZWJ/RI sequences;
- active-anchor invalidation sequences mixed with controls/cursor motion;
- lead/continuation overwrite invariants;
- mode 2027 transitions;
- width-policy transitions;
- 8,192-byte grapheme payload overflow and 2 MiB live-store pressure;
- projection-v2 sidecar lengths/offsets/UTF-8/role/width/`first_col`/cell-span combinations;
- partial-row span packing under the 65,536-byte sidecar and frame-payload ceilings;
- multi-batch logical-update assembly, supersession, and incomplete-update rejection;
- v1/v2 capability/message negotiation and resync;
- resize/reflow integration with canonical grapheme units.

Fuzzing MUST prove bounded memory/progress rather than merely absence of crashes.

## 17. Performance/resource acceptance

Numeric release gates are owned by #673, but M002 Unicode implementation MUST measure at minimum:

- scalar decode/feed throughput;
- Unicode-core incremental grapheme mutation p50/p95/p99;
- legacy-mode mutation;
- Unicode-heavy high-output throughput;
- active-grid/text-store RSS under overwrite and combining storms;
- projection bytes/cell and sidecar frequency;
- renderer shape-cache hit/miss cost;
- ARM64 macOS shaping/fallback cost;
- IME commit latency;
- 1/10/50/100 execution resource scaling with Unicode-heavy retained history.

#816/#817 MUST compare exact-head results against the closest accepted baseline and explain any material regression. Acceptance MUST distinguish semantic-path time from renderer shaping and MUST NOT present CI-host shaping microbenchmarks as key-to-photon latency.

## 18. Implementation readiness after #815

The #684 architecture spike is complete and its accepted ownership model remains unchanged. #815 closes the production-value gaps that previously blocked Unicode implementation:

- Unicode version is pinned to 17.0.0;
- grapheme/live-store bounds and non-content counters are exact;
- DECAWM-reset wide-at-edge behavior is exact;
- Candidate-D v2 capability, message IDs, 48-byte header/cell packing, sidecar bounds, partial-row cell-span addressing, and multi-batch transport transaction rules are exact;
- every legal Unicode-core `TerminalState` within those caps remains losslessly projectable without weakening Unicode semantics.

After #815 is reviewed and merged, #816 may pass its separate exact-head development-readiness gate. #817 remains dependency-blocked on #816 and must implement the v2 encoder/decoder/transaction rules frozen here. #673 remains the authority for release-level performance ceilings.

Acceptance of this specification does **not** mean M002 Unicode production implementation is complete.