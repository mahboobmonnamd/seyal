# M002 Unicode / grapheme / width / IME spike — Issue #684

> **NON-MERGEABLE R&D EVIDENCE.** This directory exists only on the isolated
> `spike/m002-unicode-grapheme-width-ime-684` branch. Do not promote this code
> wholesale into `master`.

## Question

Choose the permanent Unicode text model that lets the authoritative
`TerminalState` own grapheme identity and terminal-cell width while Metal owns
only shaping/font fallback and AppKit owns only ephemeral IME preedit state.

The spike must not create another terminal/grid authority.

## Current production pressure points

M001 intentionally stores one Rust `char` per terminal cell, advances one cell
per printed scalar, projects one scalar per cell, and serializes one scalar in
the Candidate-D display cell. That is insufficient for combining sequences,
wide characters and multi-scalar emoji/grapheme clusters.

The existing AppKit `NSTextInputClient` seam is intentionally retained:
marked/preedit text is ephemeral client state; only committed UTF-8 enters the
Runtime/PTY path.

## Candidates measured here

The first harness compares representation pressure, not production code:

1. **Owned cluster per cell** — simple but heap-heavy (`String` per cluster).
2. **Inline scalar cluster** — allocation-free for short clusters but inflates
   every cell and still needs overflow handling.
3. **State-owned byte arena + compact cell reference** — keeps ordinary cells
   compact and moves variable-length grapheme bytes to state-owned storage.
   The prototype arena is deliberately simple; a production design must prove
   bounded reclamation/lifetime semantics and must not be append-only.

The M001 single-scalar cell is reported only as a size/performance baseline; it
is not a viable M002 Unicode design.

## Corpus

The harness includes:

- ASCII;
- combining marks;
- East Asian wide characters;
- emoji presentation selectors;
- emoji modifiers and ZWJ sequences;
- regional-indicator flags;
- keycap sequences;
- Tamil and Arabic combining examples;
- supplementary-plane scalars;
- an isolated combining mark;
- an ambiguous-width character.

It reports extended-grapheme segmentation and both ordinary/CJK width results
so policy differences remain explicit.

## Measurements in this first cut

- Rust payload `size_of` for each candidate cell representation;
- scalar/UTF-8 length, grapheme count and width for each corpus item;
- segmentation and width-classification throughput;
- construction/update proxy timing for the three candidate representations;
- inline-overflow frequency for clusters longer than four scalars;
- incremental grapheme growth versus legacy scalar cursor behavior;
- late width growth at the right margin;
- overwrite/reclamation pressure for variable-length storage.

These timings are comparative spike evidence only. They are not Seyal
key-to-photon or production PTY/VT benchmarks.

## Evidence checkpoint

The first CI-backed evidence establishes the following without freezing the
production architecture:

- the measured text payload is 32 bytes for a per-cell owned `String`, 20 bytes
  for four inline Rust `char`s, and 8 bytes for the compact arena reference;
- the family emoji `👨‍👩‍👧‍👦` is seven scalars, so a fixed four-scalar inline
  representation already requires an overflow path for an ordinary sequence;
- a deliberately append-only byte arena retained 800,000 payload bytes after
  100,000 overwrites while only 25 bytes were live, proving that compact cell
  references alone do not solve bounded-memory ownership;
- scalar-by-scalar cursor accounting produces width 4 for `👩‍💻` and width 8
  for the family emoji in this corpus, while the grapheme hypothesis occupies
  width 2 for each;
- `❤` followed by VS16 demonstrates late width growth from one to two cells;
  when the first scalar is already in the final column, the spike detects an
  unresolved right-edge conflict instead of inventing wrap semantics;
- ambiguous-width policy is observable (`¡` is width 1 under the ordinary
  table and width 2 under the CJK table), so width policy must be explicit;
- the expanded deterministic suite contains 12 tests covering representation,
  storage and incremental grapheme behavior.

### Candidate status after this checkpoint

**Rejected as the direct production design:**

- one owned `String` per terminal cell;
- a fixed four-scalar-only cell representation;
- an append-only global grapheme byte arena.

**Still viable as a design family:**

A compact cell/reference representation backed by **bounded, reclaimable,
state-owned variable-length text storage**. The exact ownership unit and
allocator/layout remain intentionally undecided until overwrite, scrollback,
reflow and projection measurements are combined with #685.

## Run

```sh
cargo test --manifest-path spikes/m002-unicode-684/Cargo.toml
cargo run --release --manifest-path spikes/m002-unicode-684/Cargo.toml
```

## Promotion gate

This branch is not a merge candidate. Before production implementation:

1. close remaining semantic questions (released Unicode-version pinning,
   ambiguous width, mode-2027/legacy compatibility, cluster mutation and
   right-edge late-width behavior);
2. measure representative real terminal feeds and resize/reflow interactions;
3. validate the macOS IME commit/cancel/coordinate seam without moving preedit
   into terminal authority;
4. choose projection/wire semantics for multi-scalar clusters and wide-cell
   continuation without making the client cache authoritative;
5. combine bounded text-storage measurements with #685 scrollback/reflow
   ownership and memory-slope evidence;
6. land the accepted ADR/spec in a separate mergeable architecture PR;
7. implement clean production code under the promoted M002 implementation
   issue with TDD, fuzzing and performance gates.

Refs #684
