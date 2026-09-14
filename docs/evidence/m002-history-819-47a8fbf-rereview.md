# Independent closing re-review — Seyal OSS #819 HistoryStore / reflow

## Verdict

**NO-GO for closing #819 at `47a8fbf2764024b63877f398bc944d8315b622c6`.**
Do not use `Closes #819`, `Fixes #819`, or `Resolves #819`.

**Portable source of this SHA: GO.** The claimed miss-path repair holds.
When the wrap index is absent, `wrap_column_before` rebuilds occupancy from
the SoftWrap chain containing `from` via `wrap_occupancy_from_canonical`
instead of returning `0`. That walk stops at the preceding `HardBreak`; it
does not continue through unrelated retained rows. Standalone hard-broken
rows still yield `0` (`saw_soft` never set). The function takes `&HistoryStore`
and does not rewrite canonical payload, `LineId`s, or break lineage. Those
are not enough to close #819. Aperiodic mixed occupancy is still linear in
stored runs (SPEC-010 §7). That remaining P1 is a Done-gate defect.

Headed / native / IME / Metal / ARM64 macOS / physical-host performance /
`make check` are **ENVIRONMENT_UNSUPPORTED** on this Linux x86_64 review and
are independently unmet.

## Scope and verification

- Worktree: `/tmp/seyal-oss-work/issue-819-history-store`
- Branch: `issue/819`
- HEAD confirmed: `47a8fbf2764024b63877f398bc944d8315b622c6`
- Subject confirmed: `fix(history): rebuild wrap occupancy from the current SoftWrap chain`
- Parent: `3abc64a4db58237b8c001f3a8138ba795ccacc77` (docs-only Terra re-review of
  ancestor `9993c47`). Code predecessor: `023ceda90cb3f677a382ef2333de2da0dc38eaa1`
- Comparison: `origin/master...HEAD`
- Worktree status after review/test: clean (`## issue/819...origin/issue/819`)
- `git diff --check origin/master...HEAD`: passed (no output).
- Production tree contains no `rebuild_wrap_index` symbol. Occupancy after wrap
  drop is computed on demand from canonical units of the current chain; the
  wrap `VecDeque` is not restored.
- `rustup run 1.98.0-x86_64-unknown-linux-gnu cargo test -p seyal-terminal --locked --lib --manifest-path /tmp/seyal-oss-work/issue-819-history-store/Cargo.toml -- history::tests`: passed, 16 tests (includes `drop_derived_cache_drops_wrap_index_without_touching_payload`, which now expects occupancy `1` after drop, `wrap_column_before_walks_only_the_current_soft_wrap_chain`, `wrap_column_before_preserves_period_two_run_counts`, and `wrap_suffix_trim_drops_stale_two_run_tail_before_reappend`).
- `rustup run 1.98.0-x86_64-unknown-linux-gnu cargo test -p seyal-terminal --locked --test history_store_regressions --manifest-path /tmp/seyal-oss-work/issue-819-history-store/Cargo.toml`: passed, 20 tests.

Linux x86_64 only (`uname -m`: x86_64; `rustc 1.98.0 (88d9e12ae 2026-08-18)`).
No headed, Swift/FFI, Metal, ARM64 macOS, or physical-host performance
execution is claimed. `make check` was not run.

Issue #819 is OPEN. Acceptance still requires bounded occupancy for hostile
mixed lines, headed steps 1–5, `make check`, and #818/#673 physical evidence.
This SHA’s commit message correctly uses `Refs #819`.

## Predecessor P1 disposition at 023ceda

