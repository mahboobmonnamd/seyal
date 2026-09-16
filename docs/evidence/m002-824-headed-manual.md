# M002 #824 headed / manual ledger

- **Issue:** #824
- **Date:** 2026-09-16
- **Production path:** ADR-015 thin AppKit host over Rust snapshots; headed oracle is Flow/Blocks, not a raw terminal.
- **Exclusive Runtime rule:** if another process owns `control.sock`, the headed run is INCONCLUSIVE.

## This session

| Check | Result |
| --- | --- |
| Exclusive `control.sock` | **INCONCLUSIVE** — pid 45106 `seyal-runtime` from `/Users/mahboob/Developer/seyal/target/macos-ui-tests/.../Seyal.app` held `${TMPDIR}/seyal-runtime/control.sock`. This run did not steal or unlink that socket. |
| XCUI `SeyalHostWorkloadUITests` | Not executed this session (exclusive Runtime occupied). Tests are retained in `macos/Seyal/Tests/SeyalUITests/SeyalHostWorkloadUITests.swift`. |
| Unicode/IME/hardware pixels | Not claimed. Metal does not expose PTY bytes as AX text. |
| Performance / PHYSICAL_ARM64 | Not run. See `m002-824-performance.md`. |

Do not promote this ledger to headed PASS.

## Retained XCUI (honest Flow/Blocks oracles)

These cases must stay on composer + Blocks. A leftover alternate-screen or Raw-only launch that looks like a normal terminal is a fail.

1. `testHighVolumeComposerOutputStaysOnFlowBlocks`
2. `testUnicodeComposerSubmitAndResizeStayOnFlowBlocks`
3. `testAlternateScreenReturnRestoresFlowNotRawTerminal`
4. `testGuiRelaunchReconnectsWithoutKillingExecution` — M001 detach/reconnect after GUI terminate; requires exclusive Runtime.

Run on an exclusive-Runtime macOS host:

```sh
# Confirm no other seyal-runtime owns control.sock, then:
scripts/test-macos-ui.sh
```

Record PASS / FAIL / ENVIRONMENT_UNSUPPORTED / PLATFORM_LIMITED per case. Never backfill Unicode glyph pixels from AX.

## Manual verification (#824 body)

| Step | Result this session |
| --- | --- |
| 1. zsh/bash/fish interactive | ENVIRONMENT_UNSUPPORTED (no exclusive headed Runtime) |
| 2. SSH then nested SSH | ENVIRONMENT_UNSUPPORTED (no remote host exercise) |
| 3. Vim/Neovim interactive | ENVIRONMENT_UNSUPPORTED |
| 4. tmux child windows/panes/copy-mode | ENVIRONMENT_UNSUPPORTED for headed; automated one-PTY proof exists |
| 5. htop/watch/ncurses | `htop` PLATFORM_LIMITED; headed ENVIRONMENT_UNSUPPORTED |
| 6. git/docker/kubectl/terraform TTY | terraform PLATFORM_LIMITED; headed ENVIRONMENT_UNSUPPORTED |
| 7. CLI-agent TUI | ENVIRONMENT_UNSUPPORTED; VT equivalent retained |
| 8. high-volume while typing/scrolling | headed ENVIRONMENT_UNSUPPORTED; VT/PTY automated PASS |
| 9. search/copy Unicode after resize | headed ENVIRONMENT_UNSUPPORTED; VT coherent fixture PASS |
| 10. GUI close/reopen M001 reconnect | headed INCONCLUSIVE (socket occupied) |

A later exclusive-Runtime macOS pass should fill the XCUI four-case table and the interactive rows without inventing a raw-terminal headed oracle.
