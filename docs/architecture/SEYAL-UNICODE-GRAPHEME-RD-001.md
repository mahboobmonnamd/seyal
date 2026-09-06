# SEYAL-UNICODE-GRAPHEME-RD-001 — M002 Unicode / Grapheme / Width / IME Evidence

- **Issue:** #684
- **Parent:** #664
- **Spike PR:** #794 — non-mergeable evidence only
- **Final spike head:** `464b4c5ebffce044e84bcd88650db865f4a574f7`
- **Final spike-specific workflow:** `33978451414`
- **Decision:** ADR-011 / SPEC-011

## Purpose

Record the evidence that supports Seyal's permanent Unicode/grapheme/width/IME ownership decision without promoting experimental spike code into production.

The spike was deliberately isolated from production `seyal-terminal`, Runtime and native sources. The only graduation artifact is this evidence plus the architecture/specification decision.

## Question

What representation and ownership model can provide correct combining text, emoji/ZWJ/variation-selector behavior, wide-cell occupation, bounded pathological-input behavior, renderer shaping and macOS IME without creating a second text authority or putting platform shaping on the terminal hot path?

## Alternatives measured

The isolated harness compared:

1. one owned `String` per terminal cell;
2. fixed four-scalar inline cells;
3. compact state-owned grapheme references;
4. append-only versus reclaimable variable-length storage pressure;
5. fixed-size display projection plus batch-local multi-scalar sidecar;
6. bounded exceptional-cluster overflow semantics;
7. Unicode-grapheme versus legacy scalar placement contracts;
8. synchronous complete-cluster shaping versus bounded renderer cache lookup.

## Corpus and semantic probes

The retained spike corpus covers:

- ASCII;
- combining marks;
- CJK wide text;
- VS15 / VS16;
- emoji modifiers and ZWJ sequences;
- family emoji;
- regional-indicator flags;
- keycaps;
- Tamil and Arabic combining text;
- supplementary-plane scalars;
- isolated combining marks;
- East Asian Ambiguous characters;
- malformed and truncated UTF-8;
- arbitrary PTY byte chunking;
- control events interleaved with printable scalars;
- positional/non-positional mutation boundaries;
- width-2 lead/continuation overwrite;
- mode-2027 Unicode-core versus legacy compatibility;
- right-edge late widening;
- pathological `base + 4096 combining marks` growth;
- overwrite/reclamation pressure;
- ARM64 macOS font fallback and complete-grapheme shaping/cache behavior.

## Final exact-head test result

The final comparative lane executed **42 deterministic tests** and passed format, test and measurement enforcement.

Notable passing contracts include:

- arbitrary UTF-8 split invariance and truncated-sequence handling;
- malformed UTF-8 replacement plus following-byte reprocessing;
- Unicode-core family emoji collapsing to one two-cell unit;
- legacy mode retaining independent non-zero-width emoji-scalar placement;
- mode-2027 width changes being explicit compatibility behavior;
- positional cursor movement and destructive mutations invalidating the active grapheme anchor;
- non-positional actions preserving anchor identity where safe;
- style changes not retroactively splitting/restyling an active grapheme;
- mode switches invalidating the active anchor without rewriting prior cells;
- width-2 lead/continuation overwrite erasing the complete prior unit;
- bounded overflow preserving committed occupation and recovering on the next grapheme;
- single-scalar projection avoiding sidecar payload;
- multi-scalar grapheme sidecar round-trip;
- continuation cells having no independent text authority;
- explicit detection/reflow candidate for mode-2027 late widening at the right edge.

## Representation evidence

Measured Rust payload shape in the spike:

| candidate | payload bytes |
|---|---:|
| M001 scalar baseline | 8 |
| owned `String` cell | 32 |
| inline four-scalar cell | 20 |
| compact state reference | 8 |

The ordinary family emoji `👨‍👩‍👧‍👦` is seven scalars, immediately disproving a tiny fixed-only scalar representation as a complete model.

The spike deliberately modeled an append-only variable-length arena under overwrite pressure:

- 100,000 overwrites;
- 800,000 bytes retained by the append-only model;
- only 25 bytes remained live;
- a reclaimable slot model stayed bounded at 25 bytes capacity/live for the same sample.

Conclusion: compact references are useful only when their backing store is reclaimable and resource-bounded.

## Compatibility-mode evidence

The final harness compares `UnicodeGrapheme` and `LegacyScalar` contracts.

Representative results:

| sample | Unicode-core cells | legacy cells |
|---|---:|---:|
| combining `e + mark` | 1 | 1 |
| heart + VS16 | 2 | 1 |
| emoji ZWJ | 2 | 4 |
| family emoji | 2 | 8 |
| regional flag | 2 | 2 |

