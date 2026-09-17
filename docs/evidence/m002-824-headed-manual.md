# M002 #824 headed / manual ledger

- **Issue:** #824
- **Date:** 2026-09-17
- **Validation base:** `f0e8c01b78dd8c342dbe75edf71785163bae4e6c` plus the PTY/IME/XCUI patch; runs preceded commit and final native source fingerprints are retained below
- **Production path:** ADR-015 thin AppKit host over Rust snapshots; headed oracle is Flow/Blocks, not a raw terminal.
- **Exclusive Runtime rule:** if another process owns `control.sock`, the headed run is INCONCLUSIVE.

## Latest native verification (2026-09-17)

The previous Runtime owner shut down PID 99338 before this run. The native test
application then launched its own bundled Runtime, PID 23758; no foreign Runtime
was terminated. Results below were captured before committing the `issue/824`
patch on `f0e8c01`, not as a clean release freeze or final PR-head acceptance.

- `target/m002-ime-red.xcresult`: direct native callback tests reproduced
  cancellation retaining preedit and candidate coordinates returning the whole
  view without a projection (two failing cases, five failed assertions).
- `target/m002-ime-final-v2.xcresult`: **26/26 component tests PASS**, including
  eight native IME tests; **1/1 Block selection XCUI PASS**.
  [Retained result summary](m002-824-ime-native-results.json) has 27 passed,
  zero failed/skipped; device identifiers have been removed.
- `target/m002-ime-headed.xcresult`: **4/4 workload XCUI PASS**. The separate
  Block selection case failed in this earlier run; its AppKit hit-testing
  correction was verified in the final-v2 run above.
  [Earlier combined summary](m002-824-ime-headed-results.json) deliberately
  retains the failed overall result (30 passed, one failed); it is not a
  green combined-run claim.
- Independent source review found the initial deferred-discard and failure-path
  cleanup concerns resolved after follow-up. This is not an independent closing
  review of every #824 acceptance criterion.
- **Final full native run:** `target/m002-ime-final.xcresult` executed **45 tests:
  44 passed, one failed, zero skipped**. All eight native IME callback cases and
  the real ABC system-input-source case passed. The remaining failure is
  `testSelectingABlockRevealsRustBlockDetailsInInspector`: after clicking the
  newly submitted card, the inspector did not appear. The
  [full result summary](m002-824-ime-full-results.json) retains that failure.
  Earlier isolated Block-selection passes do not make this final gate green.

### Real macOS input-source verification

`target/m002-ime-isolation.xcresult`: **2/2 XCUI cases PASS**, zero failures or
skips, including `testSystemABCDeadKeyCommitAndCancelReachRealPty`.
The [sanitized result summary](m002-824-ime-system-results.json) is retained.
The test requires the selected macOS ABC layout and uses synthesized XCUI key
events through the actual system input context, native terminal view, Runtime,
and a real PTY child. It does not call `setMarkedText` or `insertText` directly.

- Option-E then E commits `é`.
- Option-E then Escape cancels the pending dead key; subsequent X commits `x`.
- A following ordinary Escape still reaches the terminal.
- The exact captured bytes are `c3 a9 78 1b`; cancelled preedit and its cancelling
  Escape are absent. The receiver restores termios and the primary screen.

The RED run (`m002-ime-system-v3.xcresult`) captured `c3 a9 1b` instead of `éx`:
terminal key classification intercepted cancellation before AppKit could clear
its pending dead key. Dead-key state can exist without the client's marked
document. The fix offers unmodified Escape to the input context first and
preserves terminal encoding when AppKit reports an unhandled cancel command.
Marked-document editing keys likewise go through AppKit before terminal encoding.
Modified Escape retains its existing routing.

A combined-suite attempt (`m002-ime-complete.xcresult`) passed 44 cases but failed
the system test's receiver setup. The earlier Flow test left `head -c 3` alive,
which could consume a subsequent shell command. Its replacement is a bounded
raw-mode receiver with explicit readiness and shutdown acknowledgments, without
entering alternate screen. A missing capture now fails rather than passing.
The paired Flow-then-system run above verifies the cleanup and the subsequent
IME byte oracle together. Only test-owned Runtime processes were reset.

The same shell-only two-`printf` alternate-screen fixture existed in three test
classes. Its standalone-then-Flow sequence also reproduced receiver-startup
failure. All three now share a bounded child-shell fixture with an EXIT trap,
explicit takeover/return visibility assertions, and failure-path Return cleanup.
`target/m002-ime-shared.xcresult` passed all six selected cases across the native,
history, host and workload targets. This validates child-owned alternate-screen
lifecycle; arbitrary manual shell-only `1049h`/`1049l` sequencing is not claimed.
The legacy sequence failure remains recorded here under #824 for classification
before closure, rather than being silently relabeled a product PASS.

This establishes real ABC input-source behavior using automated key events,
not physical keyboard hardware or vendor-specific multilingual candidate UI.

### Native IME result boundaries

