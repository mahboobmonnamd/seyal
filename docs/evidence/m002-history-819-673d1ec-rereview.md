# Independent closing re-review — Seyal OSS #819 HistoryStore / reflow

## Verdict

**NO-GO for closing #819 at `673d1ec26e063426fa86f957c988075df56cba33`.**
Do not use `Closes #819`, `Fixes #819`, or `Resolves #819`.

**Portable source of this SHA: NO-GO.** The claimed retained-tail
`refresh_pattern_len` scan and the c3e9a86 P2 two-run suffix skip are gone, and
period-2 occupancy still consumes stored RLE counts without expanding a
per-unit width `Vec`. Those are not enough. Wrap-index 4 MiB droppability and
aperiodic mixed occupancy remain P1 source defects on SPEC-010 §7 / §9.1 and
ADR-010 §7, which #819's Done gates require.

Headed / native / IME / Metal / ARM64 macOS / physical-host performance /
`make check` are **ENVIRONMENT_UNSUPPORTED** on this Linux x86_64 review and
are independently unmet.

## Scope and verification

- Worktree: `/tmp/seyal-oss-work/issue-819-history-store`
- Branch: `issue/819`
- HEAD confirmed: `673d1ec26e063426fa86f957c988075df56cba33`
- Subject confirmed: `fix(history): trim wrap runs without scanning the retained tail`
- Parent: `145d672b5876805d22270147a611880f359287a4` (predecessor reviewed SHA
  `c3e9a861052102de898df2af4df2615f20227513` is an ancestor, not the parent)
- Comparison: `origin/master...HEAD`
- Worktree status after review/test: clean (`## issue/819...origin/issue/819`)
- `git diff --check origin/master...HEAD`: passed (no output).
- Production Rust/Swift tree contains no `refresh_pattern_len` symbol.
- `rustup run 1.98.0-x86_64-unknown-linux-gnu cargo test -p seyal-terminal --locked --lib --manifest-path /tmp/seyal-oss-work/issue-819-history-store/Cargo.toml -- history::tests`: passed, 15 tests (includes `wrap_column_before_preserves_period_two_run_counts` and `wrap_suffix_trim_drops_stale_two_run_tail_before_reappend`).
- `rustup run 1.98.0-x86_64-unknown-linux-gnu cargo test -p seyal-terminal --locked --test history_store_regressions --manifest-path /tmp/seyal-oss-work/issue-819-history-store/Cargo.toml`: passed, 20 tests.

Linux x86_64 only (`uname -m`: x86_64; `rustc 1.98.0 (88d9e12ae 2026-08-18)`).
No headed, Swift/FFI, Metal, ARM64 macOS, or physical-host performance
execution is claimed.

## Predecessor P1 disposition at c3e9a86

| c3e9a86 finding | At `673d1ec` |
| --- | --- |
| P1 eager-resize commit scans retained uncompacted run tail (`refresh_pattern_len` on suffix trim and append) | **Addressed.** `refresh_pattern_len` is removed. Suffix trim always `drop_suffix_wrap_runs` then `note_pattern_after_suffix_trim` (O(1) last/template). Append uses `note_appended_wrap_run` (O(1) last/template). Drop work is from the end, proportional to discarded suffix runs, not the retained tail. |
| P2 `pattern_len == 2 && runs.len() <= 2` skip of suffix drop | **Addressed.** `trim_wrap_suffix_from` always calls `drop_suffix_wrap_runs`. Test `wrap_suffix_trim_drops_stale_two_run_tail_before_reappend` cuts inside the first of two runs and re-appends. |
| P1 wrap index counted as derived but not a 4 MiB droppable cache | **Remains.** Open-chain cap hole and `drop_derived_cache` still ignore wrap bytes. |
| P1 aperiodic mixed occupancy linear in stored runs | **Remains.** Closed-form path is still `pattern_len == 2` only. |
| Period-2 occupancy uses stored RLE counts (c3e9a86 claim) | **Still true.** `wrap_column_before` calls `wrap_occupancy_repeating(&chain.runs[..2], …)`. No per-unit width `Vec` on that path. |
| P2 Metal history wide-lead + continuation ordering unproven by headed evidence | **Remains as source inspection only.** ENVIRONMENT_UNSUPPORTED here. |

## What this SHA actually changes

`Screen::commit_prepared` still trims then re-appends before swapping the
prepared grid (`crates/seyal-terminal/src/screen.rs:584-613`):

```584:613:crates/seyal-terminal/src/screen.rs
        if let Some(from) = prepared.replace_from {
            self.history.truncate_from(from);
            for line in prepared.history_additions {
                self.history.append_line(line);
            }
        } else if prepared.replace_history {
            ...
        }
        ...
        self.cols = prepared.cols;
        self.rows = prepared.rows;
        self.cells = prepared.cells;
```

