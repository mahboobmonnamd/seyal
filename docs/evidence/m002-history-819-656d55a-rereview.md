# Independent closing re-review — Seyal OSS #819 HistoryStore / reflow

## Verdict

**NO-GO for closing #819. Do not use `Closes #819` at
`656d55a23ae2a850d05ed85ba1f5684fc3952ed3`.**

The predecessor's two identified P1s are addressed in source: the history
Metal path now paints all cell backgrounds before glyphs, and the uniform
SoftWrap occupancy calculation no longer replays every unit.  However, this
candidate has a new/remaining **P0**: an arbitrarily long stream of blank
history rows is retained forever in the mutable tail and can exceed both
resident-history caps.  It also has two independent **P1** resize-path
violations: the newly added occupancy index still walks every predecessor
fragment (and can be worse for mixed-width runs), and every relevant primary
resize rebuilds that index by cloning all retained history before the active
surface is committed.

No macOS, Swift/FFI, Metal, headed, ARM64, or physical-performance execution is
claimed from this Linux x86_64 review host.

## Scope and review integrity

| Item | Value |
| --- | --- |
| Candidate worktree | `/tmp/seyal-oss-work/issue-819-history-store` |
| Candidate branch | `issue/819` |
| Exact reviewed commit | `656d55a23ae2a850d05ed85ba1f5684fc3952ed3` |
| Subject | `fix(history): index SoftWrap occupancy and two-pass history glyphs` |
| Comparison | `origin/master...HEAD` |
| Host | Linux 6.12.94+ x86_64 |
| Source authority reviewed | `AGENTS.md`; [#819](https://github.com/mahboobmonnamd/seyal/issues/819); ADR-010; SPEC-010 §7; SPEC-011 §7.2 and §12.1; predecessor `24a13c5` NO-GO |

The committed object was reviewed directly with `git show HEAD:<path>`. This
matters because the candidate checkout is not clean: `git status --short`
reports one local modification, `M crates/seyal-terminal/src/history.rs`.
`git diff HEAD -- crates/seyal-terminal/src/history.rs` shows only a
non-semantic formatting refactor of the `size_of::<(u8, u32)>()` expression in
`wrap_index_allocated_bytes`; `git diff --check HEAD` passes. I did not change,
stage, revert, or otherwise modify that worktree file. HEAD remains at the
required SHA, but the worktree cannot truthfully be reported as clean.

The complete committed delta against `origin/master` covers 55 files
(7,377 insertions, 193 deletions), including the terminal/history,
protocol/runtime/FFI, client, Metal/Swift, tests, benchmark harness, fuzz seed,
and evidence paths. This review inspected the remaining production delta
through the terminal, protocol/runtime/FFI, and Metal seams in addition to the
new commit's four-file delta from `24a13c5`.

## Findings

### P0 — blank retained rows defeat both resident-history byte caps

`HistoryLine::from_cells` produces a zero-unit line for an entirely blank
scrolled row (`crates/seyal-terminal/src/history.rs:114-149`). That path is
reachable in normal terminal operation: `Screen::append_row_to_history` sends a
row with no lead-cell fragments to `HistoryStore::append_row`
(`crates/seyal-terminal/src/screen.rs:1430-1435`).

For a zero-unit line, `append_line`/`push_fragment` appends tail metadata and
wrap-index metadata but never increases `tail_payload_bytes`
(`history.rs:750-818`). Sealing is conditional on the payload-byte target, so
the tail remains unsealed no matter how many blank rows arrive.
`HISTORY_TAIL_PAYLOAD_LIMIT` is defined but has no use site beyond its
re-export.

`update_resident_bytes` correctly counts tail and wrap-index allocations
(`history.rs:895-905`), so the counter will eventually exceed the 32 MiB
per-execution cap. `evict_to_cap` then attempts to pop only sealed segments; if
there are none, it breaks without reducing `resident_bytes`
(`history.rs:907-926`). Thus an all-blank history is unbounded despite the
counter and its explicit hard cap. The Runtime-wide implementation has the
same failure mode: it considers only executions with an oldest sealed segment
and breaks when no candidate exists (`crates/seyal-runtime/src/runtime/mod.rs:183-215`).

This violates SPEC-010 §5.1's mutable-tail policy and §6.1's hard resident
caps, both of which include metadata. It is also a hostile but legal terminal
output shape (repeated blank lines). No regression test exercises an empty-row
tail growing through either cap.

### P1 — the occupancy index is not closed-form for the claimed resident chain, and degrades on mixed widths

The new `WrapChain` stores both `fragments` and RLE `runs`
(`history.rs:230-265, 438-440, 577-603`). The uniform-width part of
`wrap_occupancy_runs` can cycle-skip within one run, but
`wrap_column_before` first calls `chain.units_before(from)`
(`history.rs:553-565`). `units_before` loops from the first fragment to
`from` (`history.rs:245-264`).

The new regression itself constructs 8,000 one-unit SoftWrap source records,
asserts that its chain contains 8,000 fragments, and queries an anchor at
record 7,952 (`history.rs:1626-1645`). Therefore the path still walks 7,952
predecessor fragments for precisely the claimed resident SoftWrap-chain case.
The test checks only the resulting column, not an iteration/allocation bound.
This is not O(columns) and does not meet SPEC-010 §7's rule that active-surface
progress must not wait for work proportional to all retained history.

The RLE representation also has a distinct mixed-width failure: alternating
width-1 ASCII and width-2 CJK/grapheme units make every unit its own run.
`wrap_occupancy_runs` visits each run, while `wrap_occupancy_run` allocates and
initializes a `cols + 1` `seen` vector for each one
(`history.rs:1334-1377`). Consequently a legal alternating-width chain is
O(number of predecessor units × columns) with repeated temporary allocation,
not the asserted closed-form/O(columns) behavior. No added test covers either
fragment-walk accounting or an alternating width-1/width-2 chain.

### P1 — primary resize still rebuilds the index by cloning all retained history

The new index's invalidation/rebuild path is itself an eager full-history
operation. `HistoryStore::rebuild_wrap_index` clears the index, materializes
`self.entries()` into `Vec<HistoryLine>`, cloning every retained UTF-8 payload
through `HistoryLineRef::to_owned_line`, then rebuilds from that snapshot
(`history.rs:384-398, 605-613`).

This is on the primary resize transaction. `Screen::prepare_resize` obtains
the bounded suffix and sets `replace_from` (`screen.rs:364-400`);
`Screen::commit_prepared` calls `history.truncate_from(from)` before committing
the active screen (`screen.rs:564-595`); and `truncate_from` unconditionally
calls `rebuild_wrap_index` after its tail or sealed-segment work
(`history.rs:635-719`). A resize that has retained history therefore performs
work/allocation proportional to all preserved history even when the source
suffix and reflow projection were bounded. This is directly contrary to
ADR-010 §7 and SPEC-010 §7.

The same full clone/rebuild is also performed after automatic per-execution
eviction and after every explicit oldest-segment eviction
(`history.rs:907-926, 933-946`). That adds a full retained-history allocation
spike to append/resource-pressure processing. The predecessor review's concern
about rebuild cost on truncate/evict remains unresolved.

### P2 — the fixed Metal ordering has no history-wide regression coverage

The source ordering replacement is correct on inspection:

* `applyHistoryPrepare` recognizes continuation cells, suppresses their glyph
  flags, and marks only a width-two lead as wide
  (`macos/Seyal/Sources/MetalTerminalRenderer.swift:731-780`).
* For every history region, the encoder submits mode 0 for all cell-sized
  backgrounds, then mode 1 for glyphs (`MetalTerminalRenderer.swift:1307-1342`).
* The shader enlarges a wide instance only when the mode is nonzero and discards
  instances without glyph/underline flags in the glyph pass
  (`TerminalShaders.metal:49-55, 79-112`). With the configured alpha blend
  state, a continuation has no opaque background draw in the second pass.

That fixes the predecessor's right-half-overpaint source flow and preserves the
SPEC-011 §7.2/§12.1 lead-plus-continuation geometry. I found no remaining P1 in
that two-pass ordering itself.

There is nevertheless no direct regression for it. The existing
`wideGraphemeOffscreenSelfTest` only prepares a live `NativePreparedFrame`
(`RendererValidation.swift:337-393`). The history test immediately following
it creates only a normal width-one `NativeHistoryRange.Cell` with `flags: 0`
and retains its former “history single-pass” wording
(`RendererValidation.swift:395-472`). It does not create a history wide lead
plus continuation or assert pixels in the continuation rectangle. This is P2
coverage debt, not a claim of a macOS execution result.

### P2 — the new hard-cap and resize blockers are unrepresented by the required tests

The focused test suite passes, but neither `history_store_regressions` nor the
new unit test creates blank-only history through the cap; drives a primary
`TerminalState` resize with thousands of retained rows and counts/limits
`rebuild_wrap_index`; or uses a mixed width RLE chain. Passing result-only
tests therefore do not validate the new P0/P1 resource and progress
requirements.

## Predecessor P1 audit (`24a13c5`)

| Predecessor finding | Disposition at `656d55a` | Evidence |
| --- | --- | --- |
| Carry-column path replayed every predecessor unit into temporary width vectors | **Partially addressed, P1 remains.** The per-unit width vectors are gone and a uniform run cycle-skips, but `units_before` walks every preceding fragment. Alternating widths also make one run per unit. | `history.rs:245-265, 553-565, 1334-1377, 1626-1645` |
| Sidecar overflow lost its current-row prefix and could make `Truncated` make no `start_unit` progress | **Addressed in the reviewed source.** The packing loop retains its already packed row prefix on sidecar failure; only a failure before the first cell omits the row. The existing protocol test covers that prefix. | `crates/seyal-protocol/src/pass7.rs:481-534, 1191-1214`; Runtime uses the packed prefix before bounded admission in `history_blocks.rs:131-161` |
| History's single mode-2 pass let continuation background overpaint the wide lead glyph | **Addressed in source.** History now uses the same two-pass mode-0/mode-1 ordering as live rendering, and continuation instances are glyphless. | `MetalTerminalRenderer.swift:731-780, 1307-1342`; `TerminalShaders.metal:49-55, 79-112` |

The predecessor's two specifically reported P1s have therefore received the
intended localized source changes, but a closing review cannot accept them as
sufficient while the P0 and independent resize P1s above remain.

## Required verification

The candidate's default Cargo is `cargo 1.83.0
(5ffbef321 2024-10-29)`, which cannot parse edition 2024. The compatible
toolchain is `cargo 1.98.0 (797e8a9bc 2026-08-05)`. I ran every requested
command exactly; the three Cargo commands failed identically before building
tests, so I reran their exact package/test selections with `rustup run
1.98.0-x86_64-unknown-linux-gnu cargo`.

```text
git -C /tmp/seyal-oss-work/issue-819-history-store rev-parse HEAD
656d55a23ae2a850d05ed85ba1f5684fc3952ed3

git -C /tmp/seyal-oss-work/issue-819-history-store log -1 --oneline
656d55a fix(history): index SoftWrap occupancy and two-pass history glyphs

git -C /tmp/seyal-oss-work/issue-819-history-store diff --check origin/master...HEAD
exit 0; no output

cargo test -p seyal-protocol --locked --manifest-path /tmp/seyal-oss-work/issue-819-history-store/Cargo.toml
exit 101: manifest requires Cargo feature `edition2024`, not stabilized in Cargo 1.83.0

cargo test -p seyal-terminal --locked --lib --manifest-path /tmp/seyal-oss-work/issue-819-history-store/Cargo.toml
exit 101: manifest requires Cargo feature `edition2024`, not stabilized in Cargo 1.83.0

cargo test -p seyal-terminal --locked --test history_store_regressions --manifest-path /tmp/seyal-oss-work/issue-819-history-store/Cargo.toml
exit 101: manifest requires Cargo feature `edition2024`, not stabilized in Cargo 1.83.0

rustup run 1.98.0-x86_64-unknown-linux-gnu cargo test -p seyal-protocol --locked --manifest-path /tmp/seyal-oss-work/issue-819-history-store/Cargo.toml
exit 0: 28 protocol unit tests and 6 integration tests passed; 2 fuzz-smoke tests ignored; doc-tests passed

rustup run 1.98.0-x86_64-unknown-linux-gnu cargo test -p seyal-terminal --locked --lib --manifest-path /tmp/seyal-oss-work/issue-819-history-store/Cargo.toml
exit 0: 41 library tests passed

rustup run 1.98.0-x86_64-unknown-linux-gnu cargo test -p seyal-terminal --locked --test history_store_regressions --manifest-path /tmp/seyal-oss-work/issue-819-history-store/Cargo.toml
exit 0: 20 history-store regression tests passed
```

The compatible-toolchain test runs used the candidate checkout, whose only
uncommitted source difference is the formatting-only hunk documented above;
the exact committed objects, not that local hunk, were the basis for source
findings. The requested focused tests do not cover the P0/P1 paths above.

## Done-gate disposition

| #819 acceptance/Done gate | Status at the exact reviewed SHA |
| --- | --- |
| One canonical `TerminalState`/history authority | Source-review pass; no second mutable history authority identified |
| Hard resident caps and deterministic eviction | **Fail — P0** blank-only tail is neither sealed nor evicted |
| Bounded/lazy reflow before active-surface progress | **Fail — P1** fragment walk and full `rebuild_wrap_index` execute on the resize path |
| Width-two retained-history source draw ordering | Source P1 addressed; dedicated history wide regression missing (P2) |
| Required focused Linux Rust checks | Pass with Cargo 1.98.0 after default Cargo 1.83.0 could not parse the manifest |
| `make check` / Foundation gates at this exact SHA | Not run in this review |
| Required fuzz/property coverage of hostile long lines, resize, eviction, Unicode units | Not established for the new P0/P1 paths; the tracked evidence does not constitute a passing exact-`656d55a` campaign for them |
| Controlled ARM64 macOS Release latency/RSS/resource matrix | Unmet here; existing evidence explicitly remains comparative/incomplete and names earlier heads |
| Five-step headed history/reflow manual verification | Unmet here; Linux x86_64 cannot run `Seyal.app` or observe Metal pixels |
| macOS FFI/Swift/Metal execution | Unverified on this host |

## Closing decision

**NO-GO — leave #819 open and do not use a closing relationship.**

Before a closing review can pass, the implementation must enforce the resident
cap for zero-payload/metadata-only history, make the resize path bounded without
fragment scans or full-history index rebuilding, and cover mixed-width chains.
The history Metal two-pass fix should receive a history-wide offscreen
regression on a macOS-capable host. The resulting exact SHA then still needs
the outstanding issue-defined validation and evidence gates; this Linux review
does not supply native, headed, ARM64, or performance evidence.
