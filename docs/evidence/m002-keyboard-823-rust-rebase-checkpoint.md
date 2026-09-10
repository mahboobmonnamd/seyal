# #823 Rust A-gate checkpoint (Linux)

## Identity

| Item | Value |
| --- | --- |
| Canonical branch | `issue/823` (prefer over `issue/823-keyboard-architecture`) |
| Base | `origin/master` @ `485bdf5` |
| Relationship | `Refs #823` — do **not** use `Closes #823` |

## Status

Prior Terra review of ancestor `a9d7f51` was **source GO / close NO-GO** (headed,
native, IME, fuzz, performance still open). This checkpoint records the next
Linux-safe A-gate: the SPEC-006 §21.6 exhaustive encoder fixture matrix is now
committed and green. It does **not** renew independent review GO — re-review
this tip before merge.

## What landed

- `key_v2_section_21_6_tests.rs` enumerates all 36,384 validated rows
  (kinds × flags 0/1/2/3 × press/repeat/release × modifiers × DECCKM × DECNKM).
- Each row is classified as exact bytes, successful no-byte, or unsupported.
- A pinned FNV-1a digest guards silent table drift.
- SPEC prose examples and kind-17 invalid shifted-field negatives are covered.
- Protocol wire/security framing negatives for `TerminalKeyV2` (SPEC-006 §21.7):
  lengths 0–39 and 41+, unknown kind/event/version/modifier bits, reserved pads,
  bad scalars/values, zero `action_id`, and dispatcher exact-length rejection.
- Kitty flag stack: 17-push oldest-eviction, saturating pop, and set-on-empty base
  restore (`m002_keyboard`).

## Focused Rust verification

```text
cargo test -p seyal-runtime --locked --lib -- key_v2
# 13 passed (10 prior + 3 §21.6)

cargo test -p seyal-protocol --locked --test pass7_input_resize
# 10 passed (includes V2 wire/security framing negatives)

cargo test -p seyal-terminal --locked --test m002_keyboard
# 5 passed (modes + flag-stack eviction)

cargo test -p seyal-client --locked --lib -- v2_error
# 3 passed
```

## Still open (blocks Closes #823)

- [ ] Independent re-review GO on this exact head
- [ ] Headed keyboard / XCUI / target-TUI on macOS (exclusive Runtime)
- [ ] Runtime local-IPC admission cases (`pass7_local_ipc` is `cfg(macos)`)
- [ ] Fuzz campaigns beyond the committed matrix
- [ ] Broad latency / physical perf → #673/#824 (C-gates)

## Parallelism note

Rust-only work on `issue/823` may proceed while Mac is unavailable.
Do not run native/Pass8/XCUI concurrently with another worktree.
