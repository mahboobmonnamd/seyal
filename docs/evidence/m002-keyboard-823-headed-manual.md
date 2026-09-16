# M002 #823 headed manual evidence

- **Date:** 2026-09-10
- **App:** exact #823 Debug `Seyal.app` from the issue/823 worktree
- **Production boundary:** AppKit shell, Metal terminal surface, separately owned Runtime; no preview fixture
- **Classification:** `ENVIRONMENT_UNSUPPORTED`

## 2026-09-10 cloud-agent headed attempt

- **Host:** Linux cloud agent (`hostname=cursor`, `uname=Linux 6.12.94+ x86_64`)
- **Exact production code head:** `9848add7b03c8d402b55164cd27f9008a3df052e`
- **Exact branch head at attempt:** `f24e2a4` (client V2 admission fix after `9848add`)

A self-hosted macOS worker (`Mahboob's MacBook Pro (2)`,
`workerId=b1146e6c-9dea-5584-a5ef-152e2391887e`) was connected and idle. This
cloud-agent process is not running on that worker. A computer-use subagent
(`bc-4f1dc836-34bf-5c39-9fb6-9c136c3d3366`) also landed on Linux
(`privateWorkerId=null`) and recorded the same six-step
`ENVIRONMENT_UNSUPPORTED` ledger. Physical keyboard, IME
source, keypad, and Metal pixels are not available here. No Debug
`Seyal.app` was launched from this host, and no checklist step is PASS.

| Manual step | Result |
| --- | --- |
| Shell: arrows, Home/End, PageUp/PageDown, Insert/Delete, function keys | ENVIRONMENT_UNSUPPORTED |
| Neovim: normal vs application-cursor, modifiers, held-key repeat | ENVIRONMENT_UNSUPPORTED |
| Target TUI negotiating kitty/modern keyboard; supported subset only | ENVIRONMENT_UNSUPPORTED |
| Option/Alt/Meta; Cmd host shortcuts must not leak bytes into the PTY | ENVIRONMENT_UNSUPPORTED |
| Dead-key / IME composition beside navigation; committed text once; preedit must not leak | ENVIRONMENT_UNSUPPORTED |
| Hold keys under high output; no drops outside the backpressure contract | ENVIRONMENT_UNSUPPORTED |

Missing hardware/layout: this agent has no macOS GUI, no HID keyboard, no
IME input source, and no keypad. Do not treat prior focused XCUI shortcut
coverage (`testNativeKeyboardShortcutsSwitchWorkspaceTabsAndSidebars`) as
these six steps.

#823 remains NO-GO for a closing PR until a headed session on the exact app
records pass/fail (or a more specific `ENVIRONMENT_UNSUPPORTED` with the exact
missing layout) for every row above.

## 2026-09-10 later computer-use abort (after `b681676`)

- **Date:** 2026-09-10
- **Host:** Linux `6.12.94+` x86_64 (`privateWorkerId=null`)
- **Exact production code head:** `b6816764467ba7e2e0490f78f7a2744e517ec768`
- **Exact branch head at attempt:** `a05b13c`

A further computer-use subagent was launched after the V2 error-ID
correlation commit and also ran on Linux. It stopped after `uname -a` and
did not launch `Seyal.app`. The six-step checklist remains
`ENVIRONMENT_UNSUPPORTED`. Raw note: `/tmp/m002-reviews/headed-macos-819-823.md`.

## 2026-09-15 Linux cloud-agent attempt (exact head `379f8c6`)

- **Host:** Linux cloud agent (`uname=Linux`, x86_64)
- **Exact branch head:** `379f8c655452acc4313f4987d90b875e11337d7d`
- **Classification:** `ENVIRONMENT_UNSUPPORTED`

No macOS GUI, HID keyboard, IME source, keypad, or Metal pixels on this host.
No Debug `Seyal.app` was launched. All six headed steps remain
`ENVIRONMENT_UNSUPPORTED`. See `m002-keyboard-823-macos-handoff.md` for the
macOS exclusive-Runtime procedure and `m002-keyboard-823-379f8c6-rereview.md`
for the Linux SOURCE_GO / CLOSE_NO_GO verdict.

## 2026-09-16 Apple Silicon attempt (exact source `379f8c6`)

- **Host:** Darwin 25.5.0 arm64 / macOS 26.5.2
- **Exact source head:** `379f8c655452acc4313f4987d90b875e11337d7d`
- **Classification:** exclusive Runtime **unblocked**; headed XCUI `ENVIRONMENT_UNSUPPORTED` (automation-mode timeout)

The prior shared-socket occupant is gone. Host-component 8/8 and focused
Runtime/key_v2 suites passed. The new key→PTY XCUI case did not start. See
`m002-keyboard-823-macos-headed-2026-09-16.md`.
