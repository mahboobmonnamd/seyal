# Independent closing re-review — Seyal OSS #819 HistoryStore / reflow

## Verdict

**NO-GO for closing #819 at `c3e9a861052102de898df2af4df2615f20227513`.**
Do not use `Closes #819`, `Fixes #819`, or `Resolves #819`.

Period-2 wrap occupancy now consumes stored RLE counts without expanding them
into a per-unit width vector, and the four b2014a2 P1s are narrowed: suffix
fragments are no longer `Vec::retain`'d, wrap bytes are not charged as resident
source, and `Vec::remove(0)` is gone. The eager-resize commit path still walks
the uncompacted retained run tail, aperiodic mixed occupancy is still linear in
runs, and the wrap index is still not a droppable 4 MiB derived cache.

## Scope and verification

- Worktree: `/tmp/seyal-oss-work/issue-819-history-store`
- Branch: `issue/819`
- HEAD confirmed: `c3e9a861052102de898df2af4df2615f20227513`
- Subject confirmed: `fix(history): keep period-2 wrap occupancy on stored RLE counts`
- Comparison: `origin/master...HEAD`
- Worktree status after review/test: clean (`## issue/819...origin/issue/819`)
- `git diff --check origin/master...HEAD`: passed (no output).
- `rustup run 1.98.0-x86_64-unknown-linux-gnu cargo test -p seyal-terminal --locked --lib --manifest-path /tmp/seyal-oss-work/issue-819-history-store/Cargo.toml -- history::tests`: passed, 14 tests.
- `rustup run 1.98.0-x86_64-unknown-linux-gnu cargo test -p seyal-terminal --locked --test history_store_regressions --manifest-path /tmp/seyal-oss-work/issue-819-history-store/Cargo.toml`: passed, 20 tests.

Linux x86_64 only. No headed, Swift/FFI, Metal, ARM64 macOS, or physical-host
performance execution is claimed.

## Predecessor P1 disposition at b2014a2

| b2014a2 P1 | At `c3e9a86` |
| --- | --- |
| Suffix trim scanned retained fragments (`Vec::retain`) | **Addressed for fragments.** `trim_wrap_suffix_from` pops trailing chains, then `partition_point` + `truncate`. |
| Mixed occupancy walked every preceding run | **Addressed only for exact period-2 RLE.** `wrap_column_before` uses `wrap_occupancy_repeating(&chain.runs[..2], …)` when `pattern_len == 2`. This SHA stops forcing count 1 and no longer expands those two runs into a width `Vec`. Aperiodic mixed chains still call `wrap_occupancy_runs`. |
| Wrap index charged as resident; `Vec::remove(0)` quadratic | **Resident charging and quadratic remove addressed.** Wrap bytes are in `derived_cache_bytes` / `wrap_index_allocated_bytes`, not `resident_bytes`. Prefix trim uses `pop_front` / `drain`. The 4 MiB derived cap still does not drop an open chain, and Runtime “drop derived” does not drop wrap. |
| (this SHA) period-2 occupancy expands stored counts | **Addressed for the occupancy walk.** Repeating occupancy applies `wrap_occupancy_run` to stored counts. The stored run vector is still the full uncompacted RLE, not two compacted runs. |

## Findings

### P1 — eager-resize commit still scans the retained uncompacted run tail

`Screen::commit_prepared` still calls `HistoryStore::truncate_from` before
swapping the prepared active surface
(`crates/seyal-terminal/src/screen.rs:584-617`). That trim is
`trim_wrap_suffix_from` (`history.rs:637-687`).

Fragment work is now bounded: trailing chains are `pop_back`'d, then a binary
search + `truncate`. For any chain with `pattern_len != 2` or `runs.len() > 2`
— including the normal repeating period-2 case, where `(1,2),(2,1)` is stored
once per period rather than compacted — the function then:

1. `drop_suffix_wrap_runs` from the end (`history.rs:1483-1496`);
2. `refresh_pattern_len` (`history.rs:1468-1480`), which walks **every remaining
   run** to re-check `runs[i] == runs[i % 2]`.

`commit_prepared` then re-appends `history_additions`. Each
`extend_wrap_line` also calls `refresh_pattern_len` (`history.rs:632`). A
SoftWrap chain that spans retained history therefore still does work
proportional to retained run count on the eager-resize path before the active
grid is installed. ADR-010 §7 and SPEC-010 §7 prohibit that unbounded work, not
only payload copies or fragment scans.

