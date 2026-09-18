# M002 #824 active remediation

This note records the diagnose → fix → rerun loop required by the M002 closure audit. It does not close #824 or #672.

## Runtime IPC full-suite failure

Observed symptom: `make check` failed repeatedly in `pass7_local_ipc` while creating test executions with `Exec(Io(code: -6))`, while the isolated suite and hosted CI could pass.

Root-cause candidates found in the test harness:

1. `pass7_local_ipc::config()` used only a process-local counter for singleton and local-IPC paths (`s7-0.lock`, `s7d-0`, ...). Separate Cargo test processes each start the counter at zero, so concurrent test binaries can collide in the shared temporary directory.
2. The harness owns disposable Runtime/PTy children but previously relied on ordinary `Drop`. Seyal production semantics deliberately do not terminate a `TerminalExecution` on ordinary Drop; a test-owned Runtime must explicitly perform controlled shutdown.

Remediation on `issue/824`:

- include the process id plus the process-local counter in test singleton/runtime-dir names;
- perform controlled Runtime shutdown from the test Harness destructor;
- leave production Runtime/TerminalExecution lifecycle semantics unchanged;
- rustfmt the `runtime_dir_override` assignment so Foundation Quality does not fail `cargo fmt --check`.

**Acceptance status (exact production head `2ef8332`):** hosted Foundation
Quality `make check` PASS
([run 35305544844](https://github.com/seyal-org/seyal/actions/runs/35305544844)),
including `pass7_local_ipc`. Docs-only tip re-check also PASS
([run 35319442002](https://github.com/seyal-org/seyal/actions/runs/35319442002)).
Earlier hosted PASS on `ad50bd1`
([run 35242876394](https://github.com/seyal-org/seyal/actions/runs/35242876394))
is retained as the first isolation-fix record. The historical local double-fail
and 44/45 Block-selection FAIL are **superseded history** only.

## Block selection full-suite failure

Observed symptom: the final full native run passed 44/45; `testSelectingABlockRevealsRustBlockDetailsInInspector` failed although the same Block-selection case had passed in isolation.

The Rust selection action already validates the selected Block and a successful action bumps application generation. The remaining failure is therefore being treated as a native transcript/hit-testing/reconciliation defect until disproved, not as missing Rust product state.

The transcript currently scrolls to live end when a new Block is first projected. History-range replies arrive asynchronously and can later grow existing Block bodies. In a persistent Runtime/full-suite run this can move the newly submitted Block out of the visible clip after the initial live-end scroll. The production fix must preserve live following only when the user was already at the live end; it must not yank a user who intentionally scrolled back through history.

Remediation on `issue/824`:

- track `followingLiveEnd` from user transcript scroll proximity;
- on new Block projection and on history-range body growth, re-pin to live end only while following;
- wait for the submitted card to become hittable before the XCUI click so an off-clip ghost cannot pass existence and fail selection.

CI at `d44662c`: Block-selection XCUI and all four workload XCUI cases PASS. Remaining CI FAIL was
`testNativeIMELiveCallbacksDeliverOnlyCommittedUTF8AndTrackCursor` timing out on the
"production Runtime and projection connected" wait; harden that wait with a connection poll.

## Block selection click swallowed by Flow pane (2026-09-18)

Local full `scripts/test-macos-ui.sh` on `accd4bb` reproduced
`testSelectingABlockRevealsRustBlockDetailsInInspector` FAIL: XCUI clicked
`seyal-block-8`, then `seyal-inspector` never appeared.

Cause: `centerColumn` stacks the transcript under `ThinPaneHostView`. Flow
Metal already returns `nil` from `hitTest`, but the pane's default NSView
hit-test then returned `self` and swallowed the click, so `CommandBlockView`
never saw `mouseDown`. Isolated inspector PASS was not enough; the card
center is the body once output exists.

Fix: `ThinPaneHostView.hitTest` returns `nil` in Flow (`drawsLiveGrid == false`)
when it would otherwise claim the click. Raw/TUI keep the live grid.

Local evidence after the fix (exclusive Runtime free):

- isolated inspector XCUI **PASS** (26.2s)
- inspector in the full 19-test suite **PASS** (26.7s / 26.9s)
- 7-test cluster (inspector + both alt-screen cases + four workload cases) **7/7 PASS**
- full 19-test suite still **FAIL**s `testAlternateScreenReturnRestoresFlowNotRawTerminal`
  waiting for composer `isHittable == false` after the ABC dead-key case.
  That case **PASS**es isolated and in the 7-test cluster. Not treated as
  inspector-click FAIL. Do not weaken the alt-screen assertion.

## Alternate-screen XCUI after History+UITests prefix (2026-09-18)

Observed: `testAlternateScreenReturnRestoresFlowNotRawTerminal` **PASS** isolated,
ABC-then-alt-screen pair **PASS**, 7-test cluster **PASS**. The same assertion
**FAIL**ed after History + all `SeyalHostUITests`: composer stayed `available`
and hittable for 12s after the bash `1049h` submit (editor had cleared).
Leftover exclusive Runtime was idle `/bin/zsh` with no bash grandchild.

Two cooperating defects:

1. XCUI `typeText` of a long `FileManager.temporaryDirectory` path did not
   start the bounded child under that prefix load, so `1049h` never ran.
   The fixture now uses `/tmp/s1049-<token>.sh` and records a start marker.
2. `reconcileChrome` returned immediately while `isReconcilingChrome` was
   already true. A one-shot `1049h` during Flow `rebuildBlocks()` could be
   dropped with no further frames while bash blocks in `read`. Nested pulses
   now set `chromeNeedsReconcile` and the outer pass loops on a fresh snapshot
   (bounded). Composer hide is still Rust eligibility.

The XCUI wait still requires `isHittable == false`.

Local evidence after the fix (exclusive Runtime free):

- nested TUI chrome component case **PASS**
- isolated alt-screen XCUI **PASS** (35.3s)
- History + all `SeyalHostUITests` + workload alt-screen prefix **16/16 PASS**
  (398.9s; the previously failing case 35.8s)
- full native + XCUI `test-without-building` **27/27** component and **19/19**
  XCUI **PASS** (alt-screen in-suite 35.6s)

**Acceptance status (exact production head `2ef8332`):** hosted
`native-macos-smoke` SUCCESS
([run 35305544844](https://github.com/seyal-org/seyal/actions/runs/35305544844)).
Docs-only tip re-check SUCCESS
([run 35319442002](https://github.com/seyal-org/seyal/actions/runs/35319442002)).
Local exclusive-Runtime on `7f77a6f`: **27/27** component + **19/19** XCUI
**PASS** (inspector in-suite; alt-screen in-suite 35.6s). Earlier hosted PASS
on `ad50bd1` (run 35242876394: Block selection 28.53s, live IME 1.15s, XCUI
19 executed / 0 failures / 1 ABC skip) is superseded as the current-head
native record. The historical local 44/45 FAIL remains in the headed ledger
as **superseded history**.