On that path, `truncate_from` still reaches `trim_wrap_suffix_from`
(`history.rs:639-682`). The c3e9a86 branch that skipped suffix-run drops when
`pattern_len == 2 && runs.len() <= 2` is gone. The function always:

1. `pop_back`s wholly-after chains, then `partition_point` + `truncate` on
   fragments (unchanged, not a retained-run scan);
2. `drop_suffix_wrap_runs(&mut chain.runs, total.saturating_sub(keep_units))`
   (`history.rs:679`, implementation `history.rs:1527-1540`) — pops/shrinks from
   the **end** until the discarded unit count is consumed;
3. `note_pattern_after_suffix_trim` (`history.rs:680`, `history.rs:1494-1508`) —
   O(1) compare of the last remaining run against `runs[last % 2]`.

Prefix trim always `drop_prefix_wrap_runs` (`history.rs:726`,
`history.rs:1543-1558`) then `note_pattern_after_prefix_trim`
(`history.rs:727`, `history.rs:1511-1524`). There is no retained-tail
`runs.iter().enumerate().all(...)` re-walk.

Append no longer calls a full-run refresh. Each unit in `extend_wrap_line`
updates RLE then `note_appended_wrap_run` (`history.rs:622-633`,
`history.rs:1466-1491`): `n <= 2` sets `pattern_len`; otherwise, if already
period-2, compare the closed previous run (on push) and the last run against
the two-run template. That is O(1) per appended unit of the eager suffix, not
O(retained runs).

Period-2 occupancy is unchanged in shape from c3e9a86 and still matches the
claim:

```573:577:crates/seyal-terminal/src/history.rs
        if chain.pattern_len == 2 && chain.runs.len() >= 2 {
            wrap_occupancy_repeating(&chain.runs[..2], width, units)
        } else {
            wrap_occupancy_runs(&chain.runs, width, units)
        }
```

`wrap_occupancy_repeating` (`history.rs:1630-1659`) folds stored `(width, count)`
pairs and drives `wrap_occupancy_run` with those counts. It allocates
column-sized `seen` / `scratch` vectors (`O(cols)`), not a per-unit width
`Vec`. Tests `wrap_column_before_preserves_period_two_run_counts`
(`history.rs:2065-2127`) and
`wrap_suffix_trim_drops_stale_two_run_tail_before_reappend`
(`history.rs:2130-2202`) pass. The former now also asserts that suffix trim
preserves stored run-count totals and that re-append keeps `[(1, 2), (2, 1)]`
with `pattern_len == 2`.

## Findings

### P1 — wrap index is counted as derived but is not a 4 MiB droppable cache

Unchanged in substance from c3e9a86.

`derived_cache_bytes` includes wrap (`history.rs:480-486`).
`update_resident_bytes` does not (`history.rs:1029-1037`). Closed hard-broken
rows are omitted from the index (`extend_wrap_line` `history.rs:592-604`).
Those older charging defects stay corrected.

SPEC-010 §9.1 still requires a 4 MiB per-execution derived cap, with derived
eviction dropping indexes without deleting canonical payload. Remaining holes:

- `enforce_wrap_index_cap` (`history.rs:730-740`) only `pop_front`s **closed**
  chains, and only while `wrap_chains.len() > 1`. A single open SoftWrap chain
  can grow without that cap. Period-2 detection does not compact storage:
  occupancy reads `runs[..2]`, but the `Vec` still holds one RLE entry per
  width change (`WrapChain` comment `history.rs:242-244`).
- `drop_derived_cache` (`history.rs:489-491`) drops only `reflow_cache`.
  Runtime aggregate enforcement (`crates/seyal-runtime/src/runtime/mod.rs:217-241`)
  reads `derived_history_cache_bytes()` (wrap + reflow), calls
  `drop_derived_history_cache` → `drop_derived_cache`, then subtracts the
  pre-drop total even when wrap remains. Wrap-index pressure is accounted as
  freed and never actually dropped.

`blank_rows_seal_and_stay_inside_the_resident_cap` (`history.rs:1971`) stays
under the derived cap because those rows are omitted from wrap. There is still
no test that a long open mixed chain stays inside 4 MiB, that wrap can be
rebuilt after drop, or that canonical payload survives wrap-index pressure
without relying on the 32 MiB resident cap.

This fails #819 acceptance “derived visual/index state is bounded and
rebuildable” and SPEC-010 §9.1.

### P1 — aperiodic mixed occupancy is still linear in stored runs

