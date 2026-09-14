# Independent closing re-review — Seyal OSS #819 HistoryStore / reflow

## Verdict

**NO-GO for closing #819 at `023ceda90cb3f677a382ef2333de2da0dc38eaa1`.**
Do not use `Closes #819`, `Fixes #819`, or `Resolves #819`.

**Portable source of this SHA: NO-GO.** Wrap bytes are now actually dropped
(`drop_derived_cache` clears `wrap_chains` and shrinks; Runtime/exec APIs are
`&mut self`; cap pressure may discard the whole wrap index). Period-2 compact
keeps the first-period two-run template and does **not** sum into
`[(1,n),(2,m)]`. Uncompacted two-run suffix trim still drops a stale tail.
Those claimed repairs hold. They are not enough. Aperiodic mixed occupancy
is still linear in stored runs (SPEC-010 §7). After wrap drop, occupancy is
not rebuilt from canonical payload: `wrap_column_before` returns `0`, and
eager resize feeds that into the active-surface reflow start column
(SPEC-010 §9.1 / #819 “bounded and rebuildable”). Those remaining P1s are
Done-gate defects.

Headed / native / IME / Metal / ARM64 macOS / physical-host performance /
`make check` are **ENVIRONMENT_UNSUPPORTED** on this Linux x86_64 review and
are independently unmet.

## Scope and verification

- Worktree: `/tmp/seyal-oss-work/issue-819-history-store`
- Branch: `issue/819`
- HEAD confirmed: `023ceda90cb3f677a382ef2333de2da0dc38eaa1`
- Subject confirmed: `fix(history): drop wrap index with derived cache and compact period-2 runs`
- Parent: `b1d35b6edadc728688866ad34cbb94c73016b29c` (docs-only Grok re-review of
  `673d1ec`). Code predecessor: `673d1ec26e063426fa86f957c988075df56cba33`
- Comparison: `origin/master...HEAD`
- Worktree status after review/test: clean (`## issue/819...origin/issue/819`)
- `git diff --check origin/master...HEAD`: passed (no output).
- Production tree contains no `rebuild_wrap_index` symbol. Wrap index is
  incrementally maintained on append and cleared on drop/cap; there is no
  canonical rebuild path.
- `rustup run 1.98.0-x86_64-unknown-linux-gnu cargo test -p seyal-terminal --locked --lib --manifest-path /tmp/seyal-oss-work/issue-819-history-store/Cargo.toml -- history::tests`: passed, 16 tests (includes `drop_derived_cache_drops_wrap_index_without_touching_payload`, `wrap_column_before_preserves_period_two_run_counts`, and `wrap_suffix_trim_drops_stale_two_run_tail_before_reappend`).
- `rustup run 1.98.0-x86_64-unknown-linux-gnu cargo test -p seyal-terminal --locked --test history_store_regressions --manifest-path /tmp/seyal-oss-work/issue-819-history-store/Cargo.toml`: passed, 20 tests.

Linux x86_64 only (`uname -m`: x86_64; `rustc 1.98.0 (88d9e12ae 2026-08-18)`).
No headed, Swift/FFI, Metal, ARM64 macOS, or physical-host performance
execution is claimed. `make check` was not run.

Issue #819 is OPEN. Acceptance still requires bounded **and rebuildable**
derived indexes, hostile long-line coverage, headed steps 1–5, `make check`,
and #818/#673 physical evidence. This SHA’s commit message correctly uses
`Refs #819`.

## Predecessor P1 disposition at 673d1ec

| 673d1ec finding | At `023ceda` |
| --- | --- |
| P1 wrap index counted as derived but not a 4 MiB droppable cache (`drop_derived_cache` dropped only `reflow_cache`; open SoftWrap chain uncapped; Runtime subtracted wrap bytes that were never freed) | **Drop half addressed.** Wrap VecDeque is cleared and `shrink_to_fit`’d. Exec/Terminal drop APIs are `&mut self`. Cap compact-then-pop-closed, then may clear the whole index including a single open chain. Canonical payload is not deleted on that path. **Rebuild half remains** (P1 below). |
| P1 aperiodic mixed occupancy linear in stored runs | **Remains.** Closed-form path is still `pattern_len == 2` only. Compact does not change `wrap_occupancy_runs`. |
| Period-2 occupancy uses stored RLE counts, not a per-unit width `Vec` | **Still true, and compact does not collapse to summed `[(1,n),(2,m)]`.** Occupancy reads the two-run template via `wrap_occupancy_repeating_from`. Tests assert first-period counts. |
| Uncompacted two-run P2 stale tail must still drop | **Holds.** Compacted suffix trim skips run mutation; uncompacted `total == run_units` still `drop_suffix_wrap_runs`. Test `wrap_suffix_trim_drops_stale_two_run_tail_before_reappend` passes. |
| P2 Metal history wide-lead + continuation ordering unproven by headed evidence | **Remains as source inspection only.** ENVIRONMENT_UNSUPPORTED here. |

## What this SHA actually changes

`drop_derived_cache` now drops wrap storage, not only reflow:

```491:495:crates/seyal-terminal/src/history.rs
    pub(crate) fn drop_derived_cache(&mut self) {
        self.reflow_cache.get_mut().take();
        self.wrap_chains.clear();
        self.wrap_chains.shrink_to_fit();
    }
```

`TerminalState::drop_primary_history_derived_cache` and
`TerminalExecution::drop_derived_history_cache` are `&mut self` and call
`history_mut()` (`terminal.rs:514-516`, `execution.rs:77-79`). Runtime
aggregate enforcement (`crates/seyal-runtime/src/runtime/mod.rs:217-241`)
still subtracts the pre-drop `derived_history_cache_bytes()` total; that
subtraction is no longer a paper credit, because wrap bytes are in that
total (`history.rs:482-488`) and the drop actually clears them.

`update_resident_bytes` (`history.rs:1071-1080`) still excludes wrap. Closed
hard-broken rows are still omitted from the index (`extend_wrap_line`
`history.rs:596-600`). Those earlier charging/omission repairs stay in
place.

Period-2 compact truncates to the **first two stored runs** and shrinks:

```1513:1518:crates/seyal-terminal/src/history.rs
fn compact_period_two_wrap_runs(chain: &mut WrapChain) {
    if chain.pattern_len == 2 && chain.runs.len() > 2 {
        chain.runs.truncate(2);
        chain.runs.shrink_to_fit();
    }
}
```

That is a two-run **template**, not a sum of all matching widths. After 2_000
lines of `[1,1,2]`, the unit test requires `runs == [(1, 2), (2, 1)]`, not
`[(1, 4000), (2, 2000)]`. Alternating width-1/width-2 requires `[(1, 1), (2, 1)]`.
Occupancy then repeats that template:

```577:581:crates/seyal-terminal/src/history.rs
        if chain.pattern_len == 2 && chain.runs.len() >= 2 {
            wrap_occupancy_repeating_from(&chain.runs[..2], chain.pattern_phase, width, units)
        } else {
            wrap_occupancy_runs(&chain.runs, width, units)
        }
```

`wrap_occupancy_repeating_from` (`history.rs:1610-1626`) rotates by
`pattern_phase % period` into at most one period of runs, then
`wrap_occupancy_repeating` (`history.rs:1793-1823`) consumes the stored
`(width, count)` pairs in a column-sized `seen`/`scratch` cycle. No
per-unit width `Vec` on that path.

Suffix trim of a **compacted** template (`pattern_len == 2 && runs.len() <= 2
&& total > run_units`) leaves the two-run pattern in place unless
`keep_units == 0` (`history.rs:690-701`). Uncompacted two-run chains have
`total == run_units`, so they still take `drop_suffix_wrap_runs` +
`note_pattern_after_suffix_trim`. The stale-tail test cuts inside the first
of two runs and expects `[(1, 1)]` before re-append (`history.rs:2298-2340`).

`enforce_wrap_index_cap` (`history.rs:765-782`) first compacts every chain,
then `pop_front`s **closed** chains while over 4 MiB and `len > 1`, then if
still over `HISTORY_PER_EXECUTION_DERIVED_INDEX_CAP` (4 MiB,
`history.rs:18`) clears the whole wrap index and shrinks. That path mutates
only `wrap_chains`. It does not call `truncate_from`, `evict_oldest_segment`,
or otherwise rewrite `segments` / `tail` / `LineId`s / break lineage.

`Screen::commit_prepared` still trims then re-appends before swapping the
prepared grid (`crates/seyal-terminal/src/screen.rs:584-613`). Eager resize
still clones a bounded suffix and uses wrap occupancy as `start_col`
(`screen.rs:366-383`).

## Findings

### P1 — aperiodic mixed occupancy is still linear in stored runs

Unchanged in substance from `673d1ec`. This SHA does not alter
`wrap_occupancy_runs`.

`wrap_column_before` (`history.rs:565-581`) is on the eager-suffix path
(`eager_resize_suffix` `history.rs:554-557`; consumed as active-surface
`start_col` in `screen.rs:366-383`). Closed-form occupancy applies only when
`pattern_len == 2 && runs.len() >= 2`. Incremental detection
(`note_appended_wrap_run` `history.rs:1628-1655`) sets that only for a
strict 2-cycle of `(width, count)` pairs. Period-3 or irregular mixed-width
text (`pattern_len == 0`) still uses `wrap_occupancy_runs`
(`history.rs:1778-1791`), which iterates every preceding run. Each
homogeneous run is O(columns) via `wrap_occupancy_run`
(`history.rs:1741-1776`), so many count-1 runs remain proportional to
preceding units.

SPEC-010 §7 is not limited to period-2: “A resize must not require work
proportional to all retained history before the active surface can progress.”
Hostile long mixed lines are in the #819 acceptance set. Compact bounds
**period-2 storage** (`runs` truncated to two entries) and does not bound
this occupancy path. The 4 MiB wrap cap is not a substitute: under the cap,
the walk still scales with stored aperiodic runs; over the cap, this SHA
drops the index and answers `0` (next finding) instead of a closed-form
occupancy.

`wrap_column_before_is_closed_form_for_alternating_widths` and
`wrap_column_before_preserves_period_two_run_counts` only cover period-2
templates. There is still no test that period-3 / irregular mixed occupancy
is bounded.

This remains a #819 Done-gate miss on SPEC-010 §7 / ADR-010 §7.

### P1 — wrap index is now dropped, but occupancy is not rebuilt from canonical payload

The predecessor hole (“counted as derived, never dropped”) is closed as a
byte-accounting defect. SPEC-010 §9.1 and #819 still require derived state
to be **rebuildable**. Dropping all caches and rebuilding from canonical
history must reproduce the same visual layout (SPEC-010 §7).

After `drop_derived_cache`, `wrap_column_before` returns `0` when no chain
is found (`history.rs:573-574`). The new test documents that as the
post-drop result and does not restore occupancy from retained source:

```2385:2399:crates/seyal-terminal/src/history.rs
        assert_eq!(store.wrap_column_before(Some(from), 8), 1);
        store.drop_derived_cache();
        assert_eq!(store.wrap_index_allocated_bytes(), 0);
        assert_eq!(store.derived_cache_bytes(), 0);
        assert_eq!(store.resident_bytes(), resident);
        assert_eq!(store.wrap_column_before(Some(from), 8), 0);
        ...
        store.append_line(ascii_line(8_000, "y", HistoryBreakAfter::SoftWrap));
        assert!(store.wrap_index_allocated_bytes() > 0);
```

Resident bytes and `resolve_anchor` prove canonical payload survives. The
follow-on append only proves a **new** chain can be indexed from that line;
`continue_chain` is false when `wrap_chains` is empty (`history.rs:597`), so
the dropped prefix of an open SoftWrap chain is not reconstructed.

That `0` is not inert. `eager_resize_suffix` uses it as `start_col` when the
cloned suffix omitted a SoftWrap prefix (`history.rs:554-557`). Primary
resize feeds it into `reflow_from` (`screen.rs:366-383`), which seeds
`used = start_col` (`history.rs:1285`). For a long open SoftWrap chain this
is the hostile #819 case: after derived-budget drop or
`enforce_wrap_index_cap`’s whole-index clear (`history.rs:778-781`), the
active window can reflow as if the omitted prefix occupied column 0.

There is no `rebuild_wrap_index` (removed in earlier #819 follow-ups). Cap
pressure on a single open chain now **does** drop wrap bytes rather than
growing uncapped, but the replacement answer is silent `0`, not a rebuild
from canonical units. Fragments of a compacted period-2 chain still grow
one `WrapFragment` per source record (`history.rs:620-625`), so a legal
long SoftWrap chain can still reach 4 MiB on fragments alone and take this
clear path.

`enforce_wrap_index_cap` cannot delete canonical payload (confirmed: only
`wrap_chains` compact / `pop_front` / `clear`). That is necessary and not
sufficient for §9.1.

This fails #819 acceptance “derived visual/index state is bounded and
rebuildable” on the rebuildable half.

### P2 — source Metal ordering is still unproven for history-wide glyphs

Source inspection still shows history encoding backgrounds before glyphs
(`macos/Seyal/Sources/MetalTerminalRenderer.swift:1320-1324`) and
continuation cells receiving no glyph/wide-glyph flags
(`macos/Seyal/Sources/MetalTerminalRenderer.swift:736-765`). That matches
SPEC-011 §7.2 / §12.1 for the identified continuation-overpaint defect.

There is still no direct history wide-lead-plus-continuation offscreen
regression. `wideGraphemeOffscreenSelfTest` exercises a live
`NativePreparedFrame` (`macos/Seyal/Sources/RendererValidation.swift:340`);
the history comparison uses a width-one `A`
(`RendererValidation.swift:395-472`). This is the expected remaining P2.
**ENVIRONMENT_UNSUPPORTED:** this review did not execute Metal/offscreen tests.

No P0 found on canonical payload rewrite: trim/append/cap-clear still mutate
only the derived wrap index (or truncate source through the normal
`truncate_from` history-edit path), not retained source records, `LineId`s,
or break lineage as a side effect of derived eviction (ADR-010 §7 canonical
mutation layer).

## Claim audit (this SHA)

| Claim | Disposition |
| --- | --- |
| `drop_derived_cache(&mut self)` clears `wrap_chains` and shrinks; Runtime/exec APIs are `&mut self` | **Met.** Wrap allocated bytes go to 0. Test named above. Runtime loop (`runtime/mod.rs:239`) now drops real wrap bytes. |
| Period-2 chains compact to a two-run template; occupancy uses `wrap_occupancy_repeating_from` with `pattern_phase` | **Met.** Template stays first-period counts. Not occupancy-wrong `[(1,n),(2,m)]`. |
| `enforce_wrap_index_cap` compact then pop closed, then may clear the whole wrap index if still over 4 MiB | **Met as drop.** Does not delete canonical payload. Leaves occupancy unrebuildable (P1). |
| Suffix trim of a compacted template does not destroy the two-run pattern; uncompacted 2-run chains still drop stale tails | **Met.** Compacted branch vs `drop_suffix_wrap_runs`. Test `wrap_suffix_trim_drops_stale_two_run_tail_before_reappend`. |
| Aperiodic mixed occupancy | **Not claimed, not fixed.** Remains P1. |

## Closing-gate disposition

| #819 / SPEC-010 gate | At `023ceda` on this host |
| --- | --- |
| Wrap index droppable as a 4 MiB derived cache without deleting canonical payload (SPEC-010 §9.1 drop half) | **Met in source + `drop_derived_cache_drops_wrap_index_without_touching_payload`.** Cap may clear an open chain. |
| Dropped wrap/reflow indexes rebuild to the same visual layout (SPEC-010 §7 / §9.1; #819 “bounded and rebuildable”) | **Not met** (P1 wrap occupancy after drop is `0`). |
| Period-2 occupancy uses stored first-period RLE counts, not a per-unit width `Vec` or summed `[(1,n),(2,m)]` | **Met in source + tests named above.** |
| Resize work not proportional to retained mixed wrap runs (SPEC-010 §7; hostile long lines) | **Not met** (P1 aperiodic occupancy). |
| `make check` / Foundation gates | **Not established.** Not run. Native `Seyal.app` is ENVIRONMENT_UNSUPPORTED on Linux x86_64. |
| Headed manual verification (issue steps 1–5) | **ENVIRONMENT_UNSUPPORTED.** |
| ARM64 macOS physical-host reflow p50/p95/p99 (SPEC-010 §18.1) and measurement matrix (§18.2) | **ENVIRONMENT_UNSUPPORTED.** No such evidence is claimed. |
| SPEC-011 §7.2 / §12.1 history wide-glyph headed proof | **ENVIRONMENT_UNSUPPORTED** (remaining source P2). |

Do not close #819. Keep a non-closing relationship (`Refs #819` / `Part of
#819`) if this SHA is merged as a follow-up to wrap drop/compact.

This Linux x86_64 review makes no headed, Swift/FFI, Metal, ARM64 macOS, or
physical-performance execution claim; those issue-required evidence gates
are not established here.
