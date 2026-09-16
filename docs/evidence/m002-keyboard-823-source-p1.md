# #823 V2 error-ID correlation at `b681676`

Recorded 2026-09-10 on a Linux x86_64 cloud agent. This is not headed
acceptance and is not a latency pass.

## Exact head

```text
b6816764467ba7e2e0490f78f7a2744e517ec768
fix(823): correlate TerminalKeyV2 backpressure to sent action IDs
```

Predecessor source guards remain in `f24e2a4` (type-29 Backpressure as
recoverable admission; refuse V2 when extended-key is not negotiated or
`action_id == 0`).

## Source changes at this SHA

`LocalDisplayClient` retains connection-local `last_sent_v2_action_id` and
`highest_v2_error_id` without an unbounded pending map:

- `submit_terminal_key_v2` rejects `action_id <= last_sent_v2_action_id`
  and records last-sent after the frame is admitted.
- In-range type-29 Backpressure (`detail_code != 0`) is
  `ClientBackpressure`.
- Out-of-range, duplicate, or non-monotonic V2 error IDs are
  `ClientError::Protocol`.
- `detail_code == 0` remains `Server(Backpressure)`.

## Tests run on this Linux host

`seyal-client::local` is `#[cfg(target_os = "macos")]`. The new
`v2_backpressure_uses_sent_and_highest_error_bounds` test was **not**
executed here.

## Closing keyword

Do **not** use `Closes #823`. Headed six-step, native latency, and
independent GO remain open. This note is `Refs #823` evidence only.
