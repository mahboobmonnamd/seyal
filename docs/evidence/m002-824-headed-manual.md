# M002 #824 headed / manual ledger

- **Issue:** #824
- **Date:** 2026-09-18
- **Exact production head (fingerprints / hosted FQ):** `2ef83322a382a142d17ae1e4cc6ec0600116af41` (merge `20418fa` / PR #964). Current master at close-out measurement: `eecaf888bd8aecb3d4afcff371ad91ddce76c55f` (docs/chore only after that merge).
- **Docs-only tip after that head:** later ledger-honesty commits on `issue/824` (no further Swift/Rust delta).
- **Production path:** ADR-015 thin AppKit host over Rust snapshots; headed oracle is Flow/Blocks, not a raw terminal.
- **Exclusive Runtime rule:** if another process owns `control.sock`, the headed run is INCONCLUSIVE.
- **Relationship:** this follow-on evidence PR is the close package (`Closes #824` / `Closes #672`). PR #964 remains the merged `Refs` production delta.
- **IME 37–41 close classification:** **covered** by existing native `NSTextInputClient` + local ABC XCUI. Not a new #824 row. Do not pull #836.

## Close-out headed session (2026-09-18, exclusive Runtime)

Exclusive `control.sock` was free. Drove already-built Debug
`Seyal.app` (`dev.seyal.Seyal` adhoc, UI-test products dated 2026-09-18 07:28)
because local `xcodebuild` / `scripts/test-macos-ui.sh` rebuild is
**ENVIRONMENT_UNSUPPORTED**: Xcode 27.0 (`27A266a`) requires
`sudo xcodebuild -license`. Hosted `native-macos-smoke` on `2ef8332`
(run 35305544844) remains the XCUI record.

AX oracle: `terminal-input` `connection=` / `runtime=` / `execution=` /
`attachment=` / `alternate-screen=`. Metal does not expose PTY bytes as AX
text.

Initial attach: Runtime helper pid **96808**, GUI pid 96796,
`runtime=856ec7b7bfa631bf0000000000000001`,
`execution=c201c137e6fd067e0000000000000002`, `attachment=…0004`,
`connection=usable`.

Local exact-head `CARGO_TEST_THREADS=1 make check` on master `eecaf88` with
`SDKROOT`/`DEVELOPER_DIR` = Command Line Tools (bypass Xcode 27 license for
`cc`): **PASS** (exit 0, 71.9s). `python3 scripts/fuzz-smoke.py` is included
in that `make check`.

Owner-accepted classified gaps for M002 technical preview (not product FAIL):

- live Claude/Codex TUI scroll/prompt/resize: **ENVIRONMENT_UNSUPPORTED**
  (`claude` stayed Flow / `alternate-screen=false`; VT agent-TUI equivalent
  retained)
- headed retained-grid search/copy after resize: **ENVIRONMENT_UNSUPPORTED**
  (⌃R opened composer command history, not grid search; VT combined fixture
  retained)
- hosted ABC dead-key XCUI: `XCTSkip` without that layout; local ABC bytes
  `c3 a9 78 1b` retained

## Current-head exact evidence (`2ef8332`)

Executable Swift moved after `ad50bd1` / `204d14c`: Flow Block hit-test
fallthrough (`7cb7853`, `ThinPaneHostView`) and TUI chrome coalesce
(`7f77a6f`, `ProductChromeHostView` + nested-reconcile component test +
`/tmp` alt-screen XCUI fixture). `2ef8332` records headed-GUI steps 1–7 on
top of that production head. Later docs-only tip commits only sync ledger
honesty. Do not claim production sources are unchanged from `ad50bd1`.

Hosted Foundation Quality + production fuzz on exact production head
`2ef8332`
([run 35305544844](https://github.com/seyal-org/seyal/actions/runs/35305544844)).
Docs-only tip re-check (same fingerprints; no executable delta): Foundation
Quality
([run 35319442002](https://github.com/seyal-org/seyal/actions/runs/35319442002))
and production fuzz
([run 35319411982](https://github.com/seyal-org/seyal/actions/runs/35319411982)).

| Gate | Result on `2ef8332` (run 35305544844) |
| --- | --- |
| `repository-policy` | **SUCCESS** |
| `rust-and-harness-quality` (`make check`, includes `pass7_local_ipc`) | **SUCCESS** |
| `native-macos-smoke` | **SUCCESS** |
| `production-libfuzzer` | **SUCCESS** |
| `production-macos-state-fuzz` | **SUCCESS** |

Local exclusive-Runtime native + XCUI on the production commits under this
head (`7f77a6f`, recorded at `2ef8332`): **27/27** component + **19/19** XCUI
**PASS**, including inspector in the full suite and
`testAlternateScreenReturnRestoresFlowNotRawTerminal` (35.6s). Hosted ABC
dead-key remains `XCTSkip` without that layout. Retained local ABC bytes
`c3 a9 78 1b` under [system results](m002-824-ime-system-results.json).

Remediation that cleared earlier blockers is recorded in
[m002-824-remediation.md](m002-824-remediation.md): IPC harness PID isolation +
Harness `Drop` shutdown (`0c49cef` / `d44662c`), Flow live-end follow after
history growth (`d44662c`), IME connection poll (`ad50bd1`), Flow pane
hit-test fallthrough (`7cb7853`), TUI reconcile coalesce (`7f77a6f`).

Source fingerprints for the production/test sources at `2ef8332`:

| Source | SHA-256 |
| --- | --- |
| `InteractiveMetalSurfaceView.swift` | `9f4b8191677d33c20a99b915d638eaee7dddfccd4dc6fd2c3d44687a67259e16` |
| `ProductChromeHostView.swift` | `97df225ab9ea7b60c45f36ec58d486e3881d61cd716b2f64e8adea3cc66bea7f` |
| `ThinPaneHostView.swift` | `4241803afda53c17497e9c9ce7edcaec4500ae1026af8db73135e9ad974a5b4a` |
| `SeyalHostComponentTests.swift` | `1593f93cdcbc1477fdbca1e78e2e142091b25ecb82641ecaa097bced077e9f6d` |
| `SeyalHostUITests.swift` | `a81f851e843b225a7cd249c1f05a0f1be2b8b173cf9a73b4b6792e1dbaa00c12` |
| `SeyalHostHistoryUITests.swift` | `e14a154bce535a317edb91dde28319318fee07309a338d35dd9d8bfd7ac93a9c` |
| `SeyalHostWorkloadUITests.swift` | `d4e011fe197fdb1a043cff6cb3403916af7a06b456bf5e53631c051dbc7690e0` |
| `pass7_local_ipc.rs` | `4da73af2632285064476d1cb88a5ea2bf402ea7b5c454b6af942a8c41244d918` |

#673 remains release-performance authority (`performance_claim=false` here).
Milestone-length fuzz campaigns stay hosted production-fuzz SUCCESS plus
`fuzz-smoke`; they are not a new long campaign. Independent close review of
this follow-on PR is still required before merge. This ledger sync is
docs-only; do not treat it as further executable delta after `7f77a6f`.

## SPEC-011 IME 37–41 close classification (2026-09-18)

**Pick: covered by existing native / ABC evidence.**

| Fixture | Close classification | Evidence | Explicit limit |
| --- | --- | --- | --- |
| 37 marked text → commit | **covered** | Native `setMarkedText` / `insertText` / `unmarkText` component cases + local ABC dead-key XCUI bytes `c3 a9 78 1b` | Hosted CI `XCTSkip` without ABC layout |
| 38 cancel / abandon | **covered** | Native cancel/abandon callback + local ABC Escape-unmark | Physical non-ABC input sources not exercised |
| 39 replacement commit | **covered** | Native replacement-commit callback | Not a multilingual candidate-list replacement |
| 40 candidate coordinates | **covered** | Native `firstRectForCharacterRange` from the Rust cursor frame | Candidate popup on other displays / mixed scale unclaimed |
| 41 detach discards stale preedit | **covered** | Native `viewWillMove(toWindow:)` / `discardMarkedText` path | Live-composition reconnect across a real GUI crash not claimed |

`CompositionDocument` self-tests remain document invariants only; they are not
this classification. Language-specific input-source breadth stays post-M004
#836 and is **not required** for M002 technical preview. Headed steps 1–10 for
this close package are in the close-out session and manual table above.

## Local native session (2026-09-18, `204d14c`)

Exclusive Runtime was free at launch. `scripts/test-macos-ui.sh`:

| Gate | Result |
| --- | --- |
| `SeyalHostComponentTests` | **26/26 PASS** |
| `SeyalUITests` XCUI runner | A later exclusive-Runtime session on this host **did** enable automation. See 2026-09-18 headed rerun below. |

Do not treat the component PASS as a new headed 1–7 result.

## Local headed rerun (2026-09-18, post `ThinPaneHostView` hit-test)

Exclusive Runtime free. After Flow pane stopped swallowing Block clicks:

| Gate | Result |
| --- | --- |
| `SeyalHostComponentTests` | **26/26 PASS** |
| `testSelectingABlockRevealsRustBlockDetailsInInspector` isolated | **PASS** (26.2s) |
| Same case in full 19-test suite | **PASS** (26.7s / 26.9s) — previously FAIL |
| Workload 4/4 cluster + inspector + history alt-screen | **7/7 PASS** |
| Full native + XCUI (`test-without-building` on this head) | **27/27** component + **19/19** XCUI **PASS**. `testAlternateScreenReturnRestoresFlowNotRawTerminal` **PASS** in the full 19-test sequence (35.6s). See [m002-824-remediation.md](m002-824-remediation.md). |
| Steps 8–10 XCUI | **PASS** (high-volume, Unicode resize, GUI relaunch) |
| Steps 1–7 interactive GUI | **headed Debug `Seyal.app` on `7f77a6f`** — see 2026-09-18 headed GUI session below. Not a physical-keyboard / live Claude TUI oracle. |

Still `Refs #824`. Do not close #824 / #672.

## Headed GUI session (2026-09-18, `7f77a6f`)

Exclusive Runtime free at launch. Production Debug
`target/macos-derived-data/Build/Products/Debug/Seyal.app` (not the XCUI
unsigned runner). Composer/TUI driven through Accessibility + HID key events
on the live window — not a physical keyboard and not Metal pixel inspection.

| Observation | Value |
| --- | --- |
| Runtime pid | **33076** (survived GUI quit) |
| `runtime=` | `afec949f9612dab30000000000000001` |
| `execution=` | `094c09200fb34a0d0000000000000002` |
| `attachment=` | `…0004` before quit, **`…0005` after reopen** |
| Vim/Neovim/htop/tmux | TUI `alternate-screen=true` then Flow restore |
| Limits | nested SSH hop, tmux copy-mode/extra panes, live Claude TUI, physical keyboard |

Still `Refs #824`. Do not close #824 / #672.

Local exact-head gates on `cfbc848` (docs-only after `204d14c`; exclusive
Runtime free):

| Gate | Result |
| --- | --- |
| `python3 scripts/fuzz-smoke.py` | **PASS** — 10 active targets, campaign parity ok |
| First `make check` | **ENVIRONMENT_UNSUPPORTED** — leftover `site/node_modules` Markdown links (gitignored; not present on hosted CI). Moved aside; not a product FAIL. |
| Parallel `pass7_local_ipc` inside `make check` | **INCONCLUSIVE / harness** — `Exec(Io(code: -6))` on `history_range_combining_grapheme_round_trips_over_runtime_wire` (same class as the pre-isolation local flake). Isolated `--test-threads=1` suite **16/16 PASS**. |
| `CARGO_TEST_THREADS=1 make check` | **PASS** including `pass7_local_ipc` 16/16 |

Hosted `rust-and-harness-quality` on later docs pushes is green. Do not treat the
parallel local IPC flake as a current product FAIL; hosted Foundation Quality
on `2ef8332` (run 35305544844) is the current full-suite PASS. `ad50bd1` remains
the first isolation-fix hosted PASS.

## Historical local native verification (pre-remediation, 2026-09-17)

The previous Runtime owner shut down PID 99338 before this run. The native test
application then launched its own bundled Runtime, PID 23758; no foreign Runtime
was terminated. Results below were captured before committing the `issue/824`
patch on `f0e8c01` and **before** the Block-selection / IPC remediations landed.
They are retained for honesty; they are **not** the current-head claim.

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
- **Historical full native run (superseded):** `target/m002-ime-final.xcresult`
  executed **45 tests: 44 passed, one failed, zero skipped**. The failure was
  `testSelectingABlockRevealsRustBlockDetailsInInspector`. Retained in
  [full result summary](m002-824-ime-full-results.json). Cleared on current head
  by live-end follow + hittable wait; see Current-head table above.

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

Historical source fingerprints for the final-v2 / pre-remediation 45-test local
runs (superseded by the Current-head table above):

| Source | SHA-256 (historical) |
| --- | --- |
| `InteractiveMetalSurfaceView.swift` (final-v2) | `d03cd4ad6c640b709c7a97bf4e08b3ede0f75de9d148f8e773653a4963222c32` |
| `ProductChromeHostView.swift` (final-v2) | `a0743a81faf5e81b65e19bd254a997ab4df3990f2517d47b09267a5e4d705de2` |
| `SeyalHostComponentTests.swift` (final-v2) | `c8484732ead7191da6f97c2953cdce27cded167fbf2620b1e985686a776b6052` |
| `InteractiveMetalSurfaceView.swift` (45-test local) | `9f4b8191677d33c20a99b915d638eaee7dddfccd4dc6fd2c3d44687a67259e16` |
| `SeyalHostUITests.swift` (45-test local, pre-hittable wait) | `142473b4f8b997c966fec984676127116b75ee755660bd91fda9e60934a94ef3` |
| `SeyalHostHistoryUITests.swift` | `e14a154bce535a317edb91dde28319318fee07309a338d35dd9d8bfd7ac93a9c` |
| `SeyalHostWorkloadUITests.swift` | `d4e011fe197fdb1a043cff6cb3403916af7a06b456bf5e53631c051dbc7690e0` |

The physical-hardware and multilingual-candidate limitations remain unclaimed; #836 is not
pulled into M002. This verification does not automatically waive #824's other
manual, benchmark, or independent closing-review requirements.

**Historical `make check` note (superseded):** before harness isolation, the
complete local `make check` gate failed twice in `pass7_local_ipc` with
`Exec(Io(code: -6))` while the isolated suite passed. Root cause and fix are
in [m002-824-remediation.md](m002-824-remediation.md). Current-head hosted
`make check` is PASS on `2ef8332` (run 35305544844); `ad50bd1` run
35242876394 remains the first isolation-fix record only. Do not treat the
historical local 44/45 or IPC FAIL rows as the current claim.

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
| 1. zsh/bash/fish interactive | **PASS** close-out session: zsh/bash/fish/color composer submits; window resized to 960×640; composer stayed `available`. Not a raw-PTY line-editor session. Live PTY suite retained PASS. |
| 2. SSH then nested SSH | **PASS** composer submit nested `ssh` to `nt-ssh@orb` then again to `nt-ssh@orb`. One Seyal PTY/execution. Output bytes not AX-visible; live PTY nested fixture retained. |
| 3. Vim/Neovim interactive | **PASS** `/usr/bin/vim -Nu NONE` and `nvim -u NONE` → `alternate-screen=true`; `:qa!` restored Flow. Same `runtime=` / `execution=`. |
| 4. tmux child windows/panes/copy-mode | **PASS** TUI takeover `alternate-screen=true` with `split-window -h` in the spawn. `Ctrl-b [` sent while TUI. Restore to Flow via `Ctrl-C`. tmux hierarchy was not Seyal panes. |
| 5. htop/watch/ncurses | **PASS** `htop -d 10` and `watch -n 1 date` → `alternate-screen=true`; `q` restored Flow / composer `available`. |
| 6. git/docker/kubectl/terraform TTY | **PASS** composer: `git log --oneline --color -n 3`, `docker ps`, `kubectl version --client`, `terraform version`. Spinners/long TTY progress not claimed. |
| 7. CLI-agent TUI | `claude --version` **PASS** on Flow. Live `claude` stayed `alternate-screen=false` / composer `available`. **ENVIRONMENT_UNSUPPORTED** for live Claude/Codex TUI. VT agent-TUI equivalent retained. |
| 8. high-volume while typing/scrolling | headed Flow/Blocks XCUI PASS retained (`testHighVolumeComposerOutputStaysOnFlowBlocks`); this close-out session did not type-while-flood. VT/PTY automated PASS |
| 9. search/copy Unicode after resize | Unicode composer submit + resize **PASS**. ⌃R opened composer command history, not retained-grid search/copy. **ENVIRONMENT_UNSUPPORTED** for headed history search/copy. VT combined fixture retained. |
| 10. GUI close/reopen M001 reconnect | **PASS** GUI quit left helper pid **96808**; reopen GUI pid **2318** `connection=usable` same `runtime=` / `execution=` new `attachment=` (`…0004` → `…0005`). XCUI relaunch case retained PASS. |

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
