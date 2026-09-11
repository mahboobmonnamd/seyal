# #819 source P1/P2 at `2d04f6d`

Recorded 2026-09-10 on a Linux x86_64 cloud agent. This is not headed
acceptance and is not a performance pass.

## Exact head

```text
2d04f6d0d0df1d4d83e2e291a98405b63e23b598
fix(history): present multi-scalar units on the history wire and reflow the retained/active boundary
```

## Source changes at this SHA

- History range snapshots encode a length-prefixed UTF-8 sidecar. Header
  bytes 28..32 are `sidecar_len`. `sidecar_len == 0` keeps the M001 layout
  (`HistoryCell.reserved == 0`). Combining/ZWJ units set flag bit 7 and a
  sidecar offset instead of `HistoryRangeError::Unrepresentable`.
- Runtime packs `primary_history_wire_range` into that snapshot. The native
  bridge exposes `seyal_bridge_history_range_sidecar_for`; Metal history
  prepare uses `lookupGrapheme` when sidecar UTF-8 is present.
- Column/row-shrink resize rebuilds one canonical stream from retained history
  plus active source, then **replaces** history with the prefix that does not
  fit. Units that return to the viewport are admitted into the grapheme store
  only at commit so a dropped prepare cannot leak store IDs.
- Segment seal uses canonical UTF-8 payload only (SPEC-010 §5.1). Derived
  cache accounting uses inner `Vec` capacity. `source_breaks` are removed when
  a line is appended to history. Evicted ID range metadata is capped at 1024
  ranges; overflow folds only the oldest range into `evicted_through` and
  does not coarsen later live identities across alternate-screen gaps.

## Tests run on this Linux host

```text
cargo test -p seyal-protocol --locked
# including history_snapshot_round_trips_combining_grapheme_sidecar

cargo test -p seyal-terminal --locked
# 38 unit tests, 18 HistoryStore regressions including
# resize_reflows_soft_wrap_across_retained_and_active_boundary
# and primary_history_range_retains_combining_grapheme_rows

cargo test -p seyal-exec --locked
cargo test -p seyal-runtime --locked --lib
```

Not executed here: macOS `seyal-client` FFI, `pass7_local_ipc` (cfg macOS),
`make check`, native smoke, headed five-step checklist, ARM64 Release
resource matrix vs an accepted #673 contract.

## Closing keyword

Do **not** use `Closes #819` until independent GO and the headed five-step
PASS exist. This note is `Refs #819` evidence only.
