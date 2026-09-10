# #823 V2 Error correlation after Terra re-review of `b681676`

Recorded 2026-09-10 on a Linux x86_64 cloud agent. This is not headed
acceptance, native/IME/TUI proof, or a latency pass.

## Why this follow-up exists

The independent Terra re-review of `b681676` is retained as
`m002-keyboard-823-independent-rereview.md` and remains **NO-GO** for
`Closes #823`. The Backpressure subset was incomplete against SPEC-006
§21.5.

## Source changes in this follow-up

- Every type-29 Error with a nonzero `detail_code` is validated against
  connection-local sent/highest-error bounds, not only `Backpressure`.
- Zero-detail type-29 `Backpressure` / authorization codes are
  `ClientError::Protocol`. Zero-detail `MalformedPayload` stays the bounded
  generic fatal for unreadable V2 frames.
- In-range `Backpressure` and unsupported encoding (`MalformedPayload`)
  remain `ClientBackpressure`. In-range `PermissionDenied` /
  `StaleIdentity` / `InvalidExecution` remain `LostController` without
  tearing the connection.
- `last_admitted_v2_action_id` advances on outbound enqueue.
  `last_sent_v2_action_id` advances only after the TerminalKeyV2 frame is
  fully written. Errors for not-yet-wire-complete IDs are protocol
  failures.
- Held-key capacity overflow now reports a visible client-backpressure
  failure. A repeat of an already-tracked key at capacity is still sent.

Classification is implemented in `crates/seyal-client/src/v2_error.rs` so
the Linux host can execute the table without the macOS client module.

## Tests run on this Linux host

```text
cargo test -p seyal-client --locked --lib
# 10 passed, including the three v2_error classification tests

cargo fmt -p seyal-client
```

macOS `LocalDisplayClient` socket/WouldBlock tests, native/FFI, headed
keyboard, IME, and latency gates were not run here.
