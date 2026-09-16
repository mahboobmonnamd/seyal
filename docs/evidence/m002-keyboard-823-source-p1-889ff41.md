# #823 source follow-up after Terra re-review of `889ff41`

Recorded 2026-09-10 on a Linux x86_64 cloud agent. This is not headed
acceptance, native/IME/TUI proof, or a latency pass.

## Why this follow-up exists

The independent Terra re-review of `889ff41` is retained as
`m002-keyboard-823-889ff41-rereview.md` and remains **NO-GO** for
`Closes #823`. The prior Alt+Escape / Alt+Control+Backspace bytes were
fixed. Remaining source findings: legacy Shift/Control Escape was
rejected, and modified application-keypad repeat/release at flag 2
could emit after an unsupported press.

## Source changes in this follow-up

- Legacy Escape with any accepted modifier still emits `ESC`, or
  `ESC ESC` when Alt is present.
- Without flag 1, a modified keypad is rejected for press, repeat, and
  release.

The exhaustive §21.6 matrix remains open (P3) and is not treated as a
closing gate from this Linux host.

## Tests

`cargo test -p seyal-runtime --locked --lib -- key_v2_encode` executes
the encoder cases, including Shift/Control Escape and modified keypad
events. macOS IPC, Swift, headed, IME, and latency were not run.
