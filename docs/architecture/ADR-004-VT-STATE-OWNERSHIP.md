# ADR-004 — Seyal VT parser and authoritative terminal-state ownership

- **Status:** Accepted for M001
- **Date:** 2026-08-24
- **Issue:** #38

## Context

M001 requires the first permanent Seyal-owned VT implementation. The Runtime will eventually route PTY bytes into one authoritative `TerminalState`, then derive damage/projection for native clients. A GUI-owned parser, a second grid, or a temporary emulator would violate the accepted foundation architecture.

RILL contains useful terminal-emulation evidence, especially `crates/vt-engine/src/parser.rs` and `screen.rs`, but its implementation is not architectural authority for Seyal. In particular, RILL's large screen module combines substantially more behavior than M001 requires and carries RILL-specific mutation, reply, grapheme, mode and projection concerns.

## Decision

### One authority

`seyal-terminal` owns portable terminal semantics:

```text
incremental bytes
→ Parser
→ TerminalState mutation
→ generation-based Damage
```

There is exactly one canonical VT/grid/state instance per future `TerminalExecution`. PTY/runtime ownership is outside this crate; rendering/native UI consumes a later derived projection and must not run a second parser or grid.

### Parser and state are separate responsibilities

The parser is a bounded byte-framing state machine. It owns only framing state: escape/CSI/string state, fixed CSI parameter storage and fixed UTF-8 continuation storage. It has no terminal grid.

`TerminalState` owns semantic state: active screen, cells, cursor, style, modes, line identity and damage generation. Parser actions mutate `TerminalState` synchronously within the single-owner Runtime path.

This separation is intentionally data-oriented. Seyal does not add dynamic-dispatch layers, locks or message serialization between parser and state.

### Incremental framing is permanent

Arbitrary PTY read boundaries are semantically invisible. UTF-8 and escape sequences may be split at any byte boundary. CSI parameter storage is bounded; malformed/overlong input is consumed safely and cannot cause unbounded allocation.

OSC/DCS/SOS/PM/APC framing is recognized sufficiently to preserve parser continuity, but M001 does not implement their semantics. Completion increments deferred diagnostics rather than claiming support.

### Screen ownership

`TerminalState` owns a primary screen and, only while active, one minimal alternate screen. `CSI ?1049h/l` is the M001 alternate-screen path. Entering alternate follows the scoped xterm save/switch/clear contract: the primary cursor/rendition remains saved, the clean alternate buffer begins from the active saved pen rendition, and its blank cells carry that pen background with otherwise default cell attributes. Leaving discards the alternate buffer and reveals the preserved primary cursor/rendition state. No output copy or second terminal engine is created.

Full-screen scroll blanking follows the same canonical blank-cell rule: a newly exposed bottom row uses the active pen background and otherwise default attributes. Buffer motion must not accidentally reset visible background color merely because cells were introduced by scrolling rather than ED/EL.

### Stable logical line identity

Each screen row carries a `LineId`. IDs are stable for retained logical rows across ordinary mutation and resize. Full-screen line-feed scrolling moves the existing row identity with the row and allocates a new identity for the new bottom line. Alternate buffers use separate identity namespaces.

This is the M001 seam for future Block anchors and scrollback/reflow. Viewport row numbers are not durable identity.

### Damage

Terminal mutation records bounded row/full-screen damage. A feed/resize transaction commits at most one new monotonic damage generation. If the consumer has not yet taken damage, later generations coalesce row ranges while retaining the newest generation.

Parser/state progress never waits for a renderer acknowledgement.

### Deferred and unknown behavior

Unsupported/deferred sequences must not silently masquerade as supported behavior. Seyal records counters for deferred, unknown and malformed input where practical. Unsupported sequences are consumed without corrupting later supported parsing or panicking.

### Unicode scope

M001 accepts printable Unicode scalar input and incremental UTF-8 correctness. It does **not** claim complete grapheme clustering, emoji composition or East Asian width behavior. The initial cell path therefore preserves scalar content without importing RILL's larger width/grapheme implementation prematurely.

## Consequences

- `seyal-terminal` becomes a permanent production boundary rather than scaffolding.
- The first implementation remains small enough to test and review by responsibility.
- Later VT features extend parser handlers/state; they do not replace the parser architecture.
- Renderer, PTY, runtime IPC, Blocks and persistence stay outside the VT mutation hot path.
- Future projection code can consume cell/state/damage without becoming authoritative terminal state.
- Scroll/alternate blanking use one background-aware cell rule, avoiding renderer-side repair or a second semantic interpretation.

## RILL salvage review

### Preserved after validation

- parser state survives arbitrary byte chunking;
- parser holds no grid;
- explicit screen/cursor/style state;
- non-panicking consumption of unsupported escape families;
- pending-wrap behavior for printable output;
- primary/alternate separation;
- cursor/grid clamping on resize;
- active-background blanking for scroll-created and newly cleared alternate cells, after revalidation in Issue #68.

### Corrected or redesigned

- RILL's large `screen.rs` is decomposed into cohesive Seyal modules;
- RILL-specific `PodGrid`/diff/reply coupling is replaced by Seyal-native state plus generation damage;
- RILL mutation-test production hooks are not imported;
- deferred behavior is not exposed as supported M001 functionality;
- logical line identity is made explicit from the start for Block/history compatibility;
- Issue #68 corrected two rewrite regressions where the initial Seyal salvage accidentally used default rendition/background for scroll-created and alternate-screen cells instead of the active saved pen semantics.

### Rejected/deferred

- RILL names and type ownership;
- separate `rill-vt-types` style duplication;
- broad mouse/application modes, device replies, OSC semantics and scroll-region editing;
- RILL grapheme/East-Asian-width implementation for M001;
- projection/repaint byte generation inside the VT state boundary.

## Revisit conditions

Revisit this ADR only if evidence shows that the single-owner parser/state model cannot meet terminal correctness/performance, or if a future platform requires a materially different parser/state ownership model. Adding more VT sequences is not by itself a reason to revisit the decision.