The new period-2 occupancy test
(`wrap_column_before_preserves_period_two_run_counts`) checks the carry column
against an expanded width slice. It does not assert that trim/append avoid
the retained run scan.

### P1 — wrap index is counted as derived but is not a 4 MiB droppable cache

`derived_cache_bytes` now includes wrap (`history.rs:480-486`).
`update_resident_bytes` does not (`history.rs:1040-1048`). Closed hard-broken
rows are omitted from the index (`extend_wrap_line` `history.rs:592-604`).
Those b2014a2 defects are corrected.

SPEC-010 §9.1 still requires a 4 MiB per-execution derived cap, with derived
eviction dropping indexes without deleting canonical payload. The remaining
holes:

- `enforce_wrap_index_cap` (`history.rs:741-750`) only `pop_front`s **closed**
  chains, and only while `wrap_chains.len() > 1`. A single open SoftWrap chain
  can grow without that cap.
- Period-2 detection does not compact storage. Occupancy reads `runs[..2]`,
  but the Vec still holds one RLE entry per width change.
- `drop_derived_cache` (`history.rs:489-491`) drops only `reflow_cache`.
  Runtime aggregate enforcement (`crates/seyal-runtime/src/runtime/mod.rs:217-241`)
  reads `derived_history_cache_bytes()` (wrap + reflow), calls
  `drop_derived_history_cache`, then subtracts the pre-drop total even when wrap
  remains. Wrap-index pressure is accounted as freed and never actually dropped.

The blank-row test stays under the derived cap because those rows are omitted
from wrap. There is still no test that a long open mixed chain stays inside
4 MiB, that wrap can be rebuilt after drop, or that canonical payload survives
wrap-index pressure without relying on the 32 MiB resident cap.

### P1 — aperiodic mixed occupancy is still linear in stored runs

`wrap_column_before` (`history.rs:561-577`) is on the eager-suffix path
(`eager_resize_suffix` `history.rs:550-555`). Closed-form occupancy applies
only when `pattern_len == 2`. `refresh_pattern_len` sets that only for a
strict 2-cycle of `(width, count)` pairs. Period-3 or irregular mixed-width
text (`pattern_len == 0`) still uses `wrap_occupancy_runs`
(`history.rs:1571-1583`), which iterates every preceding run. Each homogeneous
run is O(columns) via `wrap_occupancy_run`, so many count-1 runs remain
proportional to preceding units.

SPEC-010 §7 is not limited to period-2. Hostile long mixed lines are in the
#819 acceptance set. The period-2 count fix at this SHA does not bound that
path.

### P2 — compact 2-run suffix skip can leave stale run counts

When `pattern_len == 2 && runs.len() <= 2` and `keep_units > 0`,
`trim_wrap_suffix_from` does not call `drop_suffix_wrap_runs`
(`history.rs:677-685`). Occupancy for the current prepare still uses
`units_before` as the limit, so the first carry column can stay correct. After
`truncate_from`, `commit_prepared` re-appends the retained suffix onto that
open chain. If the cut is inside the first run, the stale tail run prevents
merging and `pattern_len` falls to 0. Later resizes then take the linear
`wrap_occupancy_runs` path on inflated counts. Canonical payload is not
rewritten; derived carry columns can be.

### P2 — source Metal ordering is still unproven for history-wide glyphs

Source inspection still shows history encoding backgrounds before glyphs
(`MetalTerminalRenderer.swift:1320-1342`) and continuation cells receiving no
glyph/wide-glyph flags (`MetalTerminalRenderer.swift:736-765`). That matches
SPEC-011 §7.2 / §12.1 for the identified continuation-overpaint defect.

There is still no direct history wide-lead-plus-continuation offscreen
regression. `wideGraphemeOffscreenSelfTest` exercises a live
`NativePreparedFrame` (`RendererValidation.swift:340-393`); the history
comparison uses a width-one `A` (`RendererValidation.swift:395-472`). This is
the expected remaining P2.

## Closing-gate disposition

The period-2 occupancy claim at this SHA is true for the carry-column walk:
stored counts are used, and that walk is O(columns) rather than O(units). It
does not make #819's bounded-reflow or derived-index Done gates true. Remaining
P1s on the eager-resize commit path and wrap-index budget fail those gates.

Do not close #819. Use a non-closing relationship (`Refs #819` / `Part of
#819`) if this SHA is merged as a follow-up.

This Linux x86_64 review makes no headed, Swift/FFI, Metal, ARM64 macOS, or
physical-performance execution claim; those issue-required evidence gates are
not established here.
