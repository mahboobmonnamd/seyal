# Independent closing re-review — Seyal OSS #819 HistoryStore / reflow

Reviewer: Terra. Host: Linux x86_64. Scope: closing re-review of one candidate commit.

## Verdict

**NO-GO for closing #819 at `9993c4735f1e18cb1afaef3b43dfde125e0e17dd`.**
Do not use `Closes #819`, `Fixes #819`, or `Resolves #819`.

This follow-up genuinely fixes the charging and trim-shape defects the
predecessor NO-GO raised: the wrap index is no longer billed to the 32 MiB
resident source budget, `Vec::remove(0)` is gone, standalone hard-broken rows
no longer allocate a chain apiece, and the suffix trim no longer scans retained
fragments.

It is nonetheless a NO-GO, and the primary reason is new. The commit adds a
`pattern_len == 2` fast path to `wrap_column_before` that discards the stored
RLE run counts. That path fires on the commonest mixed-width chain shape there
is — some ASCII followed by some wide units — and returns the wrong resize
carry column. `wrap_column_before` feeds `start_col` straight into
`reflow_from`, so this is wrong visible output on the primary resize path, not
an internal accounting slip. The predecessor did not have this defect.

## Candidate verification

| Field | Value |
| --- | --- |
| Worktree | `/tmp/seyal-oss-work/issue-819-history-store` |
| Branch | `issue/819` |
| HEAD at review start | `9993c4735f1e18cb1afaef3b43dfde125e0e17dd` — confirmed |
| Subject | `fix(history): bound wrap-index trim and charge it as derived` — confirmed |
| Author date | 2026-09-10T03:37:26+00:00 |
| Comparison | `origin/master...HEAD` |
| Worktree status at review start | clean, `## issue/819...origin/issue/819` |
| `git diff --check origin/master...HEAD` | passed, no output |

Commit content: `crates/seyal-terminal/src/history.rs` (+288/−61) plus
`docs/evidence/m002-history-819-source-p1-b2014a2.md`. Nothing else. The resize
caller in `screen.rs` is unchanged by this commit.

### Required test runs (both against the live worktree at `9993c47`)

- `rustup run 1.98.0-x86_64-unknown-linux-gnu cargo test -p seyal-terminal --locked --lib --manifest-path /tmp/seyal-oss-work/issue-819-history-store/Cargo.toml -- history::tests`
  → **ok, 13 passed, 0 failed** (31 filtered out).
- `rustup run 1.98.0-x86_64-unknown-linux-gnu cargo test -p seyal-terminal --locked --test history_store_regressions --manifest-path /tmp/seyal-oss-work/issue-819-history-store/Cargo.toml`
  → **ok, 20 passed, 0 failed**.

Both suites are green. They are green over a defect that is wrong at every
terminal width I tried; see F1 for why the added test does not catch it.

### Candidate moved during this review

The worktree HEAD advanced while I was reviewing, by commits I did not make:

```
673d1ec 2026-09-10T04:41:39Z fix(history): trim wrap runs without scanning the retained tail
145d672 2026-09-10T04:33:39Z docs(819): retain Grok re-review of c3e9a86 (NO-GO)
c3e9a86 2026-09-10T04:23:54Z fix(history): keep period-2 wrap occupancy on stored RLE counts
9993c47 2026-09-10T03:37:26Z fix(history): bound wrap-index trim and charge it as derived   <- reviewed
```

`9993c47` is an ancestor of the current HEAD and the worktree is clean, so
nothing was disturbed. All evidence below was captured while HEAD was
`9993c47`: both required suites ran at ~03:38, and the frozen source copy used
for the probes was taken at ~03:45 (the hung-probe terminal log records
`started_at 03:47:06Z` against that copy), all well before `c3e9a86` at
04:23:54. **I did not audit `c3e9a86` or `673d1ec`** — they are outside this
assignment. I note only that `c3e9a86`'s subject describes the same RLE-count
defect as F1 below, so F1 appears already known downstream; whether it is
actually fixed is not something this review establishes.

## Disposition of the predecessor (`b2014a2`) findings

**Predecessor P1 #1 — suffix trim scanned all retained fragments. Fixed as to
boundedness.** `trim_wrap_suffix_from` replaces `Vec::retain` with
`partition_point` + `truncate`, and `trim_wrap_runs` (which walked from the
start of kept runs) is replaced by `drop_suffix_wrap_runs`, which pops from the
end and touches only dropped runs (`history.rs:637-687`, `1477-1497`).
`trim_wrap_before_retained` likewise uses `pop_front`/`drain` instead of
`Vec::remove(0)` (`history.rs:689-739`). The boundedness objection is answered.
The trim is now bounded but incorrect for one chain shape — see F2.

