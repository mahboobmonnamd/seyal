# M001 Multipane View

**Status:** Frozen UI reference specification  
**Parent:** `M001-CORE-TERMINAL-REFERENCE-SCREEN.md`  
**Scope:** Tab-owned split layout and per-pane terminal interaction

## 1. Purpose

The Multipane view defines how one workspace tab presents multiple panes without losing Seyal's terminal ownership, composer, inspector, or power-user density rules.

## 2. Canonical ownership

```text
Workspace
└── Tab
    └── Split tree
        ├── Pane
        │   └── TerminalExecution / non-terminal surface
        └── Pane
            └── TerminalExecution / non-terminal surface
```

Rules:

- the tab owns the split tree/layout;
- a terminal pane owns at most one `TerminalExecution`;
- each `TerminalExecution` owns exactly one PTY/child/canonical terminal state;
- splitting creates another pane and, when a terminal surface is requested, another execution/PTy according to product policy;
- splitting never duplicates or mirrors a VT engine.

## 3. Layout controls

The top tab row exposes functional split controls such as:

- split right;
- split down;
- layout selector only when supported.

The controls apply to the currently focused pane within the active tab.

No decorative layout buttons.

## 4. Pane chrome

Each pane should remain compact.

Useful pane-level context may include:

- compact execution/process title;
- running/attention indicator;
- focused-state outline/accent;
- minimal pane actions through overflow/context menu.

Avoid repeating the full global tab bar or workspace nav inside each pane.

## 5. Composer rule

**Every terminal pane has its own pane-scoped composer.**

Therefore a 2x2 terminal layout has four composers, one per terminal pane.

Rules:

- focused pane composer is visually dominant and receives keyboard interaction;
- inactive pane composers remain visible enough to communicate that commands are pane-local, but should be subdued to reduce noise;
- composer draft/history context is stored per pane;
- typing in one pane must never route to another pane because it was last active elsewhere.

A full-screen TUI takeover may temporarily replace/hide that pane's composer as defined in the TUI specification.

## 6. Block and raw/TUI coexistence

Different panes may simultaneously show different presentations of their own executions:

- normal Blocks;
- long-running streaming Block;
- raw terminal;
- full-screen TUI takeover.

No pane presentation may change terminal authority or affect another pane's PTY/VT state.

## 7. Focus

Exactly one pane is focused for primary keyboard input at a time.

Focus determines:

- active composer;
- keyboard target;
- split target;
- inspector default context;
- pane-scoped action target.

Mouse click and keyboard navigation must both update the same focus state.

## 8. Inspector

The right inspector follows the focused pane unless the user explicitly selects a Block/process/agent/resource.

For a focused terminal pane, useful inspector content:

- pane identity;
- layout/focus state;
- shell/cwd;
- execution ID/state;
- active foreground process;
- process/resource data.

Changing focus between panes should update inspector context without destroying pane state.

## 9. Left-panel pane tree

Do not render a deeply nested pane tree in the left sidebar by default.

For 15-inch power-user layouts:

- keep Workspaces, Agents and Tabs stable;
- show a compact pane count/status on the active tab where useful;
- provide pane navigation/tree only as an invoked view/overlay when complexity warrants it.

This prevents duplicate navigation and preserves horizontal space.

## 10. Resize

Dragging split boundaries changes pane geometry.

For terminal panes, resize proposals eventually flow through the authoritative Runtime/terminal resize path. UI must not present geometry that Runtime/PTY rejected.

Rendering and resize feedback should remain responsive without creating synchronous semantic work on PTY I/O.

## 11. Long-running commands

If one pane is occupied by `npm run dev`, other panes remain independently usable.

The busy pane:

- keeps streaming its active Block/live output;
- keeps its own composer state but cannot run an unrelated command in that same foreground shell until the process exits/is interrupted;
- does not block another pane's shell/composer.

## 12. Non-terminal panes

Future panes may host agent/activity/diff/artifact/inspector-like surfaces without owning a PTY.

The layout model must distinguish terminal vs non-terminal surfaces explicitly; do not create hidden PTYs merely because a pane exists.

## 13. Performance

Multipane must scale with visible work without:

- one renderer loop/thread per pane;
- duplicate terminal state;
- full-pane redraws when damage is localized;
- synchronous cross-pane coordination for terminal progress.

Focus/layout/UI metadata is not allowed to become a dependency of PTY → VT → canonical state progress.
