# #819 source follow-up after Terra re-review of `af58c16`

Recorded 2026-09-10 on a Linux x86_64 cloud agent. This is not headed
acceptance, macOS FFI/Metal execution, or a performance pass.

## Why this follow-up exists

The independent Terra re-review of `af58c16` is retained as
`m002-history-819-af58c16-rereview.md` and remains **NO-GO** for
`Closes #819`. Three P1 source defects remained.

## Source changes in this follow-up

- `wrap_column_before` walks only the current SoftWrap chain from the
  suffix cut, not every older hard-broken retained record.
- Sidecar overflow keeps the already-packed prefix of the overflowing
  row instead of truncating that sidecar and emitting a zero-lead
  `Truncated` snapshot.
- History wire cells carry continuation and terminal width in `flags`.
  The history Metal path sets the wide-glyph instance flag, skips
  continuation glyphs, and the shader doubles the rectangle in history
  render mode as well as live mode.

## Tests run on this Linux host

```text
cargo test -p seyal-protocol --locked
cargo test -p seyal-terminal --locked --lib
cargo test -p seyal-terminal --locked --test history_store_regressions
```

macOS Runtime IPC, Swift, and Metal were not executed here.
