# M002 #819 headed macOS history/reflow evidence

- **Date:** 2026-09-11
- **Host:** macOS 26.5, arm64, local headed session
- **Branch:** `issue/819`
- **Parent production SHA at test time:** `2a5ebc535dc883dffb9e68113b9ae24cadc1b50f`
- **Does not prove current HEAD (`65be0cf`).** Headed XCUITests were collected on
  parent `2a5ebc5`. Later source commits, including `65be0cf` (exact compacted
  eviction identities), are covered by focused terminal tests. A live Runtime
  from this worktree was still attached during this pass, so the five headed
  XCUITests were not re-run against `65be0cf`. Hosted `native-macos-smoke` is
  mechanics, not Metal/theme headed proof. SPEC-010 §18.1 / #842 remain open.
- **App:** `target/macos-ui-tests/Build/Products/Debug/Seyal.app`
- **Runtime:** separately owned `target/debug/seyal-runtime` (`/bin/zsh`)
- **Result bundle:** `target/macos-ui-tests-819-headed.xcresult`
- **Classification:** headed production XCUITest evidence; not SPEC-010 §18.1 ARM64 acceptance

This record supersedes the earlier partial/ENVIRONMENT_UNSUPPORTED ledger in
[`m002-history-819-headed-manual.md`](m002-history-819-headed-manual.md) for the
five #819 manual steps. Composer max-height, Metal light-theme plumbing, and
Warp-like Block chrome were explicitly out of this work.

## How

`xcodebuild test-without-building` against the production `Seyal` scheme, with
only these XCUITests enabled:

| Step | Test |
| --- | --- |
| Scrollback | `testProductionScrollbackKeepsOlderHistoryReadable` |
| Resize reflow | `testProductionResizeReflowsAsciiCjkEmojiLine` |
| Hard vs soft | `testProductionHardBreaksSurviveResizeWhileSoftWrapsRejoin` |
| Alternate screen | `testProductionAlternateScreenLeavesPrimaryHistoryClean` |
| Live resize | `testProductionLiveResizeWhileOutputPrintsStaysCoherent` |

Each test attaches `XCTAttachment` PNGs (`m002-819-headed-*`) with
`lifetime = .keepAlways`. Canonical hard/soft lineage at the SPEC-010 column
ladder is also covered by
`hist_resize_oscillation_at_spec_column_ladder` in
`crates/seyal-terminal/tests/history_store_regressions.rs`.

## Results

| Manual step | Result | Duration |
| --- | --- | ---: |
| Thousands of numbered lines; scroll to old output | PASS | 9.459s |
| Long ASCII + CJK + emoji line; narrow/wide resize | PASS | 17.411s |
| Hard newline vs autowrap rejoin | PASS | 17.006s |
| vim/htop-style alternate-screen exclusion (`DECSET 1049`) | PASS | 11.896s |
| Resize while output is arriving | PASS | 13.219s |

`** TEST EXECUTE SUCCEEDED **` on 2026-09-11.

**PASS here means HistoryStore/reflow/scroll/alt-screen mechanics**, not that
the Metal pane matches the light window theme.

Alternate-screen coverage uses the production `?1049h` / `?1049l` path and the
Metal surface `alternate-screen=` recovery field. That is the same VT switch
`vim`/`htop`/`nvim` use; those binaries are not required on the test host.

## Limits

- Headed XCUITests prove scroll, resize geometry, connection health, Block
  creation, and alternate-screen enter/leave. They do **not** assert that the
  Metal result area uses the active AppKit theme.
- The production renderer still clears to a hard-coded near-black default
  (`MetalTerminalRenderer` clear color ≈ rgb(11,13,16), default cell
  background `0xff10_0d0b`) while light chrome uses
  `SeyalProductPalette.light(.canvas)` ≈ white. After Enter, glyphs can appear
  as light text on a dark pane. That is a known presentation gap, **out of
  #819 HistoryStore scope**. These tests would still PASS if that pane stays
  dark; they are not a visual-theme gate and must not be cited as one.
- They also do not OCR Metal glyphs; grapheme/lineage correctness remains in
  the HistoryStore regressions and fuzz adapter.
- This does **not** close SPEC-010 §18.1 physical ARM64 p50/p95/p99 gates or
  the remaining #842 evidence matrix. Keep `performance_claim=false`.
