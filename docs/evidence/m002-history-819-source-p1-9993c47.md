# #819 source follow-up after wrap-index head `9993c47`

Recorded 2026-09-10 on a Linux x86_64 cloud agent. This is not headed
acceptance, macOS FFI/Metal execution, or a performance pass.

## Why this follow-up exists

Period-2 occupancy used the first two run widths with forced count 1,
so a repeating `(1×2, 2×1)` pattern encoded as `1,2,1,2…` instead of
`1,1,2,1,1,2…`. Repeating occupancy now consumes the stored RLE counts
without expanding them into per-unit work.

## Tests

```text
cargo test -p seyal-terminal --locked --lib history::tests
cargo test -p seyal-terminal --locked --test history_store_regressions
```
