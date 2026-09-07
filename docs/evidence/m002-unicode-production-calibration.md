# M002 Unicode production calibration

- **Issue:** #815
- **Parent:** #672
- **Architecture:** ADR-011
- **Behavioral authority:** SPEC-011
- **Baseline:** `baa7878f78bc50b8b4afd7715275841541f0e007`
- **Purpose:** freeze implementation parameters deliberately left open by #684 / PR #805 before production Unicode work begins.

## Decision summary

| Parameter | Frozen M002 value |
| --- | --- |
| Unicode semantic data | Unicode **17.0.0** |
| Active canonical grapheme payload | **8,192 UTF-8 bytes** maximum |
| Live variable grapheme payload per `TerminalState` | **2 MiB (2,097,152 bytes)** maximum |
| Grapheme display capability | `CAP_GRAPHEME_DISPLAY = 1 << 6` |
| Grapheme display messages | `DisplaySnapshotV2 = 27`, `DisplayDeltaV2 = 28` |
| Grapheme display schema | `2` |
| Fixed physical-cell record | **16 bytes** |
| Grapheme sidecar per display chunk | **65,536 bytes** maximum |
| Existing display transport-batch maximum | remains **4 MiB** per presentation batch |
| V2 cell addressing | whole-row spans **or** contiguous partial-row spans via `first_col` |
| DECAWM reset + new width-2 unit at final column | unit is ignored atomically; cursor/grid unchanged |
| DECAWM reset + late widening of active width-1 unit at final column | reject only the width-changing extension; retain the already committed width-1 prefix and occupation |

These values are M002 production contracts. Changing them after implementation requires a specification review with compatibility/resource evidence; they are not opportunistic tuning knobs inside #816/#817.

Projection completeness: every legal Unicode-core `TerminalState` within the grapheme/live-store caps must remain losslessly representable by v2, using partial-row chunks and/or multiple 4 MiB transport batches when required. Unicode caps are not reduced to fit the wire.

## Unicode 17.0.0

Unicode 17.0.0 was released on 2025-09-09 and is the current Unicode Standard as of this calibration. The Unicode Consortium release page is the semantic source for UAX #29 grapheme breaking and UAX #11 East Asian Width. `unicode-segmentation` 1.13.2+ also records Unicode 17.0.0 support, which makes it a viable implementation input but not architecture authority.

Sources:

- https://www.unicode.org/releases/index.html
- https://www.unicode.org/versions/Unicode17.0.0/
- https://github.com/unicode-rs/unicode-segmentation/blob/master/README.md

Upgrade rule: a Unicode semantic-data bump must update the recorded version, regenerate the retained Unicode corpus, and produce a reviewable diff for changed grapheme boundaries/widths before release.

## Active grapheme cap: 8,192 bytes

The #684 isolated evidence found that `base + 4096 combining marks` remains one extended grapheme and consumes **8,193 UTF-8 bytes**. The production cap is therefore set to 8,192 bytes so that the retained pathological fixture crosses the hard bound deterministically by one byte rather than depending on allocator behavior.

This is intentionally far above ordinary graphemes but still gives a strict adversarial bound. Once the cap is crossed:

- the already committed cell occupation remains unchanged;
- no further bytes for that active grapheme are retained;
- the grapheme becomes the SPEC-011 overflow sentinel for copy/search/selection;
- exactly bounded counters may increase, but terminal contents are never logged;
- the next grapheme boundary resumes normal operation.

The counter family is fixed as aggregate non-content diagnostics per `TerminalState`:

- `grapheme_payload_overflow_count` — saturating `u64`;
- `grapheme_store_capacity_fallback_count` — saturating `u64`.

No per-grapheme diagnostic log or payload sample is permitted.

## Live variable payload cap: 2 MiB per TerminalState

Current production geometry is capped at 512×256 = 131,072 physical cells. A full maximum-size grid containing width-2 family emoji has at most 65,536 lead cells. The #684 representative family emoji payload is 25 UTF-8 bytes, or about 1,638,400 bytes if every width-2 lead carried that payload. A **2 MiB** live-payload ceiling covers that deliberately dense representative maximum while leaving bounded headroom.

The cap is measured over **live canonical variable payload bytes**, not allocator capacity and not scalar-inline cells. Dead payload must be reclaimable. The implementation may compact/recycle storage before taking the capacity fallback. It must not preallocate 2 MiB per execution.