| 023ceda finding | At `47a8fbf` |
| --- | --- |
| P1 wrap index dropped, but `wrap_column_before` returned `0` instead of rebuilding from canonical payload (SPEC-010 §9.1 rebuild half) | **Addressed.** Miss path calls `wrap_occupancy_from_canonical`. Drop test now expects the pre-drop column (`1`), not `0`. Canonical resident bytes and `resolve_anchor` still prove payload survives. |
| P1 aperiodic mixed occupancy linear in stored runs | **Remains.** Closed-form path is still `pattern_len == 2` only. This SHA does not alter `wrap_occupancy_runs` or period detection. |
| Period-2 occupancy uses stored first-period RLE counts, not a per-unit width `Vec` or summed `[(1,n),(2,m)]` | **Still true on the indexed path.** Miss path reconstructs RLE from canonical units and uses `wrap_occupancy_runs`, not the compacted two-run template. |
| Uncompacted two-run P2 stale tail must still drop | **Holds.** Untouched in this SHA. Test `wrap_suffix_trim_drops_stale_two_run_tail_before_reappend` still passes. |
| P2 Metal history wide-lead + continuation ordering unproven by headed evidence | **Remains as source inspection only.** ENVIRONMENT_UNSUPPORTED here. |

## What this SHA actually changes

`wrap_column_before` no longer returns `0` when `wrap_chain_containing`
misses (empty index after `drop_derived_cache` / `enforce_wrap_index_cap`
whole-index clear, or a `from` whose chain was popped):

```573:590:crates/seyal-terminal/src/history.rs
    pub(crate) fn wrap_column_before(&self, from: Option<HistoryAnchor>, cols: u16) -> usize {
        let Some(from) = from else {
            return 0;
        };
        let width = usize::from(cols);
        if width == 0 {
            return 0;
        }
        if let Some(chain) = self.wrap_chain_containing(from) {
            let units = chain.units_before(from);
            return if chain.pattern_len == 2 && chain.runs.len() >= 2 {
                wrap_occupancy_repeating_from(&chain.runs[..2], chain.pattern_phase, width, units)
            } else {
                wrap_occupancy_runs(&chain.runs, width, units)
            };
        }
        wrap_occupancy_from_canonical(self, from, width)
    }
```

The miss path walks `reverse_entries` from the tail, skips records wholly after
`from`, collects units before `from`, and **breaks** on a `HardBreak` that is
wholly before `from` once any fragment has been collected:

```1796:1849:crates/seyal-terminal/src/history.rs
fn wrap_occupancy_from_canonical(store: &HistoryStore, from: HistoryAnchor, cols: usize) -> usize {
    let mut fragments_rev: Vec<Vec<(u8, u32)>> = Vec::new();
    let mut saw_soft = false;
    for entry in store.reverse_entries() {
        if line_wholly_after(entry, from) {
            continue;
        }
        if !fragments_rev.is_empty()
            && entry.break_after() == HistoryBreakAfter::HardBreak
            && line_wholly_before(entry, from)
        {
            break;
        }
        if entry.break_after() == HistoryBreakAfter::SoftWrap {
            saw_soft = true;
        }
        // ... per-fragment RLE of units before `from` ...
        fragments_rev.push(fragment_runs);
    }
    if !saw_soft {
        return 0;
    }
    // ... reverse-merge fragment RLE, then wrap_occupancy_runs ...
}
```

That stop is one older hard-broken record, not the rest of retained history.
`wrap_column_before_walks_only_the_current_soft_wrap_chain` still exercises
the **indexed** path (8_000 hard-broken rows then a two-fragment chain,
`start_col == 1`). After drop, the same HardBreak closer is the miss-path
`break`; occupancy cannot include those closed rows because `saw_soft` is
only set on `SoftWrap` and the loop exits at the preceding `HardBreak`.

Standalone hard-broken history never sets `saw_soft`, so the miss path still
returns `0`. `eager_resize_suffix_is_bounded_for_hard_broken_history` already
requires `start_col == 0` when `omitted_joins` is false. Direct
`wrap_column_before` on a hard-broken `from` with an empty wrap index takes
this `!saw_soft` return.

`drop_derived_cache` is unchanged (clears `reflow_cache` and `wrap_chains`,
`shrink_to_fit`). The drop test now asserts occupancy is rebuilt, not zeroed:

