# #823 source follow-up after Terra re-review of `2694b99`

Recorded 2026-09-10 on a Linux x86_64 cloud agent. This is not headed
acceptance, native/IME/TUI proof, or a latency pass.

## Why this follow-up exists

The independent Terra re-review of `2694b99` is retained as
`m002-keyboard-823-2694b99-rereview.md` and remains **NO-GO** for
`Closes #823`. Predecessor type-29 correlation P1s were fixed in that
SHA. Three new P1 source defects remained, plus a P2 action-ID
exhaustion hole.

## Source changes in this follow-up

- Pre-attachment type-29 frames (`AwaitHello` and `Ready`) now take the
  existing `fatal_terminal_key_v2` path instead of a non-fatal
  `InvalidState` error.
- Native V2 classification carries Enter, Tab (including Shift-Tab /
  backTab), Backspace, and Escape, including Shift/Alt/Control variants.
  Unmodified printable text still stays off the V2 ASCII kind.
- `CSI = flags;mode u` creates or updates the current per-screen Kitty
  stack entry so `set`, then `push`, then `pop` restores the set value.
- V2 action IDs stop before wrapping (`0` or `u32::MAX`). The surface
  reports a visible failure and stops the attachment so the existing
  recovery coordinator can open a fresh connection. A disconnect
  resets the local ID counter to 1.
- Client `submit_terminal_key_v2` validates the full V2 shape before
  encode so a release FFI path cannot send a malformed frame.

## Tests run on this Linux host

```text
cargo test -p seyal-terminal --locked --test m002_keyboard
# includes kitty_set_creates_a_base_stack_entry_restored_by_pop

cargo test -p seyal-protocol --locked
cargo test -p seyal-client --locked --lib
```

macOS `pass7_local_ipc`, Swift self-tests, and Runtime encoder unit tests
are cfg-gated and were not executed here.