This validates the need for an explicit runtime compatibility boundary rather than silently choosing one behavior for all applications. ADR-011 uses DEC private mode 2027 for that boundary, enabled by default and queryable/changeable.

## Width and mutation evidence

The spike demonstrates:

- VS16 can widen an active grapheme from one to two cells;
- VS15 can narrow only while the same grapheme remains active;
- late width mutation after anchor invalidation must not rewrite unrelated committed content;
- mode/policy changes are semantic boundaries and invalidate active append state;
- overwriting either half of a wide unit must remove the full lead+continuation pair;
- a Unicode-core late widen at the right edge can atomically re-place the active grapheme on the next row under autowrap instead of splitting the unit.

The exact DECAWM-disabled wide-at-final-column compatibility behavior is intentionally left as an explicit #672 fixture. Issue #684 explicitly permits stopping with named unknowns rather than inventing temporary production architecture.

## Pathological-cluster evidence

`base + 4096 combining marks` remained one extended grapheme and consumed 8,193 UTF-8 bytes in the spike.

Naively rebuilding/resegmenting the entire active grapheme on every incoming scalar showed rapidly increasing work and is rejected for the production hot path.

The bounded-overflow probe used a 256-byte experimental bound against the 8,193-byte cluster and demonstrated the required failure family:

- retained variable payload becomes unavailable at the bound;
- already committed terminal occupation remains stable;
- one overflow condition is observable;
- copy/selection exposes U+FFFD for unavailable exceptional text;
- the next grapheme resumes normal behavior.

The 256-byte number is **not** a production default. Production limits require #673/resource calibration.

## Projection evidence

A fixed 16-byte experimental projected-cell record plus batch-local sidecar successfully represented:

- inline single-scalar text;
- multi-scalar grapheme payloads;
- explicit continuation cells with no independent text payload.

One sample used 9 physical cells, 144 bytes of fixed records and 51 bytes of sidecar payload. This proves that M002 does not need a heap/string-like variable record for every projected cell.

It does **not** freeze binary field packing. ADR-011 requires a versioned projection transition and SPEC-011 freezes the logical validation contract.

## ARM64 macOS shaping evidence

The final macOS lane compiled and ran successfully on an ARM64 runner. Complete-cluster CoreText shaping selected platform fallback fonts for CJK, emoji, Tamil, Arabic and supplementary-plane samples.

Observed terminal occupation and typographic advances differed, which directly proves that font metrics cannot be terminal-width authority.

Representative uncached shaping versus cached lookup:

| sample | shape ns/op | cache ns/op |
|---|---:|---:|
| ASCII | 6,259 | 209 |
| combining | 8,515 | 351 |
| CJK wide | 11,282 | 183 |
| emoji VS16 | 15,917 | 197 |
| emoji ZWJ | 18,215 | 217 |
| family emoji | 27,540 | 243 |
| Tamil combining | 20,304 | 192 |
| Arabic combining | 21,446 | 234 |

These are CI-host comparative measurements, not release latency budgets. Their purpose is architectural: shaping belongs behind a bounded renderer cache and outside synchronous terminal-state mutation.

## Rejected architecture families

The evidence rejects:

- one owned string per terminal cell;
- tiny fixed-only scalar storage;
- append-only global text storage;
- full active-grapheme reconstruction/resegmentation on every scalar;
- CoreText/AppKit as terminal width authority;
- synchronous complete-cluster shaping in `TerminalState`;
- client-side reconstruction of semantic graphemes from scalar-only projection;
- persistence of IME preedit as terminal state.

## Selected architecture family

The selected family is:

```text
PTY bytes
 -> bounded incremental UTF-8 decode
 -> TerminalState-owned Unicode/legacy mutation mode
 -> compact lead cell + reclaimable grapheme payload
 -> explicit continuation occupation
 -> canonical history grapheme units (#685)
 -> versioned derived projection + bounded sidecar
 -> renderer-owned shaping/fallback/cache

AppKit IME marked text
 -> ephemeral native state only
 -> committed UTF-8 enters normal input path
```

## Explicit remaining production refinements

These do not keep the spike open:

- select and record the first production Unicode-data version;
- select numeric active-grapheme and aggregate text-storage caps from production measurements;
- settle DECAWM-disabled wide-at-final-column behavior in #672 retained fixtures;
- freeze exact Candidate-D vNext field packing/capability number against the current protocol implementation;
- run production-path Unicode-heavy latency/RSS gates under #673.

They are implementation/refinement values within the accepted ownership model, except that evidence requiring a different ownership model would reopen ADR-011.

## Spike closure rule

PR #794 remains non-mergeable and should be closed without merge after ADR-011/SPEC-011 evidence is accepted. No experimental code under `spikes/m002-unicode-684` graduates wholesale into production.