| SPEC-011 fixture | Native result and observation | Not established |
| --- | --- | --- |
| 37 marked text to commit | PASS: real callbacks and Runtime/PTY multi-scalar UTF-8 capture; the separate system test verifies ABC dead-key conversion through AppKit | Physical keyboard hardware; multilingual candidate UI |
| 38 cancel/abandon | PASS: real `doCommand(cancelOperation:)` clears preedit and synchronously discards AppKit composition; cancelled text was absent from the PTY capture | Vendor-specific candidate-popup cancellation |
| 39 replacement commit | PASS: native marked-document replacement and committed replacement bytes; subsequent `unmarkText` commits once, not twice | Language-specific conversion/reconversion |
| 40 candidate coordinates | PASS at this host's backing scale: real callback uses the current projected cursor cell and tracks actual window movement; absent projection/invalid range returns unavailable | Moving between displays with different scales; physical candidate-popup placement |
| 41 stale preedit on detach/reconnect | PASS for native lifecycle state: removing/reinstalling the view exercises AppKit window callbacks; disconnected bridge callback discards preedit | Full physical composition session across live Runtime reconnect |

These are native-callback and real-PTY evidence, not just `CompositionDocument`
checks. The live test uses the application's existing `ProductChromeHostView`,
not a second client or mocked terminal. Its child has a 15-second deadline,
restores termios/primary screen in `finally`, and has early-return EOT cleanup.
The captured payload is test-generated; no user terminal content is captured.

The production fixes are cancellation handling, input-context priority for
composition keys, and cursor-cell candidate geometry. The inherited Block hit-test fix delegates coordinate/visibility
validation to AppKit's superclass rather than comparing a superview point with
local bounds. Swift still owns only native adaptation.

Apple API evidence: installed macOS SDK `NSTextInputClient.h`
(`setMarkedText`, `insertText`, `unmarkText`, `firstRectForCharacterRange`) and
`NSTextInputContext.h` (`discardMarkedText`: client clears its marked range).
Host: Darwin arm64, macOS 26.5.2, Xcode 26.6 (17F113).

Native source fingerprints for the final-v2 run:

| Source | SHA-256 |
| --- | --- |
| `InteractiveMetalSurfaceView.swift` | `d03cd4ad6c640b709c7a97bf4e08b3ede0f75de9d148f8e773653a4963222c32` |
| `ProductChromeHostView.swift` | `a0743a81faf5e81b65e19bd254a997ab4df3990f2517d47b09267a5e4d705de2` |
| `SeyalHostComponentTests.swift` | `c8484732ead7191da6f97c2953cdce27cded167fbf2620b1e985686a776b6052` |

Changed-source fingerprints for the final 45-test run (the component and
ProductChrome hashes above are unchanged):

| Source | SHA-256 |
| --- | --- |
| `InteractiveMetalSurfaceView.swift` | `9f4b8191677d33c20a99b915d638eaee7dddfccd4dc6fd2c3d44687a67259e16` |
| `SeyalHostUITests.swift` | `142473b4f8b997c966fec984676127116b75ee755660bd91fda9e60934a94ef3` |
| `SeyalHostHistoryUITests.swift` | `e14a154bce535a317edb91dde28319318fee07309a338d35dd9d8bfd7ac93a9c` |
| `SeyalHostWorkloadUITests.swift` | `d4e011fe197fdb1a043cff6cb3403916af7a06b456bf5e53631c051dbc7690e0` |

The physical-hardware and multilingual-candidate limitations remain unclaimed; #836 is not
pulled into M002. This verification does not automatically waive #824's other
manual, benchmark, or independent closing-review requirements.

The complete `make check` gate failed twice in `pass7_local_ipc` while spawning
test executions (`Exec(Io(code: -6))`); the isolated suite passed on rerun.
The full gate is not PASS, and its cause must be resolved rather than hidden
by the passing native results. A read-only diagnosis suggested resource or
test-isolation interference, but did not establish a reproducible root cause.
No speculative test serialization or production workaround was applied.

## Earlier session (2026-09-17, before the exclusive window)

Exclusive Runtime pid **99338** (`…/oss/seyal/target/macos-derived-data/Build/Products/Debug/Seyal.app/Contents/Helpers/seyal-runtime`) still owns the singleton. This session did **not** steal `control.sock` and did **not** re-run `scripts/test-macos-ui.sh`.

Live PTY rows that previously required missing binaries were executed instead (see matrix).

| Check | Result |
| --- | --- |
| Exclusive `control.sock` | **Occupied** — foreign pid 99338. Headed XCUI not re-run. Classification: INCONCLUSIVE for a new headed pass, not a product FAIL. |
| Prior XCUI `SeyalHostWorkloadUITests` | **4/4 PASS** retained from 2026-09-16 exclusive-Runtime session. |
| Prior full `scripts/test-macos-ui.sh` | **18/18 PASS** retained (Block-card click ownership fix). |
| Unicode/IME/hardware pixels | Not claimed. Metal does not expose PTY bytes as AX text. |
| Performance / PHYSICAL_ARM64 | Not run. See `m002-824-performance.md`. |

