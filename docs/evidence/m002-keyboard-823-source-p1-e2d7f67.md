# #823 source follow-up after Terra re-review of `e2d7f67`

Recorded 2026-09-10 on a Linux x86_64 cloud agent. This is not headed
acceptance, native/IME/TUI proof, or a latency pass.

## Why this follow-up exists

The independent Terra re-review of `e2d7f67` is retained as
`m002-keyboard-823-e2d7f67-rereview.md` and remains **NO-GO** for
`Closes #823`. Flags-0 modified keypad release is fixed. Remaining
source finding: modified F3 press used `CSI 1;mR`, which collides
with a cursor-position reply.

## Source changes in this follow-up

- Modified F3 press at flags 0/2 emits `CSI 13;m~` (SPEC-006 §21.6
  decision 4). Unmodified F3 remains `\x1bOR`; flag 1 already used
  the tilde form.

The exhaustive §21.6 matrix remains open (P3).

## Tests

`cargo test -p seyal-runtime --locked --lib -- key_v2_encode`
