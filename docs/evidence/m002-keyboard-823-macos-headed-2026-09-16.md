# M002 #823 macOS exclusive-Runtime headed attempt — 2026-09-16

| Field | Value |
| --- | --- |
| Branch | `issue/823` |
| Source head | `379f8c655452acc4313f4987d90b875e11337d7d` (SOURCE_GO) |
| Docs ancestor | `f6e2dee3d1a4f22dab172ea2ae8aa6d86187505c` |
| Host | Darwin 25.5.0 arm64 / macOS 26.5.2 / Rust 1.98.0 / Xcode 26 |
| Relationship | `Refs #823` — not `Closes` |

## Unblock

The previous headed blocker (PID 26466 owning the canonical `control.sock`) is gone. Before this run:

- no `seyal-runtime` / `Seyal.app` process
- Darwin per-user socket
  `/var/folders/jg/42_krzk50wx37y55whwswcpc0000gn/T/seyal-runtime/control.sock`
  did not exist

## Portable / component evidence (this host)

| Check | Result |
| --- | --- |
| `cargo test -p seyal-runtime --locked --test pass7_local_ipc` | 13/13, including `controller_terminal_key_is_encoded_by_runtime_and_reaches_pty` (ArrowUp → `27 91 65`) and pre-attach type-29 fatal |
| `cargo test -p seyal-runtime --locked --lib -- key_v2` | 13/13 |
| `cargo test -p seyal-terminal --locked --test m002_keyboard` | 5/5 |
| `cargo test -p seyal-client --locked --lib -- input_policy` | 4/4 |
| `SeyalHostComponentTests` exact-head ad-hoc-signed products | 8/8, including `testNativeKeyClassifierAndActionIDs` |

Host-component 8/8 still sends no terminal keys or commands. Chrome Auto Layout warnings remain; they are outside #823 / #824 scope.

IPC ArrowUp→PTY is Runtime-wire evidence, not headed AppKit→client→Runtime→PTY proof.

## Headed XCUI

Added `SeyalHostUITests.testKeyboardToPtyEncodesArrowUpShiftF3AndDoesNotLeakCommandShortcuts`.
It enters alternate-screen TUI so `terminal-input` is the direct route, then captures admitted bytes via `dd` files:

- ArrowUp → `CSI A` (`27 91 65`)
- Shift+F3 → `CSI 13;2~`
- Cmd-C must not leak; following ArrowUp must still be `CSI A`

`xcodebuild test-without-building` for that case failed before any assertion:

```text
Timed out while enabling automation mode.
The test runner failed to initialize for UI testing.
```

Classification: `ENVIRONMENT_UNSUPPORTED` for XCUIAutomation on this session (TCC / automation-mode enablement), not a product FAIL and not a headed PASS.

A subsequent `open` of the same signed Debug `Seyal.app` created the exclusive `control.sock` as a `0600` socket within 1s. That app was quit so the singleton was not left occupied.

## Six-step matrix

| # | Step | Result |
| --- | --- | --- |
| 1 | Shell navigation / function keys | ENVIRONMENT_UNSUPPORTED (XCUI automation mode) |
| 2 | Neovim DECCKM / modifiers / repeat | ENVIRONMENT_UNSUPPORTED |
| 3 | Modern-keyboard TUI subset | ENVIRONMENT_UNSUPPORTED |
| 4 | Option-as-Alt; Cmd non-leak | ENVIRONMENT_UNSUPPORTED |
| 5 | Dead-key / IME | ENVIRONMENT_UNSUPPORTED |
| 6 | Hold under high output | ENVIRONMENT_UNSUPPORTED |

## Remaining for `Closes #823`

1. Enable UI automation for the XCTest runner on this Mac, rerun the new key→PTY case and the six-step matrix on the exact headed head.
2. Fresh independent headed + native GO.
3. Latency / physical perf remains `#673` / `#824` authority.

Do not open a closing PR from this attempt.