## Retained XCUI (honest Flow/Blocks oracles)

These cases must stay on composer + Blocks. A leftover alternate-screen or Raw-only launch that looks like a normal terminal is a fail.

| Case | Result this session |
| --- | --- |
| `testHighVolumeComposerOutputStaysOnFlowBlocks` | PASS retained (20.97s, 2026-09-16) |
| `testUnicodeComposerSubmitAndResizeStayOnFlowBlocks` | PASS retained (29.08s, 2026-09-16) |
| `testAlternateScreenReturnRestoresFlowNotRawTerminal` | PASS retained (34.96s, 2026-09-16) |
| `testGuiRelaunchReconnectsWithoutKillingExecution` | PASS retained (24.93s, 2026-09-16) |

Run on an exclusive-Runtime macOS host:

```sh
# Confirm no other seyal-runtime owns control.sock, then:
scripts/test-macos-ui.sh
```

Record PASS / FAIL / ENVIRONMENT_UNSUPPORTED / PLATFORM_LIMITED per case. Never backfill Unicode glyph pixels from AX.

## Manual verification (#824 body)

| Step | Result this session |
| --- | --- |
| 1. zsh/bash/fish interactive | Live PTY `zsh_bash_and_optional_fish_print_through_one_pty_vt` PASS. Headed interactive GUI ENVIRONMENT_UNSUPPORTED (exclusive Runtime occupied) |
| 2. SSH then nested SSH | PTY `live_ssh_and_nested_ssh_use_one_pty_vt_when_orbstack_present` PASS (`nt-ssh@orb`, remote `vim -c qa`, Docker sshd nested hop, one `TerminalExecution`). Headed GUI interactive SSH ENVIRONMENT_UNSUPPORTED (exclusive Runtime occupied) |
| 3. Vim/Neovim interactive | Live PTY `live_vim_and_neovim_restore_primary_after_alternate_screen` PASS (`:qa!` restores primary). Headed GUI ENVIRONMENT_UNSUPPORTED |
| 4. tmux child windows/panes/copy-mode | Live PTY `tmux_as_child_owns_one_pty_when_present` + `live_tmux_split_stays_one_seyal_pty` PASS (child markers on one Seyal PTY). Headed GUI ENVIRONMENT_UNSUPPORTED |
| 5. htop/watch/ncurses | Live PTY `live_htop_and_watch_restore_primary_after_ncurses` PASS (`htop` 3.5.3 installed this session). Headed GUI ENVIRONMENT_UNSUPPORTED |
| 6. git/docker/kubectl/terraform TTY | Live PTY git color log, `docker ps`, `kubectl version --client`, `terraform version` PASS (`terraform` v1.9.8 installed this session). Headed GUI ENVIRONMENT_UNSUPPORTED |
| 7. CLI-agent TUI | VT equivalent retained PASS. Live agent TUI headed ENVIRONMENT_UNSUPPORTED |
| 8. high-volume while typing/scrolling | headed Flow/Blocks XCUI PASS retained (`testHighVolumeComposerOutputStaysOnFlowBlocks`); not a type-while-flood interactive session. VT/PTY automated PASS |
| 9. search/copy Unicode after resize | headed Flow/Blocks XCUI PASS retained for composer Unicode submit + resize; not search/copy from retained history. VT coherent fixture PASS |
| 10. GUI close/reopen M001 reconnect | headed XCUI PASS retained (`testGuiRelaunchReconnectsWithoutKillingExecution`); Runtime unit `pass8_resync_reattach` + `macos_runtime` 12 PASS this session |

Do not treat Flow/Blocks XCUI as a raw-terminal Vim/htop/tmux/ssh oracle.

## Earlier document-only checks (superseded by native results above)

| Fixture | Coverage limit in this Refs PR |
| --- | --- |
| 37 IME marked text → commit | INCONCLUSIVE. `compositionMarkedCommitSelfTest` mutates and clears the document model; it does not invoke `insertText` or `unmarkText`, or prove committed input delivery. |
| 38 IME cancel/abandon | INCONCLUSIVE. `compositionCancelAbandonSelfTest` clears the document model; it does not exercise native cancellation. |
| 39 IME replacement commit | INCONCLUSIVE. `compositionReplacementCommitSelfTest` checks document replacement and clear, not the native replacement/commit path. |
| 40 IME candidate-coordinate validity | INCONCLUSIVE. `compositionCandidateCoordinateSelfTest` validates text ranges, not actual candidate-window coordinates. |
| 41 detach/reconnect discards stale preedit | INCONCLUSIVE. `compositionDetachDiscardsPreeditSelfTest` calls document `clear()` directly, not `viewWillMove(toWindow:)` or reconnect. |

These are `CompositionDocument` invariants wired into `pass7InputSelfTest`, not proofs of the native IME fixture outcomes. Headed physical IME remains unverified. Language-specific input-source breadth remains post-M004 #836; this Issue does not pull #836. Metal/AX is not an IME oracle.
