# Independent closing re-review — M002 #823 keyboard architecture

## Verdict

**NO-GO for a closing #823 merge.**

- Candidate worktree: `/tmp/seyal-oss-work/issue-823-keyboard-architecture`
- Candidate branch: `issue/823`
- Exact SHA reviewed: `2694b99d9ff28615b484f6d61d44b226ec7486b2` — `fix(823): correlate every type-29 V2 error to wire-complete IDs`
- Comparison: `origin/master...HEAD`, with `origin/master` at `9b8408506ee2137c4b0cb20b4b2b2a92549649ab`
- Review host: Linux `6.12.94+` x86_64
- Scope: independent source/evidence review only. No candidate-tracked or production file was modified.

`HEAD` and its one-line subject matched the requested candidate before and after review. `git diff --check origin/master...HEAD` exited successfully. The candidate worktree was clean at final inspection.

The predecessor evidence files and the implementer note were read only as context. They are not credited as evidence for this SHA.

Authority reviewed: `AGENTS.md`; [issue #823](https://github.com/mahboobmonnamd/seyal/issues/823); and `docs/specs/SPEC-006-M001-NATIVE-INPUT-RESIZE.md` §§21.3 and 21.5–21.7.

## Findings

### P0

None found.

### P1 — unnegotiated type-29 frames before attachment are not connection-fatal

**Files:** `crates/seyal-runtime/src/runtime/local/session.rs:38-49`, `crates/seyal-runtime/src/runtime/local/ingress.rs:466-479`, `crates/seyal-runtime/src/local_ipc/connection.rs:115-125`

SPEC-006 §21.5 requires unnegotiated V2 to be connection-fatal after at most one bounded generic error. The fatal route is correctly used after an attached client fails the capability check, but the dispatcher never reaches that route before attachment:

1. In `AwaitHello` and `Ready`, `TerminalKeyV2` is not in `ConnectionState::validate_incoming`'s allowlist.
2. `dispatch_local_ipc_frame` therefore sends `InvalidState` with zero detail and returns.
3. It does not call `fatal_terminal_key_v2` and does not set `close_after_flush`.

A peer can consequently send a type-29 frame before Hello or between Hello and Attach, receive an error, and keep the same connection open. The existing `unnegotiated_v2_key_closes_connection_after_bounded_error` test sends after attachment, so it does not cover this state-machine path. This violates the explicit fatal framing rule.

### P1 — the native V2 route omits Escape and all modified Enter/Tab/Backspace cases

**Files:** `macos/Seyal/Sources/TerminalInputSurface.swift:221-276,490-555`, `crates/seyal-runtime/src/runtime/local/ingress.rs:95-144,146-183`

The Runtime V2 encoder implements the specified modern/legacy exceptions when it receives a `TerminalKeyV2`. The production Swift classifier prevents that route for required keys:

- `TerminalNativeKeyClassifier.v2` explicitly returns `nil` for Enter, Tab, and Backspace.
- It has no Escape case; its fallback requires printable ASCII, so Escape also cannot produce a V2 intent.
- The fallback `semanticKey` path accepts only Caps Lock in addition to the key. It cannot represent Shift/Alt/Control variants, and delivers only the old unmodified `TerminalKey` shape.

Thus V2 cannot carry Shift-Tab, Alt-Enter, Control-Enter, Control-Backspace, or their required legacy-exception behavior. It also cannot carry Escape to the Runtime when flag 1 is negotiated, so the old V1 path produces literal `ESC` rather than the required `CSI 27;m u`. This is a production routing defect in the terminal's advertised modern-keyboard subset, not merely missing headed evidence.

### P1 — Kitty set/clear does not create or update the required per-screen base stack entry

**Files:** `crates/seyal-terminal/src/terminal.rs:473-541,999-1022`, `crates/seyal-terminal/tests/m002_keyboard.rs:33-52`

Section 21.4 requires set/clear to modify the current stack value, creating a base value when the stack is empty. `set_keyboard_flags` changes only `modes.keyboard_flags` and the per-screen flags scalar; it never writes a stack element or increments `*_keyboard_stack_len`. `push_keyboard_flags` then stores only its newly supplied flags, and `pop_keyboard_flags` restores only stored elements.

For example, `CSI = 3;1 u`, then `CSI > 1 u`, then `CSI < 1 u` yields flags `0` in this implementation, because the set value `3` was never made the base stack entry. The required restored state is `3`. The three-case terminal test covers masking, a push/pop sequence without a preceding set, and screen isolation; it does not exercise this required sequence.

### P2 — V2 action-ID exhaustion leaves native semantic input permanently silent instead of reconnecting

**Files:** `macos/Seyal/Sources/TerminalInputSurface.swift:367,518-524,563-569`

`nextKeyboardActionID` advances with wrapping addition. After action ID `u32::MAX`, it becomes zero. Both V2 press and release paths return immediately when the next ID is zero, without a visible failure, recovery request, or fresh connection/attachment. The wrap does not replay an ID, but §21.5 requires admission to stop *before* exhaustion and establish a fresh connection/attachment under the recovery contract. This is a rare but permanent availability failure for the modern semantic-key path.

### P3 — the FFI accepts malformed V2 field combinations and delegates their rejection to a connection-fatal Runtime path

**Files:** `crates/seyal-client/src/ffi/input.rs:87-110`, `crates/seyal-client/src/local/input_resize.rs:302-338`, `crates/seyal-protocol/src/pass7.rs:1131-1213`

`seyal_bridge_submit_key_v2` validates only kind, event, and modifier bits. It does not validate value ranges, ASCII/shifted-ASCII relationships, or the full V2 shape before calling `TerminalKeyV2::encode`; the latter has only a debug assertion. A release build can therefore send malformed semantic data through the bridge, and the Runtime correctly treats that frame as fatal. The shipped Swift classifier produces valid inputs, so this is an availability/defense-in-depth defect at the C ABI rather than a terminal-input injection path.

### P3 — required matrix coverage is absent

**Files:** `crates/seyal-terminal/tests/m002_keyboard.rs`, `crates/seyal-protocol/tests/pass7_input_resize.rs`, `macos/Seyal/Sources/TerminalInputSurface.swift:966-1227`

Section 21.6 requires fixtures for every V2 kind × flags 0/1/2/3 × press/repeat/release × accepted modifier combination, plus printable ASCII shifted-field cases. The exact-candidate terminal test has three cases and the protocol integration test has eight; the macOS-gated self-test samples selected classifier properties. No exhaustive encoder/admission matrix is present.

## Audit of predecessor re-review findings

| Predecessor finding | Status at `2694b99` | Source proof |
| --- | --- | --- |
| Every structurally readable type-29 Error must use sent/highest-error validation | **Fixed in source** | `LocalDisplayClient::classify_incoming_error` routes every type-29 Error to `classify_v2_error` (`input_resize.rs:341-371`); `v2_error.rs:26-50` checks a nonzero detail against sent/highest bounds before categorizing all known Error codes. |
| Zero-detail Backpressure was accepted as ordinary server error | **Fixed in source** | `v2_error.rs:26-35` returns `Protocol` for zero-detail Backpressure, PermissionDenied, StaleIdentity, and InvalidExecution; zero-detail MalformedPayload remains fatal, which is the permitted bounded generic malformed-frame result. |
| Sent bound advanced on enqueue, not a wire-complete write | **Fixed in source** | `submit_terminal_key_v2` records `last_admitted_v2_action_id` after FIFO admission (`input_resize.rs:318-338`); `flush_control_write` updates `last_sent_v2_action_id` only after the front V2 frame is fully written and popped (`:497-514`). The direct classifier test covers unsent IDs; the socket WouldBlock test exists under macOS-only `local.rs` and did not run on this Linux host. |
| Held-key overflow silently dropped a new press and also dropped an existing-key repeat | **Fixed in source** | `TerminalInputSurface.swift:510-517` tests whether the key is new before enforcing the 256-key limit, sets `nativeFailure = .clientBackpressure`, refreshes the visible failure layer, and allows already tracked repeats to proceed. This source path was not executable on this Linux host. |

The predecessor P1s are therefore fixed in this tree, but the three P1 findings above independently block a closing merge.

## SPEC-006 §21.7 evidence-gate audit

| Gate | Exact-candidate disposition |
| --- | --- |
| Modes / key bytes / negotiation | **Incomplete.** Exact Linux terminal tests passed only three focused cases. The required full key/flags/events/modifiers matrix is absent, and the set/push/pop P1 invalidates the implemented negotiation contract. |
| Modern events | **Missing.** No real Neovim flags-3 handshake, Control-I versus Tab, Escape, repeat/release, and legacy-exception result exists for this SHA. The native routing P1 also blocks correct execution of several required cases. |
| Wire / security / admission / recovery | **Incomplete.** Exact candidate client/protocol unit coverage passed, and type-29 correlation is improved, but pre-attachment unnegotiated V2 is nonfatal and the macOS-gated real Runtime/PTY test executed zero tests here. |
| Native | **Missing.** Linux has no Debug `Seyal.app`, Metal display, HID keyboard, keypad, or IME. The retained headed record is for predecessor commits and records `ENVIRONMENT_UNSUPPORTED`; it is neither promoted to this SHA nor a pass. |
| Fuzz / property | **Missing for this SHA.** The retained bounded fuzz result is for `9848add`, not this candidate, and is smoke-grade rather than the required exact-head property/campaign evidence. |
| Performance | **Missing.** No exact baseline/candidate Release ARM64 three-run measurement of native-key→Runtime and key→PTY percentiles, CPU/RSS/queue high water, or rejection/defer counts. Linux platform-limited records are not a pass. |
| Exact-head build / test / check | **Partial only.** The focused Linux commands below ran at `2694b99`; no exact-head `make build`, `make test`, `make check`, macOS client/FFI/native, or headed result was established. |
| Independent code/security/native review | **This source review is independent, but returns NO-GO.** Native review evidence remains unavailable on this host. |

The headed six-step session, physical layout/keypad and IME work, target-TUI validation, and native latency evidence remain mandatory completion gates.

## Verification performed

```text
git -C /tmp/seyal-oss-work/issue-823-keyboard-architecture rev-parse HEAD
PASS — 2694b99d9ff28615b484f6d61d44b226ec7486b2

git -C /tmp/seyal-oss-work/issue-823-keyboard-architecture log -1 --oneline
PASS — 2694b99 fix(823): correlate every type-29 V2 error to wire-complete IDs

git -C /tmp/seyal-oss-work/issue-823-keyboard-architecture diff --check origin/master...HEAD
PASS — exit 0

cargo test -p seyal-client --locked --lib --manifest-path /tmp/seyal-oss-work/issue-823-keyboard-architecture/Cargo.toml
PASS — 10 passed, including all 3 portable v2_error tests.
NOTE — local client/FFI code is cfg(target_os = "macos") and did not compile or execute on Linux.

cargo test -p seyal-protocol --locked --manifest-path /tmp/seyal-oss-work/issue-823-keyboard-architecture/Cargo.toml
PASS — 26 unit tests and 8 pass7_input_resize integration tests passed; 2 fuzz-smoke test shims ignored.

cargo test -p seyal-terminal --test m002_keyboard --locked --manifest-path /tmp/seyal-oss-work/issue-823-keyboard-architecture/Cargo.toml
PASS — 3 passed.

cargo test -p seyal-runtime --test pass7_local_ipc --locked --manifest-path /tmp/seyal-oss-work/issue-823-keyboard-architecture/Cargo.toml
CFG SKIP — command passed with 0 tests; the test target has #![cfg(target_os = "macos")].
```

No headed XCUI, native macOS, Metal, HID keyboard, IME, or performance test was run or claimed.

## Closing decision

There are remaining source defects: three P1 violations of V2 framing/routing/negotiation, the P2 action-ID exhaustion failure, and the P3 ABI validation/matrix-coverage gaps. Separately, the mandatory exact-head native, target-TUI, fuzz/property, and performance evidence gates are unmet. Either category is sufficient to reject a closing #823 merge; together they require further implementation and a later independent exact-head re-review.
