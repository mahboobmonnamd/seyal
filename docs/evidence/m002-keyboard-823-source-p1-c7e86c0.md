# #823 source follow-up after Terra re-review of `c7e86c0`

Recorded 2026-09-10 on a Linux x86_64 cloud agent. This is not headed
acceptance, native/IME/TUI proof, or a latency pass.

## Why this follow-up exists

The independent Terra re-review of `c7e86c0` is retained as
`m002-keyboard-823-c7e86c0-rereview.md` and remains **NO-GO** for
`Closes #823`. Predecessor P1s were fixed. Legacy Alt+Escape was
rejected, and Alt+Control+Backspace dropped the Alt prefix.

## Source changes in this follow-up

- Legacy Alt+Escape (flags 0 or 2) encodes `ESC ESC`. Flag 1 still uses
  `CSI 27;m u`.
- Enter/Tab/Backspace apply Control-Backspace (`BS`) before the Alt
  prefix, so Alt+Control+Backspace is `ESC BS`.
- `encode_terminal_key_v2` lives in portable `seyal-runtime`
  `key_v2_encode` so the encoder table executes on Linux. Admission and
  PTY submit remain in macOS-only local ingress.

The exhaustive §21.6 matrix remains open (P3) and is not treated as a
closing gate from this Linux host.

## Tests

`cargo test -p seyal-runtime --locked --lib -- key_v2_encode` executes
the encoder cases, including Alt+Escape and Alt+Control+Backspace.
Portable `m002_keyboard`, protocol, and client lib tests still run here.
macOS IPC, Swift, headed, IME, and latency were not run.
