# SPEC-011 — M002 Unicode, Grapheme, Width, Projection, and IME Contract

- **Status:** Active
- **Issue:** #684
- **Parent milestone:** #664
- **Architecture:** ADR-011
- **Coordinates with:** accepted ADR-010 / #685 retained-history/reflow contract

## Purpose

Define the observable M002 contract for Unicode decoding, grapheme clustering, terminal-cell width, wide-cell occupation, projection transport, renderer shaping ownership and macOS IME behavior.

This specification does not implement the contract. It is the behavioral authority beneath ADR-011 and above M002 implementation work.

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

- exact production slab allocator implementation;
- exact numeric memory limits before production calibration;
- exact binary field packing/capability number for Candidate-D vNext;
- graphics/image protocols;
- BiDi terminal semantics;
- persistence backend design;
- agent/workspace semantic processing.

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

A production build MUST record one explicit Unicode semantic data version used for:

- extended grapheme breaking;
- East Asian width;
- emoji/variation-selector-sensitive width behavior used by the terminal core.

The build MUST NOT derive semantic width/grapheme behavior from CoreText, AppKit, locale or installed fonts.

### 2.2 Upgrade discipline

Changing the Unicode semantic data version MUST regenerate retained Unicode fixtures and produce a reviewable compatibility diff for changed grapheme boundaries/widths before release.

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

### 5.3 Continuation representation

A continuation cell MUST:

- identify itself as continuation;
- resolve to its lead for overwrite/hit-test purposes;
- contain no independent text payload or style authority.

### 5.4 Reclamation

Erasing/overwriting/replacing canonical text MUST make dead variable payload reclaimable. Append-only dead-payload retention is non-conforming.

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

### 8.4 DECAWM reset edge

The exact wide-at-final-column rule with DECAWM reset remains a required #672 compatibility fixture. M002 production mode-2027 work MUST NOT be considered complete until that fixture is accepted. Regardless of the selected behavior, a width-2 canonical unit MUST never be split or represented by independent text halves.

## 9. Pathological grapheme/resource contract

### 9.1 Required bounds

Before production implementation is Ready, the implementation issue MUST define:

- maximum retained bytes for one active canonical grapheme;
- maximum resident variable text bytes per `TerminalState` active/grid storage;
- observable counters for overflow/resource fallback.

### 9.2 Overflow behavior

If one active grapheme exceeds its hard payload limit:

- already committed terminal width/occupation MUST remain stable;
- additional payload for that grapheme MUST stop growing resident memory;
- copy/search/selection MUST return U+FFFD for the unavailable exceptional payload;
- exactly bounded diagnostic state MAY record the overflow, but terminal contents MUST NOT be logged;
- input MUST recover normally at the next grapheme boundary.

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

### 11.1 Version transition

A grapheme-capable projection MUST use an explicit protocol version/capability transition from the M001 scalar-only projection. A client MUST NOT silently reinterpret an old record layout as a new grapheme schema.

### 11.2 Logical record

Each projected physical cell MUST encode enough derived information to distinguish:

- `Empty`;
- `Lead` with width/style/text reference;
- `Continuation` with no text authority.

### 11.3 Inline scalar

A single-scalar lead SHOULD be encoded inline when the wire schema permits.

### 11.4 Sidecar

A multi-scalar lead MAY reference bounded batch-local UTF-8 bytes in a sidecar.

Sidecar references MUST be validated for:

- bounds/overflow;
- UTF-8 validity;
- role consistency;
- width consistency;
- maximum batch payload.

### 11.5 Atomic commit/resync

Malformed projection data MUST fail the batch or force resync. A client MUST NOT partially commit malformed grapheme sidecar state and continue as if authoritative.

### 11.6 Reconstructability

Dropping all client display state and rebuilding from the authoritative source MUST recover the same canonical text roles/width/style.

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
- hostile combining storms MUST be bounded;
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
28. accepted DECAWM-reset wide-at-edge fixture from #672;
29. width-2 alternate-screen behavior;
30. 4096-combining-mark bounded overflow and recovery;
31. projection single-scalar inline round-trip;
32. projection multi-scalar sidecar round-trip;
33. malformed/out-of-bounds sidecar rejection/resync;
34. IME marked text -> commit;
35. IME cancel/abandon;
36. IME replacement commit;
37. IME candidate-coordinate validity;
38. detach/reconnect discards stale preedit.

## 16. Property/fuzz requirements

Production work MUST add or extend fuzz/property coverage for:

- arbitrary PTY chunk partitions;
- malformed UTF-8 byte streams;
- long combining/ZWJ/RI sequences;
- active-anchor invalidation sequences mixed with controls/cursor motion;
- lead/continuation overwrite invariants;
- mode 2027 transitions;
- width-policy transitions;
- grapheme payload overflow;
- projection sidecar lengths/offsets/UTF-8/role combinations;
- resize/reflow integration with canonical grapheme units.

Fuzzing MUST prove bounded memory/progress rather than merely absence of crashes.

## 17. Performance/resource acceptance

Numeric gates are owned by #673, but M002 Unicode implementation MUST measure at minimum:

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

Acceptance MUST distinguish semantic-path time from renderer shaping and must not present CI-host shaping microbenchmarks as key-to-photon latency.

## 18. Definition of done for #684 spike

The architecture spike is complete when:

- ADR-011 and this specification are accepted;
- the final #684 isolated evidence is retained in `SEYAL-UNICODE-GRAPHEME-RD-001.md`;
- non-mergeable spike PR #794 is closed without merge;
- production follow-ups remain assigned to #672/#673/#685-compatible implementation rather than leaving experimental code in the production tree.

Acceptance of this specification does **not** mean M002 Unicode production implementation is complete.
