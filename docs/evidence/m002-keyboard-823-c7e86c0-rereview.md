# Independent closing re-review — M002 keyboard architecture, issue #823

## Verdict

**NO-GO for a closing merge of #823.**

- Candidate worktree: `/tmp/seyal-oss-work/issue-823-keyboard-architecture`
- Candidate branch: `issue/823`
- Exact SHA reviewed: `c7e86c02142cf703ba6e4aa30e90c39c0266cc6a`
- Subject: `fix(823): fatal pre-attach V2, route ETB/Escape, and stack set/clear`
- Comparison: `origin/master...HEAD` (`origin/master` = `9b8408506ee2137c4b0cb20b4b2b2a92549649ab`; merge base = `43e443ee93f6e441d3b1658dcc6675beff6bccf2`)
- Review host: Linux `6.12.94+` x86_64
- Scope: independent source/evidence review. No candidate-tracked or production file was modified.

`HEAD` matched the requested SHA and subject both before and after review. The candidate worktree was clean. `git diff --check origin/master...HEAD` exited 0.

Authority reviewed: `AGENTS.md`; [issue #823](https://github.com/mahboobmonnamd/seyal/issues/823); and SPEC-006 §§21.3 and 21.5–21.7. The issue itself remains `OPEN`, declares its state `Blocked`, and has no completed acceptance checkbox. That alone precludes a closing handoff.

## Findings

### P0

None found.

### P1 — legacy Alt+Escape is rejected, and Alt+Control+Backspace loses its Alt prefix

**Files:** `crates/seyal-runtime/src/runtime/local/ingress.rs:146-463` (especially `157-182`, `314-352`, and `458-462`); native production route in `macos/Seyal/Sources/TerminalInputSurface.swift:539-559`.

SPEC-006 §21.2 requires legacy Escape to encode as `ESC`, or `ESC ESC` with Alt. It also says Alt prefixes the legacy Enter/Tab/Backspace result, while Control+Backspace produces `BS`.

For a valid V2 Alt+Escape press while flags are 0 or 2, `encode_terminal_key_v2` reaches the `modifiers != 0` branch. That match has no `Escape` arm and returns `Err(())`. `handle_terminal_key_v2` converts it to a type-15 `MalformedPayload` rejection, so the PTY receives no required `ESC ESC`.

For valid V2 Alt+Control+Backspace, the Enter/Tab/Backspace branch first prefixes `DEL` with Alt, then its Control-Backspace clause overwrites the entire output with `BS`. The required result is `ESC BS`; the implementation emits only `BS`.

The c7e86c0 regression test covers unmodified Escape, flag-1 Escape, Shift-Tab, Alt-Enter, Control-Enter, and Control-Backspace, but neither failing combination. This is a direct standard/modern keyboard compatibility failure in the advertised M002 matrix.

### P2

None found beyond the P1 encoding failure above.

### P3 — the required exhaustive V2 matrix remains absent

**Files:** `crates/seyal-runtime/src/runtime/local/ingress.rs:833-1149`; `crates/seyal-terminal/tests/m002_keyboard.rs`; `crates/seyal-protocol/tests/pass7_input_resize.rs`; `macos/Seyal/Sources/TerminalInputSurface.swift:1176-1295`.

SPEC-006 §21.6 requires fixtures over every V2 kind × flags `0/1/2/3` × press/repeat/release × accepted Shift/Alt/Control combination, and every printable-ASCII shifted-field outcome. The portable terminal target contains four parser/state tests; the protocol integration target contains one V2 positive fixture and limited malformed cases. Runtime unit tests are selected cases, and the macOS self-test is likewise selected classifier properties. The missing cases include the P1 combinations above. There is no exhaustive, externally expected decision-table fixture.

## Audit of the predecessor P1 findings

| Predecessor P1 | Status at c7e86c0 | Evidence |
| --- | --- | --- |
| Type-29 sent before attachment was nonfatal | **Fixed in source.** | `crates/seyal-runtime/src/runtime/local/session.rs:47-55` now routes invalid `TerminalKeyV2` in `AwaitHello`/`Ready` through `fatal_terminal_key_v2`; `ingress.rs:466-479` queues at most the generic error and sets `close_after_flush`. New cases are in `crates/seyal-runtime/tests/pass7_local_ipc.rs:329-342`. |
| Native V2 classifier omitted Enter/Tab/Backspace and Escape | **Fixed in source.** | `macos/Seyal/Sources/TerminalInputSurface.swift:249-283` now creates V2 intents for Enter, Tab/back-tab, Backspace, and Escape, retaining modifiers for Runtime encoding. `semanticKeyMatrixSelfTest` adds selected checks at `1202-1225`. This repair does not fix the new Runtime P1 for valid Alt+Escape / Alt+Control+Backspace bytes. |
| Kitty set/clear did not create/update a screen-local stack value | **Fixed and portable-test covered.** | `crates/seyal-terminal/src/terminal.rs:500-519` creates a base entry when empty and replaces the current entry otherwise. `kitty_set_creates_a_base_stack_entry_restored_by_pop` passed in `crates/seyal-terminal/tests/m002_keyboard.rs:55-64`. |

The predecessor P2 action-ID exhaustion is also fixed in source: `TerminalInputSurface.swift:854-868` stops before zero/wrap and calls the existing recovery path. The predecessor FFI/shape P3 is fixed in source: `crates/seyal-client/src/local/input_resize.rs:321-332` performs `TerminalKeyV2::validate()` before encoding, so `ffi/input.rs` cannot rely only on `encode`'s debug assertion. Neither source repair is native-runtime evidence on this host.

## SPEC-006 §21.7 evidence-gate audit

| Gate | Disposition for c7e86c0 |
| --- | --- |
| Modes | **Partial automated coverage.** The four portable terminal tests exercise DEC 1/66, `ESC =/>`, masking, a stack base restore, screen-local flags, and deferred `RIS`. They do not cover the required full set/reset/query and `push 17` / `pop 0/1/16/65535` matrix. |
| Key bytes | **Blocked.** Selected Runtime source tests exist, but the P1 proves required legacy modifier bytes are wrong and the complete table is untested. |
| Negotiation | **Partial.** V2 type/capability/wire round-trip checks pass, and source has pre-attach fatal routing. No portable execution of the macOS-gated Runtime IPC cases occurred. |
| Modern events | **Missing.** No exact-head real Neovim flags-3 handshake, Control-I versus Tab, repeated-arrow, release, and Enter/Tab/Backspace-exception evidence exists. |
| Wire/security | **Partial.** Protocol framing checks and source validation are present. The portable V2 cases are narrow; V2 observer/stale/detach and error-output-capacity behavior did not execute on this host. |
| Admission/recovery | **Incomplete.** No exact-head real Runtime/PTY evidence covers output-mode change between admission/write, partial writes, saturation, persistent pressure/fairness, focus/detach/reconnect, or the V2 P1 regression. |
| Native | **Missing.** Linux has no Debug `Seyal.app`, Metal, HID keyboard, keypad, physical layouts, or IME. The retained headed ledger is `ENVIRONMENT_UNSUPPORTED` and names older heads; it is not a pass for c7e86c0. |
| Fuzz/property | **Missing as exact-head completion evidence.** The retained 45-second decoder campaign is for `9848add`, not this SHA, and the sole V2 fuzz target decodes frames rather than proving the required encoder/admission decision matrix. |
| Performance | **Missing.** There is no exact baseline/candidate Release ARM64, three-run, 1,000-action native-key→Runtime/key→PTY percentile, CPU/RSS, queue-high-water, and rejected/deferred-count record. Linux reports `performance_claim=false`; that is not a passing result. |
| Required build/test/check | **Partial.** The focused portable commands below passed. No exact-head `make build`, `make test`, or `make check` result, and no exact-head native build/XCUI result, was established by this review. |
| Independent review | **Completed, NO-GO.** The unresolved P1 and unmet mandatory evidence gates block closure. |

## Verification performed

```text
git -C /tmp/seyal-oss-work/issue-823-keyboard-architecture rev-parse HEAD
PASS — c7e86c02142cf703ba6e4aa30e90c39c0266cc6a

git -C /tmp/seyal-oss-work/issue-823-keyboard-architecture log -1 --oneline
PASS — c7e86c0 fix(823): fatal pre-attach V2, route ETB/Escape, and stack set/clear

git -C /tmp/seyal-oss-work/issue-823-keyboard-architecture diff --check origin/master...HEAD
PASS — exit 0

cargo test -p seyal-terminal --locked --test m002_keyboard --manifest-path /tmp/seyal-oss-work/issue-823-keyboard-architecture/Cargo.toml
PASS — 4 passed

cargo test -p seyal-protocol --locked --manifest-path /tmp/seyal-oss-work/issue-823-keyboard-architecture/Cargo.toml
PASS — 26 unit tests and 8 pass7_input_resize integration tests passed.
CFG/TEST NOTE — pass7_fuzz_smoke and pass8_fuzz_smoke each had 1 ignored test.

cargo test -p seyal-client --locked --lib --manifest-path /tmp/seyal-oss-work/issue-823-keyboard-architecture/Cargo.toml
PASS — 10 passed, including 3 portable v2_error tests.
CFG SKIP — `crates/seyal-client/src/lib.rs:17-21` gates `local` (including client IPC/FFI admission) to macOS, so it did not compile or execute here.

cargo test -p seyal-runtime --locked --test pass7_local_ipc --manifest-path /tmp/seyal-oss-work/issue-823-keyboard-architecture/Cargo.toml
CFG SKIP — command exited 0 with 0 tests because `crates/seyal-runtime/tests/pass7_local_ipc.rs:1` is `#![cfg(target_os = "macos")]`.
```

No macOS IPC, Swift, encoder, Debug-app, Metal, HID, IME, XCUI, physical-layout/keypad, target-TUI, or performance test was run or claimed.

## Remaining source versus evidence-only gaps

**Source gaps:** the P1 legacy encoder defects and P3 exhaustive-matrix absence above. The issue's own acceptance remains unchecked.

**Evidence-only gaps:** exact-head real Runtime/PTY adversarial tests; native/XCUI shell and Neovim exercise; physical keyboard/layout/keypad and dead-key/IME observations; repeat under high output; target modern-protocol negotiation; exact-head fuzz/property campaign; and the required controlled ARM64 latency/resource comparison. Existing retained evidence predates this candidate or explicitly records an unsupported Linux environment, so it cannot close these rows.

## Closing disposition

Keep #823 open. Correct the P1 bytes, add the required exhaustive matrix and regressions, then establish all exact-head §21.7 evidence on the required macOS/Apple-Silicon native environment before another independent closing review.
