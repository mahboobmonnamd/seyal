# #823 Rust rebase checkpoint (source path)

## Identity

| Item | Value |
| --- | --- |
| Canonical branch | `issue/823` (prefer over `issue/823-keyboard-architecture`) |
| Exact head | `c84a613` (production tip after rebase; this docs commit is tip of branch) |
| Base | `origin/master` @ `485bdf5` |
| Relationship | `Refs #823` — do **not** use `Closes #823` |

## Status

Prior Terra review of ancestor `a9d7f51` was **source GO / close NO-GO** (headed,
native, IME, fuzz, performance still open). This checkpoint only records that
the branch was **rebased onto current master** and focused portable Rust suites
still pass on Linux. It does **not** renew independent review GO — re-review
the new tip before merge.

## Focused Rust verification

```text
cargo test -p seyal-runtime --locked --lib -- key_v2
# 10 passed

cargo test -p seyal-client --locked --lib -- v2_error
# 3 passed
```

## Still open (blocks Closes #823)

- [ ] Independent re-review GO on this exact head (post-rebase)
- [ ] Headed keyboard / XCUI / target-TUI on macOS (exclusive Runtime)
- [ ] Broad latency / physical perf → #673/#824 (C-gates), not per-commit here

## Parallelism note

Rust-only work on `issue/823` may proceed while `issue/819` holds no Runtime.
Do not run native/Pass8/XCUI concurrently with another worktree.