**Predecessor P1 #2 — mixed-width occupancy walked every preceding run. Fixed
only for exactly-periodic chains.** The alternating 1/2 case is now handled by
`wrap_occupancy_repeating`, which is bounded by `O(cols × period)`. But
`refresh_pattern_len` sets `pattern_len = 0` unless the *entire* run vector is
period-2 in `(width, count)`, and real mixed-width history is not periodic.
Non-periodic chains fall back to `wrap_occupancy_runs`, which is
`O(runs × cols)` with one run per width change. Measured against a chain with a
one-in-five chance of a wide unit, `cols = 80` (release, on the frozen `9993c47`
sources):

```
units=  10000 runs=  3226 pattern_len=0 wrap_column_before= 169.687µs
units=  40000 runs= 12768 pattern_len=0 wrap_column_before= 659.167µs
units= 160000 runs= 51161 pattern_len=0 wrap_column_before=   2.648ms
units= 640000 runs=204237 pattern_len=0 wrap_column_before=  10.560ms
```

Exactly linear in retained units: 4× the units costs 4× the time. This is one
synchronous call on the eager resize path before the prepared active surface is
committed, so SPEC-010 §7 ("A resize must not require work proportional to all
retained history before the active surface can progress") and ADR-010 §7 are
still not satisfied for ordinary CJK-mixed history. The fix is narrowly shaped
to the synthetic alternating-width case the new test exercises.

**Predecessor P1 #3 — wrap index charged to resident source; quadratic chain
removal. Charging fixed; droppability newly broken.** `update_resident_bytes`
no longer adds `wrap_index_allocated_bytes()` (`history.rs:1040-1048`) and
`derived_cache_bytes` now includes it (`history.rs:480-487`), which is what
SPEC-010 §6.1 requires — derived indexes are not resident source content, and
the wrap index can no longer evict canonical payload. `Vec<WrapChain>` became a
`VecDeque`, removing the quadratic `Vec::remove(0)`. Standalone hard-broken rows
no longer create a chain each (`history.rs:592-596`). `enforce_wrap_index_cap`
does bound the index: measured, chain count plateaus at 18,432 and
`derived_cache_bytes()` plateaus at exactly 4,194,304 bytes, the §9.1 cap. That
is a real improvement. But the index is now *reported* as derived while being
*undroppable* — see F3 — and the cap is enforced by a full walk on every append
— see F4.

**Predecessor P2 — Metal history-wide wide-glyph coverage. Unchanged.** This
commit touches no Swift. Carried forward as F5.

## Findings

### F1 — P0, introduced by this commit: `wrap_column_before` returns wrong resize carry columns

`refresh_pattern_len` compares whole `(width, count)` tuples, so it reports
`pattern_len == 2` for any run vector that is period-2 *in RLE space*
(`history.rs:1465-1477`). `wrap_column_before` then rebuilds the pattern as
`[(runs[0].0, 1), (runs[1].0, 1)]`, discarding the counts
(`history.rs:573-577`):

```573:577:crates/seyal-terminal/src/history.rs
        if chain.pattern_len == 2 && chain.runs.len() >= 2 {
            wrap_occupancy_repeating(&[(chain.runs[0].0, 1), (chain.runs[1].0, 1)], width, units)
        } else {
            wrap_occupancy_runs(&chain.runs, width, units)
        }
```

Two runs is the commonest non-trivial chain shape — any soft-wrapped line that
is ASCII followed by CJK, such as `Error: 日本語のメッセージ`, produces
`[(1, N), (2, M)]`, which trivially satisfies the period-2 test. The fast path
then replays `1, 2, 1, 2, …` instead of N ones followed by M twos.

Reproduced against the real `HistoryStore` (frozen `9993c47` sources, isolated
copy, never in the candidate worktree). A chain of 40 width-1 units then 20
width-2 units, `chain.runs == [(1,40),(2,20)]`, `chain.pattern_len == 2`,
compared against the crate's own `wrap_occupancy` replay:

```
cols=8  prefix=4  expected=4  got=6      cols=20 prefix=41 expected=2  got=3
cols=8  prefix=41 expected=2  got=1      cols=20 prefix=55 expected=10 got=4
cols=8  prefix=55 expected=6  got=7      cols=20 prefix=60 expected=0  got=12
cols=20 prefix=4  expected=4  got=6      cols=80 prefix=4  expected=4  got=6
cols=20 prefix=15 expected=15 got=3      cols=80 prefix=41 expected=42 got=61
```

An `a a a <wide>` cadence (`runs = [(1,3),(2,1)]` repeated) is wrong in all 15
sampled `(cols, prefix)` combinations. Note the very first row: a prefix of four
*pure width-1* units at `cols = 8` returns 6 instead of 4, because the fast path
substitutes the whole chain's width sequence.

A randomized differential over 200,000 chains (random widths 1–2, length 1–40,
`cols` 1–100) against a unit-by-unit replay:

```
wrap_occupancy_runs (the b2014a2 path)   : 0 mismatches / 200000
new fast path eligible                   : 15203 chains (7.6%)
new fast path wrong                       : 6847 of those (45.0%)
```

So the path this commit replaced was correct on every one of 200,000 cases, and
the replacement is wrong on 45% of the chains it claims. This is a regression
introduced here, not a carried-over defect.

It is not internal-only. `screen.rs:366-383` takes `start_col` from
`eager_resize_suffix` — which is exactly `wrap_column_before` when the cut
omits a soft-wrap join — and passes it into `source.reflow_from(cols,
usize::MAX, start_col)`. A wrong carry column shifts where the reflowed
near-visible surface begins and therefore where every subsequent wrap falls, so
a resize that cuts a mixed-width soft-wrap chain produces misaligned output.
That contradicts SPEC-010 §7 items 4–5 and the #819 acceptance criterion
"resize narrower/wider/oscillation preserves source text, styles, LineIds and
hard/soft lineage".

The added test `wrap_column_before_is_closed_form_for_alternating_widths`
passes because strictly alternating 1/2 is the one period-2 family where all
run counts are already 1, making the discarded counts a no-op. The test
therefore validates the only input the fast path handles correctly.

### F2 — P1, introduced by this commit: trims leave stale runs on two-run chains

Both trim paths skip run maintenance entirely when `pattern_len == 2 &&
runs.len() <= 2` unless the chain empties (`history.rs:677-685` and
`730-738`). For a partial suffix trim the fragments shrink and the runs do not,
so the index describes more units than it retains. Reproduced on the real store:
after building `runs == [(1,40),(2,20)]` and calling `truncate_from` at line 50,

```
runs=[(1, 40), (2, 20)] describe 60 units but fragments retain 50
```

The stale runs are then consumed by `wrap_column_before` on the next resize,
which compounds F1. This is index corruption that survives the resize, so it is
distinct from F1 even though the two share a trigger.

### F3 — P1: the wrap index is reported as derived but cannot be dropped

`derived_cache_bytes` now includes `wrap_index_allocated_bytes()`, but
`drop_derived_cache` takes `&self` and can only clear the `RefCell` reflow cache
(`history.rs:480-491`); `wrap_chains` is a plain field and needs `&mut`.
Measured on the real store after 20,000 soft-wrapped lines and one reflow:

```
derived_cache_bytes before drop = 865240
derived_cache_bytes after  drop = 786688   (91% not reclaimed)
```

SPEC-010 §9 requires reflow/layout caches to be "bounded and rebuildable" and
§9.1 requires dropping derived caches on derived-budget pressure. Reporting
bytes into that budget that the drop path cannot release inverts the intent.

It also corrupts the runtime's aggregate enforcement. `Runtime::
enforce_derived_history_cache_budget` (`crates/seyal-runtime/src/runtime/mod.rs:217-242`)
reads `before`, calls `drop_derived_history_cache()`, then unconditionally does
`total = total.saturating_sub(before)`. The loop terminates, so there is no
hang, but it decrements by memory it did not free. With the per-execution wrap
index pinned at the 4 MiB cap (F4's measurement shows it reaching exactly
4,194,304 bytes and staying there), eight such executions pin the entire 32 MiB
`HISTORY_RUNTIME_DERIVED_INDEX_CAP` with no reclamation path, while the runtime
reports the budget as satisfied.

This is a direct consequence of the otherwise-correct reclassification in this
commit: under `b2014a2` these bytes were wrongly charged to resident source but
were at least trimmed; now they are correctly excluded from resident but are
unreachable by the derived-pressure path.

### F4 — P1: every appended line walks the whole wrap index

`extend_wrap_line` ends with `self.enforce_wrap_index_cap()`
(`history.rs:634`), whose loop condition evaluates `wrap_index_allocated_bytes()`
(`history.rs:741-751`), which sums over every chain
(`self.wrap_chains.iter().map(...).sum()`, `history.rs:753-769`). Append is the
hottest path in the store. Measured (release, frozen `9993c47` sources), each
row is the cost of appending 4,000 lines after preloading the given number of
soft-wrap groups:

```
chains=  1000  derived=  185344  per_line=  1620ns
chains=  4000  derived=  741376  per_line=  3676ns
chains= 16000  derived= 2965504  per_line= 11942ns
chains= 18432  derived= 4194304  per_line= 18961ns
chains= 18432  derived= 4194304  per_line= 20022ns
```

The good news is the plateau: the §9.1 cap does bound chain count, so this is
`O(1)` in retained history rather than unbounded. The bad news is the constant
— steady-state per-line append cost is 12.4× the small-history cost, about
20 µs per line, spent recomputing a value that could be maintained
incrementally. Under `b2014a2` the same walk sat in `update_resident_bytes`, so
this is carried over rather than newly introduced, but the commit had the
opportunity to remove it and instead relocated it.

For completeness: I hypothesised that `trim_wrap_before_retained`'s
`prefix_units` reindex would make steady-state eviction scale with chain
length, and tested it. It does not. At a fixed 32 MiB resident cap, an 8.0×
increase in retained fragments (398 → 3,173) changed the cost of appending
4 MiB of cells by only 1.1× (169 ms → 181 ms), and `derived_cache_bytes` stayed
at 12 KiB and 99 KiB, far under the cap. That hypothesis is **not** supported
and I am not raising it as a finding.

### F5 — P2, carried: no history-wide wide-glyph Metal coverage

Unchanged by this commit, which touches no Swift. As recorded against
`b2014a2`, the source ordering is correct — history encodes all cell
backgrounds before its glyph pass, and continuation cells receive no
glyph/wide-glyph flags — but there is still no direct history
wide-lead-plus-continuation offscreen regression;
`wideGraphemeOffscreenSelfTest` drives a live `NativePreparedFrame` while the
history comparison uses a width-one `A`. Source inspection only.

### Observation, not a finding — `wrap_chain_containing` can return a stale chain

`wrap_chain_containing` selects the last chain whose *first* fragment precedes
`from` and never checks that `from` lies inside that chain
(`history.rs:580-590`). Now that standalone hard-broken rows create no chain,
an anchor after a closed chain resolves to that earlier chain and
`units_before` returns its full unit count instead of 0. In the normal resize
path this looks unreachable, because `eager_resize_suffix` only consults
`wrap_column_before` when `omitted_joins` is true, which requires the preceding
entry to be `SoftWrap`, which in turn implies the anchor is not a chain's first
fragment. The one gap I can construct requires a zero-unit `SoftWrap` line,
which `extend_wrap_line` skips entirely (`history.rs:597-604`); I could not
establish that a real terminal produces one. Recording it because the guard is
absent rather than deliberate, and F1/F2 change the surrounding code. **Not
reproduced, and not part of the NO-GO basis.**

## Evidence integrity and method

All probes ran against a frozen copy of the `9993c47` sources at
`/tmp/terra-scratch/wt`, taken at ~03:45 (before any later commit existed) and
since deleted. The candidate worktree was never written to; it was verified
clean before and after, and `git diff HEAD` is empty. Test scaffolding lived
only in the copy. The standalone differential harness — a verbatim port of
`wrap_advance`, `wrap_occupancy_run`, `wrap_occupancy_runs`,
`wrap_occupancy_repeating` and `refresh_pattern_len` from `9993c47`, plus a
unit-by-unit reference — is retained at `/tmp/terra-scratch/occupancy.rs` and
builds with `rustup run 1.98.0-x86_64-unknown-linux-gnu rustc --edition 2024 -O`.

The timings in F2's disposition, F4 and the F4 negative result are **differential
scaling probes on one Linux x86_64 host, not benchmark evidence**. They are
reported only as ratios and orders of magnitude to locate cost that scales with
retained history. They are not, and must not be read as, a #818/#673
performance claim, and they establish nothing about macOS, ARM64 or the
required resource matrix.

## Closing-gate disposition

F1 is a P0 wrong-output regression on the primary resize path and by itself
blocks the #819 acceptance item "resize narrower/wider/oscillation preserves
source text, styles, LineIds and hard/soft lineage". F2 corrupts the wrap index
across a resize. F3 fails "derived visual/index state is bounded and
rebuildable" — bounded yes, rebuildable-under-pressure no. The still-linear
non-periodic occupancy path (predecessor P1 #2) fails SPEC-010 §7 and
ADR-010 §7.

Independently of the source defects, the closing evidence gates are not
established by this review and cannot be closed from here. This is a Linux
x86_64 source-and-unit-test review only. It makes **no** headed, Swift/FFI,
Metal, ARM64 macOS, or physical-performance execution claim. The #819
acceptance items for append/reflow p50/p95/p99 and 1/10/50/100 execution
resource evidence under #818/#673 policy, the manual verification steps, and
`make check` / Foundation gates are not demonstrated here. Even had the source
P0–P1 findings been absent, those outstanding gates would still preclude
closing the issue.
