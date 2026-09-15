# M002 #823 macOS headed handoff — exact head `379f8c6`

Use this checklist on an Apple Silicon macOS host with exclusive Runtime ownership.
Do not run concurrently with another worktree that binds the per-user Runtime socket.

## Identity

| Item | Value |
| --- | --- |
| Branch | `issue/823` |
| Exact head | `379f8c655452acc4313f4987d90b875e11337d7d` |
| Relationship | `Refs #823` until headed PASS + independent close GO |
| Linux source status | SOURCE_GO at this head (see `m002-keyboard-823-379f8c6-rereview.md`) |

## Preflight

1. `git fetch origin issue/823 && git checkout issue/823 && git rev-parse HEAD` → must equal `379f8c655452acc4313f4987d90b875e11337d7d`.
2. Confirm no other Seyal Runtime owns the canonical socket (`lsof` / Activity Monitor). If occupied by a stale helper from another issue, stop and reclaim only after verifying ownership.
3. `make check` on Darwin arm64 at the exact head.
4. `cargo test -p seyal-runtime --locked --test pass7_local_ipc` (pre-attach type-29 fatal + unnegotiated V2).
5. Build Debug `Seyal.app` from this head; do not substitute preview fixtures or deleted `SeyalShell*` types.

## Headed six-step matrix (record PASS / FAIL / ENVIRONMENT_UNSUPPORTED per row)

| # | Step | Expected |
| --- | --- | --- |
| 1 | Shell: arrows, Home/End, PageUp/PageDown, Insert/Delete, function keys | Correct bytes in the active shell |
| 2 | Neovim: normal vs application-cursor; modifiers; held-key repeat | Mode-sensitive encoding; no duplicate inject |
| 3 | Target TUI negotiating modern keyboard; supported subset only | Unsupported portions not advertised |
| 4 | Option/Alt both `input.option_as_alt` values; Cmd host shortcuts | Cmd must not leak PTY bytes |
| 5 | Dead-key / IME beside navigation | Commit once; preedit must not leak |
| 6 | Hold keys under high output | No drops outside backpressure contract |

Also capture: key→PTY byte proof for at least one navigation, one modified function key (F3 → `CSI 13;m~`), Escape/Backspace modifier pairs, and held-key overflow rejection (256 distinct held kinds).

## Explicit non-claims

- Host component XCTest 8/8 at this SHA is **not** key→PTY evidence.
- Prior 13-case headed run at ancestor `3d49fbf` is **inconclusive** (shared Runtime socket) and must not be promoted.
- Linux cloud agents must record `ENVIRONMENT_UNSUPPORTED` rather than invent Metal/IME results.

## After PASS

1. Retain screenshots / XCUI / notes under `docs/evidence/` on `issue/823`.
2. Request a fresh independent review of the **exact** headed head.
3. Only then open a write-capable `Refs #823` PR (still not `Closes` until close GO).
