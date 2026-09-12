# ADR-015 — Rust Product UI and Thin Swift Host

- **Status:** Accepted on merge
- **Date:** 2026-09-12
- **Issue:** #877
- **Depends on:** `R-010`, `R-011`, `R-012`, ADR-004, ADR-009, ADR-011, UI architecture
- **Coordinates with:** #875 / PR #876 agent and review enforcement

## Decision requested

This repository keeps one mixed-language macOS application. It does **not** extract
Swift UI to another tree and does **not** delete Swift.

Ownership is split by *authority*, not by deleting a language:

```text
Rust owns portable Seyal product/UI state and behavior.
Swift owns only inherently macOS platform integration.
```

There is no second Swift UI product elsewhere. Future Windows/Linux hosts must
implement the same portable Rust contract; they must not reimplement Seyal
product semantics.

## Problem and conflict

`R-010` assigns macOS application/platform behavior to Swift + AppKit. That is
correct for windowing, IME, accessibility and Metal drawable hookup. It is not
a license for Swift to own Workspace/Tab/Pane models, Blocks, composer,
commands, focus, layout or agent/inspector behavior.

#875 / PR #876 already made that split a merge-blocking agent/review rule.
This ADR is the independent architecture decision those rules enforce.

A prior reading of #877 that would remove all Swift sources, or isolate a
separate Swift UI repository, is rejected.

## Alternatives considered

### A. Remove all Swift from this repository

Rejected. IME (`NSTextInputClient`), accessibility, AppKit window lifecycle and
Metal drawable creation remain first-class macOS integration. Deleting Swift
would force a premature universal GUI or a weaker input/accessibility path.

### B. Extract Swift UI to a sibling repository / superproject tree

Rejected. Product intent is one OSS application with a thin native host, not
two UI products. A second tree would duplicate packaging, CI and the
Rust↔native boundary without changing terminal authority.

### C. Keep product UI in Swift because the macOS host already exists

Rejected. That is the drift #875 forbids: Windows/Linux hosts would have to
rewrite Seyal feature logic. Passing XCTest/XCUI and local convenience are not
architecture.

### D. Rust product/UI authority; Swift thin OS adapter in this repository

Selected. Matches `R-010` native-host intent, `R-012` (no premature universal
GUI framework), and Metal as the production renderer (`R-011`).

## Normative split

### Swift MAY own

- `NSApplication` / `NSWindow` lifecycle
- native event collection
- IME / `NSTextInputClient` bridge
- accessibility adapter
- clipboard / drag-drop
- Metal drawable/surface creation
- other inherently macOS-specific APIs

Swift may render pixels and forward events. Presentation state shown by Swift
must be derived from the authoritative Rust model.

### Swift MUST NOT own

- Workspace / Tab / Pane model
- Block state/lifecycle
- composer behavior
- command submission semantics
- keyboard command decisions
- focus/navigation policy
- split/layout state
- Agent / Inspector state
- other Seyal feature/business logic

If a behavior would have to be rewritten for a second desktop host, it is
Rust-owned. Ambiguous ownership is an architecture stop, not a Swift default.

## Invariants that remain true

1. One `TerminalExecution` owns one PTY and one canonical `TerminalState`.
   The GUI never owns a second VT/grid.
2. Flow / Raw / TUI remain presentations over the same execution.
3. Metal remains the production macOS terminal renderer. No NSTextView,
   SwiftUI, or CPU-full-frame terminal engine (`R-011`).
4. No per-cell Rust↔Swift callback on the terminal hot path.
5. IME preedit stays ephemeral host state as required by ADR-011. Swift may
   own the `NSTextInputClient` bridge; it may not commit preedit into
   `TerminalState`.
6. Headless Runtime survives GUI detach/crash.
7. This ADR does not choose a new Rust UI toolkit. Toolkit/metrics changes
   are follow-up implementation Issues after this decision is accepted.

## Required follow-up (not this PR)

- Implementation Issues that move any remaining product/UI authority out of
  Swift and into Rust, without deleting the thin host.
- Re-baseline headed UI latency/CPU/RSS/GPU metrics when a new Rust UI
  approach lands. This ADR does not by itself change production metrics.
- Do not mix those implementation PRs with further ADR edits.

## Reopen conditions

Reopen only if measured IME, accessibility, or Metal-lifecycle evidence shows
that a listed Swift MAY item cannot remain a thin adapter, or that a listed
Swift MUST NOT item cannot be expressed as portable Rust state without
violating hot-path or `R-011` constraints.

Approved by product authority on 2026-09-12. Independent architecture review
remains required before merge.
