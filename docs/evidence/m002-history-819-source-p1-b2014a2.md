# #819 source follow-up after Terra re-review of `b2014a2`

Recorded 2026-09-10 on a Linux x86_64 cloud agent. This is not headed
acceptance, macOS FFI/Metal execution, or a performance pass.

## Why this follow-up exists

The independent Terra re-review of `b2014a2` is retained as
`m002-history-819-b2014a2-rereview.md` and remains **NO-GO** for
`Closes #819`. Blank tails now seal. Remaining source findings: suffix
trim scanned retained wrap fragments/runs; mixed-width occupancy walked
every predecessor run; wrap index was charged as resident source and could
evict canonical payload; `Vec::remove(0)` was quadratic.

## Source changes in this follow-up

- Wrap index is a SPEC-010 §9.1 derived index, not resident source.
  Closed hard-broken rows are omitted. Closed chains drop under the 4 MiB
  derived cap without deleting payload.
- Suffix trim uses `partition_point`/`truncate` and pops runs from the end.
  Prefix trim uses `pop_front`/`drain`.
- Alternating mixed-width occupancy uses the repeating-pattern path.

History-wide Metal wide-glyph coverage remains an unmet P2. The exhaustive
headed/performance gates remain open.

## Tests run on this Linux host

```text
cargo test -p seyal-terminal --locked --lib history::tests
cargo test -p seyal-terminal --locked --test history_store_regressions
```

macOS Runtime IPC, Swift, and Metal were not executed here.
