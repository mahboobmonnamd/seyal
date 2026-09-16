# M002 #823 independent source re-review — `379f8c6`

| Field | Value |
| --- | --- |
| Exact head | `379f8c655452acc4313f4987d90b875e11337d7d` |
| Branch | `issue/823` |
| Date | 2026-09-15 |
| Reviewer role | Independent source review on a Linux cloud agent (not the original thin-host implementer) |
| Relationship | `Refs #823` — never `Closes #823` |

## Verdict

| Gate | Result |
| --- | --- |
| Source | **SOURCE_GO** |
| Close / merge | **CLOSE_NO_GO** |

Prior `CHANGES_REQUIRED` at `dce62a5` (rustfmt on §21.6 fixtures; silent held-key overflow) is cleared by `3d49fbf` + `379f8c6`. No P0–P2 source reopeners were found at this head.

## Portable evidence retained on this host

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | pass |
| `cargo test -p seyal-runtime --locked --lib -- key_v2` | 13/13 |
| `cargo test -p seyal-terminal --locked --test m002_keyboard` | 5/5 |
| `cargo test -p seyal-client --locked --lib -- input_policy` | 4/4 |
| `cargo test -p seyal-protocol --locked --lib` | 30/30 |
| `cargo test -p seyal-terminal --locked --lib` | 55/55 |
| `cargo clippy -p seyal-client -p seyal-runtime --locked -- -D warnings` | clean |
| `python3 scripts/check-thin-swift-boundary.py` | pass |

macOS-gated `pass7_local_ipc` pre-attach type-29 cases and headed key→PTY were not executed here.

## Findings

### P0 / P1 / P2

None.

### P3 (non-blocking for source GO)

1. Held-key overflow announces via Accessibility only; it does not also set `InputAdmissionFailure` / `seyal_bridge_input_failure`. Structural reject (no track / no submit; tracked repeats continue) is correct.
2. `process_input_policy()` discards warning strings that unit tests prove are produced for invalid keys.
3. Pre-attach type-29 fatal coverage lives in `pass7_local_ipc` under `cfg(macos)` and cannot be re-proven on Linux.

## Disposition of prior CHANGES_REQUIRED

| Prior blocker at `dce62a5` | Disposition at `379f8c6` |
| --- | --- |
| rustfmt failure in §21.6 fixtures | Fixed in `3d49fbf`; `cargo fmt --all -- --check` green |
| Silent held-key overflow / risk of dropping tracked repeats | Fixed in `3d49fbf`: new press → `rejectHeldKeyOverflow()`; tracked keys still admit repeats |
| Rust host / UI test compile break | Fixed in `379f8c6` |

## Remaining for `Closes #823`

1. Exclusive-Runtime headed six-step matrix on this exact head (shell / Neovim DECCKM / modern-keyboard TUI / Option-as-Alt / IME / hold-under-load).
2. Fresh independent headed + native GO after that evidence.
3. Latency / physical perf remains `#673` / `#824` authority (`performance_claim` stays false).

Do not open a closing PR from this Linux agent. Keep `Refs #823` until headed PASS + independent close GO.