```2447:2470:crates/seyal-terminal/src/history.rs
    fn drop_derived_cache_drops_wrap_index_without_touching_payload() {
        // ... 8_000 width-1 SoftWrap units ...
        assert_eq!(store.wrap_column_before(Some(from), 8), 1);
        store.drop_derived_cache();
        assert_eq!(store.wrap_index_allocated_bytes(), 0);
        assert_eq!(store.derived_cache_bytes(), 0);
        assert_eq!(store.resident_bytes(), resident);
        assert_eq!(store.wrap_column_before(Some(from), 8), 1);
        assert!(matches!(
            store.resolve_anchor(HistoryAnchor {
                line_id: LineId(0),
                unit_offset: 0
            }),
            HistoryAnchorResolution::Resolved { .. }
        ));
```

`from` is `LineId(4_001)` / offset `0`, so units before the cut are 4_001
width-1 cells; `4001 % 8 == 1`. Indexed and miss-path answers match. Follow-on
append still starts a **new** chain (`continue_chain` is false when
`wrap_chains` is empty). This SHA does not restore wrap fragments for the
dropped prefix; it only recomputes the eager `start_col`.

`eager_resize_suffix` still feeds that column into active-surface reflow
(`history.rs:561-566`; `screen.rs:366-383`; `reflow_from` seeds
`used = start_col` at `history.rs:1293`). After wrap drop, that seed is the
canonical chain occupancy, not silent `0`.

Indexed period-2 / aperiodic dispatch is unchanged. `note_appended_wrap_run`
still sets `pattern_len == 2` only for a strict 2-cycle of `(width, count)`
pairs (`history.rs:1637-1663`).

## Findings

### P1 — aperiodic mixed occupancy is still linear in stored runs

Unchanged from `023ceda` / `673d1ec`. This SHA does not alter
`wrap_occupancy_runs`.

`wrap_column_before` (`history.rs:581-587`) is still on the eager-suffix path
(`eager_resize_suffix` `history.rs:561-566`; consumed as active-surface
`start_col` in `screen.rs:366-383`). Closed-form occupancy applies only when
`pattern_len == 2 && chain.runs.len() >= 2`. Incremental detection
(`note_appended_wrap_run` `history.rs:1637-1663`) sets that only for a
strict 2-cycle of `(width, count)` pairs. Period-3 or irregular mixed-width
text (`pattern_len == 0`) still uses `wrap_occupancy_runs`
(`history.rs:1851-1864`), which iterates every preceding run. Each
homogeneous run is O(columns) via `wrap_occupancy_run`
(`history.rs:1749-1784`), so many count-1 runs remain proportional to
preceding units.

SPEC-010 §7 is not limited to period-2: “A resize must not require work
proportional to all retained history before the active surface can progress.”
Hostile long mixed lines are in the #819 acceptance set. Compact bounds
**period-2 storage** (`runs` truncated to two entries) and does not bound
this occupancy path. The 4 MiB wrap cap is not a substitute: under the cap,
the walk still scales with stored aperiodic runs; over the cap, this SHA
now rebuilds from the current chain (previous finding addressed) instead of
answering `0`, but that miss path still calls `wrap_occupancy_runs` on the
reconstructed RLE and does not make the indexed aperiodic case closed-form.

`wrap_column_before_is_closed_form_for_alternating_widths` and
`wrap_column_before_preserves_period_two_run_counts` only cover period-2
templates. There is still no test that period-3 / irregular mixed occupancy
is bounded.

This remains a #819 Done-gate miss on SPEC-010 §7 / ADR-010 §7.

### P2 — source Metal ordering is still unproven for history-wide glyphs

Source inspection still shows history encoding backgrounds before glyphs
(`macos/Seyal/Sources/MetalTerminalRenderer.swift:1320-1324`) and
continuation cells receiving no glyph/wide-glyph flags
(`macos/Seyal/Sources/MetalTerminalRenderer.swift:736-765`). That matches
SPEC-011 §7.2 / §12.1 for the identified continuation-overpaint defect.

