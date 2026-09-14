# Independent closing re-review — Seyal OSS #819 HistoryStore / reflow

## Verdict

**NO-GO for closing #819 at `b2014a2fca721795c6fe424aa1e3e73be1e306e6`.**
Do not use `Closes #819`, `Fixes #819`, or `Resolves #819`.

The follow-up corrects the predecessor's blank mutable-tail escape and replaces
the prior UTF-8 cloning rebuild.  It does not make the resize path bounded:
trimming scans the complete retained SoftWrap-chain index before the prepared
active surface is committed, and mixed-width occupancy still scans one run per
source unit.  The wrap index also exceeds the frozen derived-index model for
large hard-broken/blank histories.

## Scope and verification

- Worktree: `/tmp/seyal-oss-work/issue-819-history-store`
- Branch: `issue/819`
- HEAD confirmed: `b2014a2fca721795c6fe424aa1e3e73be1e306e6`
- Subject confirmed: `fix(history): seal blank tails and trim wrap occupancy`
- Worktree status after review/test: clean (`## issue/819...origin/issue/819`)
- `git diff --check origin/master...HEAD`: passed (no output).
- `rustup run 1.98.0-x86_64-unknown-linux-gnu cargo test -p seyal-terminal --locked --lib --manifest-path /tmp/seyal-oss-work/issue-819-history-store/Cargo.toml -- history::tests`: passed, 11 tests.
- `rustup run 1.98.0-x86_64-unknown-linux-gnu cargo test -p seyal-terminal --locked --test history_store_regressions --manifest-path /tmp/seyal-oss-work/issue-819-history-store/Cargo.toml`: passed, 20 tests.

## Findings

### P1 — primary resize still scans the retained wrap index

`Screen::commit_prepared` calls `HistoryStore::truncate_from` before swapping
the prepared active surface (`crates/seyal-terminal/src/screen.rs:564-595`).
That path calls `trim_wrap_suffix_from` (`history.rs:721-804`).

For a suffix beginning near the end of a long SoftWrap chain,
`trim_wrap_suffix_from` computes the binary-search unit count, but then
`Vec::retain` scans every fragment in that chain (`history.rs:640-643`).
It also calls `trim_wrap_runs`, which walks width runs from the beginning
(`history.rs:1418-1435`).  Therefore a chain that spans retained history still
does work proportional to retained history before active-surface progress.
The old full UTF-8 clone is gone, but ADR-010 §7 and SPEC-010 §7 prohibit the
unbounded work itself, not only copying payload.

### P1 — prefix lookup does not bound mixed-width occupancy

The new `prefix_units` makes `WrapChain::units_before` a binary search
(`history.rs:245-268`), which addresses the predecessor's fragment-prefix
walk.  However, `wrap_occupancy_runs` still iterates every preceding RLE run
(`history.rs:1505-1517`).  Alternating width-one and width-two units create one
run per unit, so the carry-column calculation remains proportional to all
preceding retained units.  Reusing one `seen` buffer removes the prior
per-run allocation but does not satisfy SPEC-010 §7's bounded eager-resize
requirement.

### P1 — blank/hard-broken histories leave a derived index unbounded and may evict source for index pressure

The direct predecessor P0 is corrected: `push_fragment_inner` seals when the
tail's resident allocation reaches the tail threshold, and `evict_to_cap`
force-seals a non-empty tail when no segment exists (`history.rs:883-912,
1001-1027`).  The new 50,000-row test confirms this narrower condition.

But each blank hard-broken row creates a separate `WrapChain` and
`WrapFragment` even with zero units (`history.rs:580-609`).  `wrap_chains` has
no 4 MiB derived-index limit and `derived_cache_bytes` reports only
`reflow_cache` (`history.rs:477-486`).  Its allocation is instead charged to
the 32 MiB canonical resident count (`history.rs:989-999`).  This contradicts
SPEC-010 §6.1 (derived indexes are not resident source content) and §9.1:
derived state is capped at 4 MiB and must be dropped/rebuilt under pressure,
without deleting canonical payload.  Here `evict_to_cap` removes sealed
canonical segments to compensate for wrap-index allocation.  The test neither
crosses the derived cap nor verifies that canonical retention survives
wrap-index pressure.

The eviction trim is also not bounded: `trim_wrap_before_retained` repeatedly
uses `Vec::remove(0)` (`history.rs:668-699`).  Evicting a segment containing
many hard-broken rows shifts the remaining chain vector once per dropped
chain, producing a quadratic index-maintenance path.

### P2 — source Metal ordering is corrected, but history-wide wide-glyph coverage remains absent

Source inspection confirms history encodes all cell backgrounds before its
glyph pass (`MetalTerminalRenderer.swift:1320-1342`), and continuation cells
receive no glyph/wide-glyph flags (`MetalTerminalRenderer.swift:736-765`).
This resolves the predecessor's continuation-overpaint source defect and is
consistent with SPEC-011 §7.2 and §12.1.

There is still no direct history wide-lead-plus-continuation offscreen
regression.  `wideGraphemeOffscreenSelfTest` exercises a live
`NativePreparedFrame` (`RendererValidation.swift:340-393`), while the history
comparison uses a width-one `A` (`RendererValidation.swift:395-472`).

## Closing-gate disposition

The P1 resize/index defects fail the #819 acceptance requirement for bounded
reflow and derived indexes, so the Done gates are not met.  This Linux x86_64
review makes no headed, Swift/FFI, Metal, ARM64 macOS, or physical-performance
execution claim; those issue-required evidence gates are not established here.