If admitting/replacing a canonical grapheme would exceed the live cap after allowed reclamation, that grapheme uses the same overflow sentinel/fallback family without blocking PTY progress. The terminal must not evict unrelated live visible canonical text merely to make the new grapheme fit.

This cap covers active/grid variable text only. ADR-010/#818 separately owns retained scrollback/history resident-memory budgets; see [`m002-scrollback-production-calibration.md`](m002-scrollback-production-calibration.md).

## DECAWM-reset wide-at-final-column fixture

Cross-implementation evidence supports ignoring a width-2 glyph that cannot fit when wraparound is disabled:

- Termux retains a test named `testWideCharacterWithoutWrapping` whose expected behavior ignores a wide character at the final column with DECAWM reset: https://github.com/termux/termux-app/blob/3b66f8799635a4dba4a206563048ff0e6792c487/terminal-emulator/src/test/java/com/termux/terminal/UnicodeInputTest.java
- Windows Terminal's `_WriteToBuffer` explicitly identifies DECAWM disabled plus a wide glyph in the last column as a case that cannot be written and advances past/throws away that glyph: https://github.com/microsoft/terminal/blob/093e49e29a9f806ff83025c49be5d0c970673b00/src/terminal/adapter/adaptDispatch.cpp

Seyal therefore freezes these two related cases:

### New width-2 canonical unit

Given a cursor at the final column and DECAWM reset, a newly completed canonical unit whose terminal width is 2 is ignored atomically. The grid, cursor, pending-wrap state, hard/soft lineage and active-grapheme anchor are unchanged by that unit. No lead or continuation half is created.

### Late widening of an existing active width-1 unit

If an already committed active width-1 grapheme occupies the final column and a subsequent scalar would make that same grapheme width 2 while DECAWM is reset, Seyal rejects only the width-changing extension. The previously committed prefix, its width-1 occupation, cursor and lineage remain unchanged. The rejected extension must not be retained in the canonical payload. This prevents both retroactive deletion of already committed text and an impossible split wide unit.

A subsequent grapheme begins normally from the retained state. The deterministic #816 fixture must cover both cases.

## Candidate-D grapheme display v2

### Negotiation

- Existing M001 scalar display remains capability bit 0 and message types 12/13.
- M002 allocates `CAP_GRAPHEME_DISPLAY = 1 << 6`; bit 5 is already owned by Pass 8 Block metadata.
- M002 allocates R→C display message types `27` and `28` for snapshot/delta v2. Type 26 remains BlockState.
- The SPEC-004 envelope remains protocol major/minor 1.0. New semantics are negotiated by capability and distinct message IDs; no old 16-byte record is silently reinterpreted.
- A production M002 graphical client must advertise `CAP_GRAPHEME_DISPLAY`. Runtime must not send v2 to a peer lacking the bit.
- A peer without the capability may receive the legacy display only while the entire projected batch is losslessly representable by the M001 scalar schema. Otherwise Runtime must fail display attachment/resync with the existing bounded `DisplayUnavailable` family rather than lossy down-conversion.

### Chunk layout

A v2 display payload is:

```text
48-byte chunk header
N × 16-byte physical-cell records
sidecar_len bytes UTF-8 sidecar
```

The first 40 header bytes keep the existing Candidate-D fields in the same order. Bytes 40..48 are:

```text
40..44  sidecar_len : u32 little-endian
44..46  schema      : u16 little-endian = 2
46..48  first_col   : u16 little-endian
```

Validation requires:

```text
48 + cell_count * 16 + sidecar_len == payload_len
sidecar_len <= 65,536
payload_len <= MAX_FRAME_PAYLOAD (262,144)
first_col < columns
cell_count >= 1
all checked arithmetic before allocation/use
```

Cell-span rules:

- whole-row: `first_col == 0` and `cell_count == row_count * columns`;
- partial-row: `row_count == 1` and `first_col + cell_count <= columns`;
- lead/continuation pairs stay inside one chunk;
- grapheme sidecar bytes are never split across chunks.

The existing `MAX_DISPLAY_BATCH_BYTES = 4 MiB` remains the per-transport-batch ceiling. One logical generation update may span multiple transport batches; the client applies atomically only after all `chunk_count` chunks validate.

### Representability proofs retained by #815

#### One-row sidecar overflow without partial spans

Nine legal 8,192-byte width-1 graphemes require `9 × 8192 = 73,728` sidecar bytes, which exceeds the 65,536-byte per-chunk sidecar. Whole-row-only Candidate-D packing cannot split that row under the frozen 48-byte header. Partial-row spans with at most eight such graphemes per chunk (`8 × 8192 = 65,536`) close the gap without weakening Unicode caps.

