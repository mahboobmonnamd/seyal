# #819 source follow-up after Terra re-review of `656d55a`

Recorded 2026-09-10 on a Linux x86_64 cloud agent. This is not headed
acceptance, macOS FFI/Metal execution, or a performance pass.

## Why this follow-up exists

The independent Terra re-review of `656d55a` is retained as
`m002-history-819-656d55a-rereview.md` and remains **NO-GO** for
`Closes #819`. Predecessor Metal two-pass and uniform-run occupancy
math were fixed. Remaining source findings: blank rows never sealed;
`units_before` walked every predecessor fragment; resize/evict rebuilt
the wrap index by cloning retained UTF-8.

## Source changes in this follow-up

- Empty retained rows seal when tail resident bytes reach the mutable
  tail ceiling, and eviction force-seals a non-empty tail if no
  segment exists.
- Wrap fragments store prefix unit counts so carry-column lookup is
  a binary search. Truncate and eviction trim the wrap index instead of
  cloning canonical payload.

## Tests run on this Linux host

```text
cargo test -p seyal-terminal --locked --lib history::tests
cargo test -p seyal-terminal --locked --test history_store_regressions
```

macOS Runtime IPC, Swift, and Metal were not executed here.
