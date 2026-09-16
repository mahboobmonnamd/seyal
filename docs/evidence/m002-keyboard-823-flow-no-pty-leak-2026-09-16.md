# M002 #823 remaining headed evidence — 2026-09-16

| Field | Value |
| --- | --- |
| Branch | `issue/823` from `master` `1aa1a7c` |
| Relationship | `Refs #823` — not `Closes` |
| Host | Darwin arm64 / macOS 26.5 / Rust 1.98 |

## Why this is not a Flow key→PTY oracle

ADR-015 Flow eligibility does not admit direct terminal keys. Native
`InteractiveMetalSurfaceView.allowsDirectTerminalInput` is true only for Raw
and TUI. Headed XCUI must stay on Flow/Blocks, so it cannot be the V2 byte
oracle. Runtime IPC (`controller_terminal_key_is_encoded_by_runtime_and_reaches_pty`,
ArrowUp → `27 91 65`) remains the encoder→PTY proof.

The withdrawn alt-screen / raw `dd` XCUI path looked like a normal terminal
and is still withdrawn.

## Added headed case

`SeyalHostUITests.testFlowBlocksDoesNotForwardArrowUpIntoWaitingPty`:

- stay on Flow/Blocks after attach
- composer-submit `head -c 3` into a capture file
- send Cmd-C and ArrowUp
- capture file must stay empty
- composer + Blocks + transcript must remain

This is stronger than `testComposerSubmitAndHostShortcutsStayOnFlowBlocks`,
which only asserts the chrome stays on Flow.

Focused headed result on this host after rebase onto `1aa1a7c`:

```text
** TEST EXECUTE SUCCEEDED **
SeyalHostUITests.testFlowBlocksDoesNotForwardArrowUpIntoWaitingPty
Executed 1 test, with 0 failures in 34.571s
```

## Still open for `Closes #823`

1. Manual six-step: shell navigation, Neovim DECCKM, Kitty subset TUI,
   Option-as-Alt, dead-key/IME, repeat under load. These presentations are
   Raw/TUI by design and are not XCUI oracles.
2. Fresh independent headed + native GO of this head.
3. #673 physical latency rows remain deferred.

Documentation impact: N/A for User Guide (no new setting). Developer evidence
only.
