# #819 lazy resize and history-wire continuation after Terra re-review of `2d04f6d`

Recorded 2026-09-10 on a Linux x86_64 cloud agent. This is not headed
acceptance, macOS FFI execution, or a performance pass.

## Why this follow-up exists

The independent Terra re-review of `2d04f6d` is retained as
`m002-history-819-independent-rereview.md` and remains **NO-GO** for
`Closes #819`. Two P1 source defects remained: eager whole-history
resize/reflow, and an unrecoverable sidecar-truncated multi-scalar row.

## Source changes in this follow-up

- `Screen::prepare_resize` clones only the near-visible history suffix
  (viewport plus two screenfuls of slack) instead of reconstructing the
  entire 32 MiB store into a temporary reflow. Older sealed source stays in
  place via `truncate_from` rather than `replace_payload` of the whole
  transcript.
- Hard-broken history does not walk prefix wrap occupancy. A mid-chain
  budget stop still computes start-column without cloning prefix units.
- History range requests carry `start_unit` in previously reserved bytes
  56..60 (`0` remains the beginning of the line range). Runtime/VT skip
  that many lead cells before packing.
- `SeyalHistoryRange.reserved` now carries `HistoryRangeStatus`. The
  native bridge continues a `Truncated` response instead of dropping the
  unread suffix.

## Tests run on this Linux host

```text
cargo test -p seyal-terminal --locked --lib
# 39 passed, including eager_resize_suffix_is_bounded_for_hard_broken_history

cargo test -p seyal-terminal --locked --test history_store_regressions
# 20 passed, including resize_reflows_soft_wrap_across_retained_and_active_boundary,
# resize_keeps_early_hard_broken_history_without_rewriting_it, and
# history_wire_skip_leads_returns_the_unconsumed_suffix

cargo test -p seyal-protocol --locked --lib
# 27 passed, including history_range_request_rejects_unbounded_or_reversed_ranges
```

macOS client/FFI, Runtime `pass7_local_ipc`, headed history/reflow, and
physical ARM64 performance were not run here.
