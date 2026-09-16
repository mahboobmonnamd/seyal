# M002 #819 current-head closing evidence — 2026-09-16

| Field | Value |
| --- | --- |
| Source head | `2444c78` (`origin/master`) |
| Host | Darwin arm64 / macOS 26.5.2 / Rust 1.98.0 |

HistoryStore production code is already on master. Remaining acceptance was tracked as a follow-up. This record is the current-head parent evidence package for independent review.

## Portable history

HistoryStore regressions and SPEC-010 §17 fixture gaps are on master (`history_store_regressions` plus the 2026-09-16 fixture fill).

## Headed Flow/Blocks

From the exclusive-Runtime `scripts/test-macos-ui.sh` run on this host:

```text
SeyalHostHistoryUITests 2/2 in 58.129s
  testLongOutputAndResizeStayOnFlowBlocks
  testAlternateScreenExitRestoresFlowHistorySurface
```

## Reviewer verification

Independent exact-head review of HistoryStore on `2444c78` plus the headed continuity cases. Comparative benchmark rows remain `performance_claim=false`.

Documentation impact: N/A for User Guide (no new setting). This file is developer evidence only.