There is still no direct history wide-lead-plus-continuation offscreen
regression. `wideGraphemeOffscreenSelfTest` exercises a live
`NativePreparedFrame`; the history comparison uses a width-one `A`
(`RendererValidation.swift:395-472`). This is the expected remaining P2.
**ENVIRONMENT_UNSUPPORTED:** this review did not execute Metal/offscreen tests.

This P2 is SPEC-011, not SPEC-010 §7 / §9.1.

No P0 found on canonical payload rewrite: `wrap_occupancy_from_canonical`
is read-only. Trim/append/cap-clear still mutate only the derived wrap index
(or truncate source through the normal `truncate_from` history-edit path),
not retained source records, `LineId`s, or break lineage as a side effect of
derived eviction (ADR-010 §7 canonical mutation layer).

The miss path does not restore `wrap_chains`. That is occupancy rebuild for
`start_col`, not a persistent wrap-index rebuild. Visual layout after drop can
match the indexed column (drop test). It is not a remaining P1 on the
023ceda “returns 0” hole.

## Claim audit (this SHA)

| Claim | Disposition |
| --- | --- |
| When wrap index is dropped, `wrap_column_before` rebuilds occupancy from the current SoftWrap chain via `wrap_occupancy_from_canonical` instead of returning `0` | **Met.** Drop test expects `1` after `drop_derived_cache`. Eager `start_col` can use that value. |
| Must not walk all retained history (only the chain containing `from`) | **Met in source.** `HardBreak` wholly before `from` breaks after fragments are collected. `wholly_after` skips newer records without adding them. Unrelated older hard-broken rows are not folded into `runs`. |
| Standalone hard-broken rows still yield `0` | **Met in source.** `!saw_soft` returns `0`. Closed hard-broken rows remain omitted from the wrap index (`extend_wrap_line`). |
| Canonical payload unchanged | **Met.** Read-only miss path. Drop still clears only derived wrap/reflow. Resident bytes and `resolve_anchor` unchanged in the drop test. |
| Aperiodic mixed occupancy | **Not claimed, not fixed.** Remains P1. |

## Closing-gate disposition

| #819 / SPEC-010 gate | At `47a8fbf` on this host |
| --- | --- |
| Wrap index droppable as a 4 MiB derived cache without deleting canonical payload (SPEC-010 §9.1 drop half) | **Met** (inherited from `023ceda`; this SHA does not regress it). |
| Dropped wrap/reflow indexes rebuild to the same visual layout (SPEC-010 §7 / §9.1; #819 “bounded and rebuildable”) | **Occupancy half met** for `wrap_column_before` / eager `start_col`. Wrap `VecDeque` is still not restored. Reflow-cache drop/rebuild equivalence remains the existing `reflow_cache_matches_rebuild_from_canonical_history` test. |
| Period-2 occupancy uses stored first-period RLE counts, not a per-unit width `Vec` or summed `[(1,n),(2,m)]` | **Met on the indexed path** (inherited). Miss path uses reconstructed RLE + `wrap_occupancy_runs`. |
| Resize work not proportional to retained mixed wrap runs (SPEC-010 §7; hostile long lines) | **Not met** (P1 aperiodic occupancy). |
| `make check` / Foundation gates | **Not established.** Not run. Native `Seyal.app` is ENVIRONMENT_UNSUPPORTED on Linux x86_64. |
| Headed manual verification (issue steps 1–5) | **ENVIRONMENT_UNSUPPORTED.** |
| ARM64 macOS physical-host reflow p50/p95/p99 (SPEC-010 §18.1) and measurement matrix (§18.2) | **ENVIRONMENT_UNSUPPORTED.** No such evidence is claimed. |
| SPEC-011 §7.2 / §12.1 history wide-glyph headed proof | **ENVIRONMENT_UNSUPPORTED** (remaining source P2). |

Do not close #819. Keep a non-closing relationship (`Refs #819` / `Part of
#819`) if this SHA is merged as a follow-up to wrap occupancy after drop.

This Linux x86_64 review makes no headed, Swift/FFI, Metal, ARM64 macOS, or
physical-performance execution claim; those issue-required evidence gates
are not established here.