#### Full-snapshot batch ceiling

Maximum fixed-cell payload is `512 × 256 × 16 = 2,097,152` bytes. Maximum live variable payload is also `2,097,152` bytes. Content alone equals the 4 MiB batch ceiling before frame/chunk overhead, so a legal max-geometry `TerminalState` near the live-store cap cannot always fit in one transport batch. Multi-batch logical-update assembly preserves the 4 MiB batch constant and the Unicode caps.

### 16-byte cell record

```text
0..4    text_ref : u32 little-endian
4..8    foreground : existing DisplayColor encoding
8..12   background : existing DisplayColor encoding
12..16  meta : u32 little-endian
```

`meta`:

```text
bits  0..2   bold / underline / inverse (existing meanings)
bits  3..4   role: 0 Empty, 1 Lead, 2 Continuation, 3 invalid
bits  5..6   terminal width: 0, 1 or 2; 3 invalid
bit      7   sidecar reference flag
bits  8..20  sidecar_len_minus_1 (13 bits; 1..8192 bytes when sidecar=1)
bits 21..31  reserved = 0
```

Role constraints:

- `Empty`: `text_ref=0`, width=0, sidecar=0, sidecar length field=0. Color/attribute metadata may still paint an empty cell.
- `Lead` width 1 or 2, inline scalar: sidecar=0, sidecar length field=0, `text_ref` is one valid Unicode scalar.
- `Lead` width 1 or 2, multi-scalar/variable payload: sidecar=1, `text_ref` is a byte offset from the start of this chunk's sidecar, and decoded length is `sidecar_len_minus_1 + 1` in 1..8192.
- `Continuation`: `text_ref=0`, width=0, sidecar=0, sidecar length field=0. It carries no independent text authority. Presentation colors/attributes are derived from its lead and must match the producer's lead styling.

The producer writes sidecar payloads in physical-cell order within each chunk span with no gaps or overlap. The decoder validates the same canonical ordering, UTF-8 validity, bounds, role/width consistency and continuation adjacency before exposing a chunk to the client cache.

### Atomicity and resync

A malformed v2 chunk or sidecar invalidates the complete logical update, including any multi-batch transport fragments. The client must not partially commit cells/sidecar and continue. It requests the existing bounded resync path. Client display state remains disposable; rebuilding from Runtime canonical state must reproduce the same roles/text/width/style.

## Performance evidence contract

#673 owns release-level ceilings. #816/#817 must still record narrow change attribution on the exact implementation head:

- scalar baseline vs Unicode-core feed throughput;
- Unicode-core mutation p50/p95/p99;
- overwrite and 8,193-byte combining-storm live-store behavior;
- projected bytes/cell and sidecar frequency for ASCII, combining, CJK and emoji workloads;
- renderer shaping/cache hit/miss cost on ARM64 macOS;
- Unicode-heavy 1/10/50/100 execution RSS/CPU attribution.

CI-host shaping numbers are comparative only and must never be labelled end-user key-to-photon latency.

## Manual calibration verification

1. Confirm the Unicode Consortium latest-release page still identifies 17.0.0 for this implementation baseline.
2. Re-run the retained `base + 4096 combining marks` probe and confirm its payload is 8,193 bytes; the selected 8,192-byte cap must cross exactly once and recover on the next grapheme.
3. Calculate maximum geometry `512 × 256 = 131,072` cells and representative width-2 family-emoji live payload `65,536 × 25 = 1,638,400` bytes; confirm it is below 2 MiB.
4. Run the DECAWM-reset fixture for both direct width-2-at-last-column and late-widen-at-last-column behavior.
5. Validate one inline v2 cell, one sidecar v2 cell, one partial-row multi-chunk row packing case (`9 × 8192` sidecar bytes), one multi-batch logical snapshot assembly case, and malformed role/offset/length/`first_col` cases against the schema above.
6. Confirm type 26 and capability bit 5 remain Block metadata; v2 uses type 27/28 and bit 6 without reinterpreting 12/13.
7. Confirm `512 × 256 × 16 + 2,097,152 = 4,194,304` reaches the 4 MiB batch ceiling before overhead, so multi-batch transport remains mandatory for adversarial max-geometry live-store pressure.

## Non-goals

This calibration does not implement the allocator, grapheme state machine, display encoder/decoder, Metal shaping, IME changes, HistoryStore or #673 release gates. Those remain separate production Issues/PRs under #672/#673.