Unchanged in substance from c3e9a86; this SHA does not touch the occupancy
walk.

`wrap_column_before` (`history.rs:561-577`) is on the eager-suffix path
(`eager_resize_suffix` `history.rs:550-555`). Closed-form occupancy applies
only when `pattern_len == 2`. Incremental detection sets that only for a
strict 2-cycle of `(width, count)` pairs. Period-3 or irregular mixed-width
text (`pattern_len == 0`) still uses `wrap_occupancy_runs`
(`history.rs:1615-1627`), which iterates every preceding run. Each homogeneous
run is O(columns) via `wrap_occupancy_run` (`history.rs:1578-1612`), so many
count-1 runs remain proportional to preceding units.

SPEC-010 §7 is not limited to period-2: “A resize must not require work
proportional to all retained history before the active surface can progress.”
Hostile long mixed lines are in the #819 acceptance set. Removing
`refresh_pattern_len` bounds **pattern bookkeeping** on trim/append; it does
not bound this occupancy path.

Additional trigger introduced by this SHA (not a separate P0; occupancy stays
correct): `note_pattern_after_prefix_trim` (`history.rs:1511-1524`) keeps
`pattern_len == 2` only when `n == 2` or `drop_units % period_units == 0`.
A mid-chain segment eviction that is not a whole-period multiple (normal for
SPEC-010 §5.1 fragmented SoftWrap) sets `pattern_len = 0`. The next eager
`wrap_column_before` then takes `wrap_occupancy_runs` even if the remaining
runs are still a phase-shifted 2-cycle. c3e9a86's full-run refresh would have
re-detected period-2 at eviction cost; this SHA trades that scan for losing
the closed-form flag. That is extra linear occupancy after ordinary eviction,
not a substitute for fixing aperiodic chains.

### P2 — source Metal ordering is still unproven for history-wide glyphs

Source inspection still shows history encoding backgrounds before glyphs
(`macos/Seyal/Sources/MetalTerminalRenderer.swift:1320-1342`) and continuation
cells receiving no glyph/wide-glyph flags
(`macos/Seyal/Sources/MetalTerminalRenderer.swift:736-765`). That matches
SPEC-011 §7.2 / §12.1 for the identified continuation-overpaint defect.

There is still no direct history wide-lead-plus-continuation offscreen
regression. `wideGraphemeOffscreenSelfTest` exercises a live
`NativePreparedFrame` (`macos/Seyal/Sources/RendererValidation.swift:340-393`);
the history comparison uses a width-one `A`
(`RendererValidation.swift:395-472`). This is the expected remaining P2.
**ENVIRONMENT_UNSUPPORTED:** this review did not execute Metal/offscreen tests.

No P0 found on canonical payload rewrite: trim/append still mutate only the
derived wrap index, not retained source records, `LineId`s, or break lineage
(ADR-010 §7 canonical mutation layer).

## Closing-gate disposition

| #819 / SPEC-010 gate | At `673d1ec` on this host |
| --- | --- |
| Eager resize must not walk retained wrap-run tails before the active surface is installed (SPEC-010 §7 / ADR-010 §7) | **This SHA's claimed scan is gone** (`refresh_pattern_len` removed; suffix drop + O(1) pattern notes). |
| Period-2 occupancy uses stored RLE counts, not a per-unit width `Vec` | **Met in source + tests named above.** |
| Derived 4 MiB wrap/reflow indexes droppable without deleting canonical payload (SPEC-010 §9.1; #819 “bounded and rebuildable”) | **Not met** (P1 wrap index). |
| Resize work not proportional to retained mixed wrap runs (SPEC-010 §7; hostile long lines) | **Not met** (P1 aperiodic occupancy). |
| `make check` / Foundation gates | **Not established.** Not run. Native `Seyal.app` is ENVIRONMENT_UNSUPPORTED on Linux x86_64. |
| Headed manual verification (issue steps 1–5) | **ENVIRONMENT_UNSUPPORTED.** |
| ARM64 macOS physical-host reflow p50/p95/p99 (SPEC-010 §18.1) and measurement matrix (§18.2) | **ENVIRONMENT_UNSUPPORTED.** No such evidence is claimed. |
| SPEC-011 §7.2 / §12.1 history wide-glyph headed proof | **ENVIRONMENT_UNSUPPORTED** (remaining source P2). |

Do not close #819. Use a non-closing relationship (`Refs #819` / `Part of
#819`) if this SHA is merged as a follow-up to the tail-scan fix.

This Linux x86_64 review makes no headed, Swift/FFI, Metal, ARM64 macOS, or
physical-performance execution claim; those issue-required evidence gates are
not established here.
