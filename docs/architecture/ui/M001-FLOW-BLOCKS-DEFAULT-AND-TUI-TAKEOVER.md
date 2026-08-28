# M001 Flow/Blocks default and full-screen TUI takeover

**Status:** Decision adopted for the first production slice; implementation is partial pending native/manual acceptance
**Parent:** `SEYAL-UI-ARCHITECTURE-001`, `M001-CORE-TERMINAL-REFERENCE-SCREEN.md`
**Date:** 2026-08-28

The first production slice now launches this shell by default with one real
bridge-backed surface and one Block. Fixture workspaces remain restricted to
the explicit debug preview. Runtime workspace/BlockTimeline metadata and
physical AppKit acceptance remain follow-up gates.

## Decision

Seyal has one normal application presentation: **Flow/Blocks**.

The default window presents the Workspace → Tab → Pane composition. Normal
shell commands, workflows, agent activity, artifacts and results are presented
as Blocks in the Pane transcript. There is no separately selectable Raw
terminal application path competing with the Block experience.

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
        ├─ normal state: Flow/Blocks transcript
        └─ canonical TUI state: full-pane terminal surface
```

Takeover is a presentation state transition, not a second application path,
execution, PTY, VT engine or grid. The transition is driven by canonical
terminal behavior/state (for example alternate-screen/full-screen ownership),
not by process-name allowlists or output scraping.

While takeover is active:

- Block chrome and the Pane composer yield;
- the same execution receives keyboard, mouse and resize input;
- the TUI owns terminal scrolling and spatial interaction;
- other Panes remain in normal Flow/Blocks presentation.

When canonical TUI state exits, the same Pane returns to its Block transcript;
it does not create a new Block stream from alternate-screen frames.

## Consequences

- `SeyalShellView` is the production default surface, not a debug fixture path.
- Preview fixtures may remain only as isolated design-test data and must never
  be the normal application launch path or claim Runtime state.
- The production shell needs Runtime workspace/BlockTimeline metadata before
  the one-execution projection can grow beyond its honest single local
  Workspace/Tab/Pane/Block shape.
- Existing references to Raw mode mean the terminal surface used during TUI
  takeover or an explicit diagnostic fallback; Raw is not a peer default mode.
- Warp behavior may be researched for observable takeover, scrollback, focus,
  and return semantics. Seyal must not copy Warp's architecture or introduce a
  second terminal/grid authority.

## Required implementation evidence

Before the full implementation is accepted, record:

1. a real Runtime-backed default Flow/Blocks launch;
2. one Pane retaining the same `ExecutionId` across normal and TUI states;
3. canonical alternate-screen entry/exit and composer/focus transitions;
4. a real normal-screen command represented as a Block without output copying;
5. representative agent/TUI cases, including Claude Code and Neovim, with the
   classification based on observed terminal behavior;
6. independent architecture, security, and native UI review.
