# M001 Flow/Blocks default and full-screen TUI takeover

**Status:** Historical M001 decision; presentation details are subordinate to ADR-009/SPEC-008 and proposed #858 amendment
**Parent:** `SEYAL-UI-ARCHITECTURE-001`, `M001-CORE-TERMINAL-REFERENCE-SCREEN.md`
**Date:** 2026-08-28

> **Supersession notice for #858:** this M001 document established Flow/Blocks as
> the product default, but its references to one bridge-backed surface and Raw
> as only a diagnostic/TUI fallback are not current presentation authority.
> Under the proposed ADR-009 amendment, Flow, Raw and TUI are mutually exclusive
> presentations of the same `TerminalExecution`: Flow has no visible/focusable
> terminal viewport underneath it; Raw is the full-Pane safe fallback whenever
> structured/direct-input semantics cannot be proven; TUI is full-Pane takeover.
> Presentation transitions must fence the old first-responder/IME/mouse/input
> route before enabling the new route. ADR-009/SPEC-008 win on conflict.

The first production slice originally launched this shell by default with one
real bridge-backed surface and one Block. Fixture workspaces remain restricted
to the explicit debug preview. Runtime workspace/BlockTimeline metadata and
physical AppKit acceptance remain follow-up gates.

## Decision

Seyal's normal supported structured-shell presentation is **Flow/Blocks**.

The default window presents the Workspace → Tab → Pane composition. Normal
shell commands, workflows, agent activity, artifacts and results are presented
as Blocks in the Pane transcript when trusted structured presentation is safe.
Raw and TUI remain replacement presentations of the same execution rather than
coexisting layers under Flow.

Blocks are presentation over real Runtime/workspace metadata keyed by the same
`ExecutionId`. A Block never owns a PTY, VT parser, terminal grid, child
process, renderer or copied terminal authority. Block grouping and metadata are
observed asynchronously and cannot block PTY → VT → canonical state → damage
progress.

## Full-screen TUI exception

Full-screen applications—including Neovim/Vim, `htop`, tmux and terminal-backed
agent applications such as Claude Code when they require full-screen terminal
ownership—temporarily take over the **same Pane**.

```text
one Workspace / Tab / Pane
        │
        ├─ structured-safe state: Flow/Blocks transcript
        ├─ direct-terminal fallback: full-Pane Raw
        └─ canonical TUI state: full-Pane TUI
```

Takeover is a presentation state transition, not a second execution, PTY, VT
engine or grid. The transition is driven by canonical terminal behavior/state
(for example alternate-screen/full-screen ownership), not by process-name
allowlists or output scraping.

While takeover is active:

- Block chrome and the Pane composer yield;
- the same execution receives keyboard, mouse and resize input;
- the TUI owns terminal scrolling and spatial interaction;
- other Panes retain their own independently selected presentation state.

When canonical TUI state exits, the same Pane re-evaluates whether Flow or Raw
is currently valid; it does not create a new Block stream from alternate-screen
frames.

## Consequences

- `SeyalShellView` is the production default shell, not a debug fixture path.
- Preview fixtures may remain only as isolated design-test data and must never
  be the normal application launch path or claim Runtime state.
- The production shell needs Runtime workspace/BlockTimeline metadata before
  the one-execution projection can grow beyond its honest single local
  Workspace/Tab/Pane/Block shape.
- Raw means a mutually exclusive full-Pane direct-terminal presentation for
  unsupported/unsafe structured interaction or explicit diagnostic use; it is
  never a hidden input viewport underneath Flow.
- TUI is a mutually exclusive full-Pane application takeover.
- Warp behavior may be researched for observable takeover, scrollback, focus,
  and return semantics. Seyal must not copy Warp's architecture or introduce a
  second terminal/grid authority.

## Required implementation evidence

Before the corrected implementation is accepted, record:

1. a real Runtime-backed default Flow/Blocks launch with no coexisting raw
   terminal rectangle/input target;
2. one Pane retaining the same `ExecutionId` across Flow, Raw and TUI states;
3. canonical alternate-screen entry/exit and composer/focus transitions;
4. source input/focus/IME/mouse route revoked before destination route opens;
5. stale execution/attachment/presentation eligibility rejected;
6. a real normal-screen command represented as a Block without output copying;
7. representative Raw fallback and agent/TUI cases, including Claude Code and
   Neovim, classified from trusted/canonical state rather than process-name
   scraping;
8. independent architecture, security, and native UI review.
