# #823 source follow-up after Terra re-review of `641f740`

Recorded 2026-09-10 on a Linux x86_64 cloud agent. This is not headed
acceptance, native/IME/TUI proof, or a latency pass.

## Why this follow-up exists

The independent Terra re-review of `641f740` is retained as
`m002-keyboard-823-641f740-rereview.md` and remains **NO-GO** for
`Closes #823`. Shift/Control Escape and flag-2 modified keypad events
were fixed. Remaining source finding: flags-0 modified keypad
release returned a successful no-byte result before the rejection
guard.

## Source changes in this follow-up

- Modified keypad without flag 1 is rejected before the flags-0
  release early return, covering numeric and application keypad.

The exhaustive §21.6 matrix remains open (P3).

## Tests

`cargo test -p seyal-runtime --locked --lib -- key_v2_encode`
