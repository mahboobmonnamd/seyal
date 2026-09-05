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
Runtime/PTY path. M001 already contains production qualification coverage for
dead-key composition, IME cancel/abandon, replacement commit and candidate
rectangle validity; M002 should reuse that seam rather than inventing a second
text authority.

## Candidates measured here

The harness compares representation and semantic pressure, not production code:

1. **Owned cluster per cell** — simple but heap-heavy (`String` per cluster).
2. **Inline scalar cluster** — allocation-free for short clusters but inflates
   every cell and still needs overflow handling.
3. **State-owned byte arena + compact cell reference** — keeps ordinary cells
   compact and moves variable-length grapheme bytes to state-owned storage.
   The prototype arena is deliberately simple; a production design must prove
   bounded reclamation/lifetime semantics and must not be append-only.
4. **Fixed-size projection cell + batch-local grapheme sidecar** — a wire
   pressure model showing that Candidate-D can remain fixed-width for ordinary
   cells while multi-scalar grapheme bytes travel in a bounded derived sidecar.
   This is not yet an accepted wire schema.

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
- an ambiguous-width character;
- pathological combining-mark storms used to expose cluster-size and
  incremental-processing pressure.

It reports extended-grapheme segmentation and both ordinary/CJK width results
so policy differences remain explicit.

## Measurements and semantic probes

- Rust payload `size_of` for each candidate cell representation;
- scalar/UTF-8 length, grapheme count and width for each corpus item;
- segmentation and width-classification throughput;
- construction/update proxy timing for the three state representation candidates;
- inline-overflow frequency for clusters longer than four scalars;
- arbitrary PTY chunk boundaries and malformed/truncated UTF-8 recovery;
- separation of printable scalar events from control events;
- incremental grapheme growth versus legacy scalar cursor behavior;
- VS16 late width growth at the right margin, including a mode-2027/autowrap
  reflow hypothesis;
- cursor/control mutation boundaries and wide-cell lead/continuation erasure;
- overwrite/reclamation pressure for variable-length storage;
- fixed 16-byte projection-record pressure with a multi-scalar sidecar;
- pathological active-cluster growth and the cost shape of naively resegmenting
  the entire growing cluster on every scalar.

These timings are comparative spike evidence only. They are not Seyal
key-to-photon or production PTY/VT benchmarks.

## Evidence checkpoint

The CI-backed evidence now establishes the following without freezing the
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
  when the first scalar is already in the final column, the spike can either
  expose the conflict or model whole-cluster reflow under DECAWM. The latter is
  aligned with the Terminal Unicode Core mode-2027 draft but is not yet Seyal's
  accepted wrap-state implementation;
- VS15/narrowing must not silently move already-placed following cells
  backwards under the monotonic-width hypothesis;
- ambiguous-width policy is observable (`¡` is width 1 under the ordinary
  table and width 2 under the CJK table), so width policy must be explicit;
- arbitrary PTY read boundaries are semantically invisible to UTF-8 decoding:
  a four-byte scalar and a multi-scalar ZWJ grapheme survive byte-at-a-time
  feeds without buffering the PTY stream as text;
- malformed UTF-8 can emit the replacement scalar and reprocess the following
  valid byte, preserving the M001 parser's current recovery shape;
- controls remain distinct events from printable Unicode scalars, which means
  the terminal mutation layer — not the decoder — must define whether a given
  control ends active-cluster append eligibility;
- positional cursor movement must end append eligibility; overwriting either
  the lead or continuation half of a width-2 grapheme must erase the entire
  prior grapheme rather than leave an orphan continuation;
- a spike projection record can remain 16 bytes while directly encoding
  single-scalar cells and placing only multi-scalar grapheme payloads in a
  batch-local sidecar. This proves per-cell wire inflation is not mandatory,
  but the exact versioned Candidate-D schema remains undecided;
- a base scalar followed by 4,096 combining marks is still one extended
  grapheme and exceeds 8 KiB of UTF-8. Unicode therefore does not provide a
  small fixed maximum grapheme size. Production must have an explicit resource
  policy and must not rely on a tiny inline upper bound;
- repeatedly copying/resegmenting the entire active grapheme on each incoming
  scalar has pathological growth pressure, so that spike implementation style
  is rejected for the production hot path;
- the deterministic suite is now broader than the initial 12-test checkpoint
  and covers transport, mutation, projection and pathological-bound behavior in
  addition to representation/storage/streaming semantics.

### Candidate status after this checkpoint

**Rejected as the direct production design:**

- one owned `String` per terminal cell;
- a fixed four-scalar-only cell representation;
- an append-only global grapheme byte arena;
- a production algorithm that reconstructs and fully resegments/re-measures the
  complete active grapheme for every appended scalar.

**Still viable as a state design family:**

A compact lead-cell/reference representation backed by **bounded, reclaimable,
TerminalState-owned variable-length text storage**, with explicit continuation
cells for width-2 occupation. The exact ownership unit, allocator/layout and
resource-limit failure behavior remain intentionally undecided until #685 adds
scrollback/reflow/history memory-slope evidence.

**Still viable as a projection design family:**

A versioned fixed-size derived display-cell record that keeps direct
single-scalar text inline and references batch-local variable-length payloads
for multi-scalar graphemes. Continuation cells carry no independent text
authority. The client remains a disposable cache; TerminalState remains the
only semantic authority.

## Run

```sh
cargo test --manifest-path spikes/m002-unicode-684/Cargo.toml
cargo run --release --manifest-path spikes/m002-unicode-684/Cargo.toml
```

## Promotion gate

This branch is not a merge candidate. Before production implementation:

1. freeze the released Unicode-version policy, ambiguous-width default and
   mode-2027/legacy compatibility contract;
2. settle control boundaries, active-cluster mutation, DECAWM/right-edge late
   widening and resource-limit failure behavior against retained VT fixtures;
3. combine the lead/continuation model with #685 resize/reflow, logical-line,
   selection/search and bounded-history experiments;
4. validate the existing production macOS IME seam on the accepted M002 model,
   keeping preedit outside TerminalState and retaining commit/cancel/candidate
   coordinate qualification;
5. choose the versioned projection/wire schema for full grapheme text + wide
   continuation without making the client cache authoritative;
6. measure shaping/font-fallback/cache behavior on the macOS Metal path without
   moving width authority into CoreText/AppKit;
7. land the accepted ADR/spec in a separate mergeable architecture PR;
8. implement clean production code under the promoted M002 implementation
   issue with TDD, fuzzing and performance gates.

Refs #684
