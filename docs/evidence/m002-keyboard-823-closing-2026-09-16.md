# M002 #823 current-head closing evidence — 2026-09-16

| Field | Value |
| --- | --- |
| Source head | `2444c78` (`origin/master`) |
| Host | Darwin arm64 / macOS 26.5.2 / Rust 1.98.0 |
| Runtime | exclusive (no leftover `seyal-runtime` / `Seyal.app` before the run) |

Production keyboard implementation is already on master (#941, #945). This record is the current-head evidence package for independent review.

## Portable encoder and Runtime→PTY

```text
m002_keyboard                          5/5
seyal-runtime --lib key_v2            13/13
seyal-client --lib input_policy        5/5
pass7_local_ipc controller_terminal_key 1/1
  controller_terminal_key_is_encoded_by_runtime_and_reaches_pty
  ArrowUp → 27 91 65
```

Runtime remains the sole mode-sensitive encoder. Flow does not admit direct terminal keys (ADR-015 / ADR-009).

## Headed Flow/Blocks (exact products from `scripts/test-macos-ui.sh`)

`SeyalHostComponentTests` 18/18, including `testNativeKeyClassifierAndActionIDs`.

`SeyalHostUITests` keyboard/Flow cases:

```text
testComposerSubmitAndHostShortcutsStayOnFlowBlocks     passed (30.442s)
testFlowBlocksDoesNotForwardArrowUpIntoWaitingPty      passed (33.822s)
testCopyPasteAndQuitMenusAreWired                      passed (16.040s)
```

Cmd-C / ArrowUp on Flow did not write PTY bytes into a waiting `head -c 3` capture. Composer + Blocks + transcript stayed visible.

The same full `test-macos-ui.sh` invocation also ran `SeyalHostHistoryUITests` 2/2. One unrelated hosted case, `testSelectingABlockRevealsRustBlockDetailsInInspector`, failed; it is Block-inspector chrome, not keyboard encoding.

## Reviewer verification (Raw/TUI six-step)

These presentations are mutually exclusive with Flow. They are the remaining headed checks on this PR, not a second implementation:

1. Shell: arrows, Home/End, PageUp/PageDown, Insert/Delete, function keys.
2. Neovim: normal vs application cursor mode, modifiers, repeat.
3. Kitty-subset TUI: only the negotiated subset is advertised/encoded.
4. Option-as-Alt; Cmd host actions must not leak bytes.
5. Dead-key / IME: committed text once, preedit does not leak.
6. Repeat under high output within the backpressure contract.

Release latency rows stay under the versioned performance contract.

Documentation impact: N/A for User Guide (no new setting). This file is developer evidence only.
