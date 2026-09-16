# M002 #842 current-head closing evidence — 2026-09-16

| Field | Value |
| --- | --- |
| Source head | `2444c78` (`origin/master`) |
| Host | Darwin arm64 / macOS 26.5.2 / Rust 1.98.0 |

HistoryStore implementation and headed resize/alt-screen cases are already on master. This record is the current-head evidence package for independent review of remaining #819 acceptance gates owned by this follow-up.

## Portable history

SPEC-010 §17 fixture gaps and HistoryStore regressions are on master. Comparative StatsAlloc matrix notes are retained under `docs/evidence/m002-history-842-comparative-bd99e15.md` with `performance_claim=false`.

## Headed Flow/Blocks

From the exclusive-Runtime `scripts/test-macos-ui.sh` run on this host:

```text
SeyalHostHistoryUITests 2/2 in 58.129s
  testAlternateScreenExitRestoresFlowHistorySurface   passed (28.109s)
  testLongOutputAndResizeStayOnFlowBlocks             passed (30.020s)
```

Long numbered ASCII/CJK/emoji output, narrow then wide resize, and alternate-screen exit all kept composer + Blocks. Metal does not expose PTY bytes as AX text; these cases are surface/continuity oracles.

## Reviewer verification

1. Interpret remaining comparative HistoryStore rows against the versioned performance contract (five-cohort promotion is not claimed here).
2. Repeat the headed history cases on this exact head with an exclusive Runtime.
3. Independent exact-head review of HistoryStore + this evidence.

Documentation impact: N/A for User Guide (no new setting). This file is developer evidence only.
