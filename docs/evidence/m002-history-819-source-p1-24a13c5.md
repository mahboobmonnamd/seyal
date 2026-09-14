# #819 source follow-up after Terra re-review of `24a13c5`

Recorded 2026-09-10 on a Linux x86_64 cloud agent. This is not headed
acceptance, macOS FFI/Metal execution, or a performance pass.

## Why this follow-up exists

The independent Terra re-review of `24a13c5` is retained as
`m002-history-819-24a13c5-rereview.md` and remains **NO-GO** for
`Closes #819`. Predecessor sidecar-overflow was fixed. Two P1s
remained: wrap occupancy of a resident-capacity SoftWrap chain, and
history Metal painting a continuation background over a width-two
lead.

## Source changes in this follow-up

- `HistoryStore` keeps SoftWrap-chain width runs. Resize carry columns
  apply those runs instead of replaying every predecessor unit. A
  uniform 8,000-unit SoftWrap chain is one run.
- History Metal draws cell-sized backgrounds and then glyphs, matching
  the live surface, so a continuation instance cannot cover the right
  half of a width-two lead.

## Tests run on this Linux host

```text
cargo test -p seyal-terminal --locked --lib history::tests
cargo test -p seyal-terminal --locked --test history_store_regressions
cargo test -p seyal-protocol --locked
```

macOS Runtime IPC, Swift, and Metal were not executed here.
