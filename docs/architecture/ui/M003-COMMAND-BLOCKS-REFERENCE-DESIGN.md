# M003 Command Blocks and Pane Composer — Reference Design

**Status:** Accepted design authority for the ADR-009 / SPEC-008 baseline; #858 presentation correction proposed until merge

## Source visual

Approved user reference: `docs/architecture/ui/references/1-full view terminal.png`.
Original dimensions: 1448×1086 pixels. Scale factor, native window size,
appearance, font rasterization and source implementation are unknown; the image
is treated as a product-behavior and composition reference, not byte-identical
rendering authority.

The reference appears to be a dark terminal-workspace concept/mockup rather
than verified native AppKit output. Product architecture and macOS conventions
remain higher authority where the image is silent or conflicting.

## Visible inventory

| ID | Region | Observable contract |
|---|---|---|
| window | full window | Seyal shell with native traffic lights and centered title |
| workspace-sidebar | left column | Workspace list, active state, path/detail and tab counts |
| tab-strip | top content | active tab plus sibling tabs and new-tab affordance |
| pane | center content | one active presentation: Flow, Raw or TUI |
| completed-block | Flow transcript | command header, status, elapsed time, actions and terminal-derived output |
| running-block | Flow transcript | active command, running indicator and live output clipped to that Block |
| busy-strip | Flow | real foreground-process state and stop/split guidance |
| pane-composer | Flow bottom | one command editor with prompt, hint and execute affordance |
| raw-terminal | Raw only | full-Pane primary terminal interaction; never visible under/beside Flow |
| tui-terminal | TUI only | full-Pane application presentation; Flow/composer yielded |
| inspector | right column | Workspace, Tab, Pane, Process and resource context |
| separators | region boundaries | thin, low-contrast structural dividers |
| focus | active controls | accent border/indicator for active tab, Pane and Block |

Unknown from the image: shell-integration protocol, command identity format,
exact scroll behavior, accessibility tree, persistence, failure semantics,
multi-pane execution ownership, and whether the source is native. These must be
defined by SPEC-008 and runtime evidence, not inferred from pixels.

## Component and state model

Exactly one presentation is active for a terminal Pane:

```text
SeyalWindow
└─ Workspace / Tab / Pane
   └─ PanePresentation
      ├─ FlowPresentation                 [active when structured-safe]
      │  ├─ PaneTranscript (single scroll owner)
      │  │  └─ CommandBlock × N
      │  │     └─ BlockOutputRegion
      │  │        └─ canonical terminal/history projection
      │  ├─ BusyStrip (when real foreground state exists)
      │  └─ PaneComposer (exactly one per Pane)
      │
      ├─ RawPresentation                  [replacement mode]
      │  └─ full-Pane primary terminal grid + direct input
      │
      └─ TUIPresentation                  [takeover mode]
         └─ full-Pane alternate/full-screen grid + direct input
```

The renderer implementation may reuse one Pane-owned Metal compositor across
these states. That is not a user-visible fourth layer.

### Flow compositor rule

A single Pane Metal drawable may efficiently compose many visible Block output
regions. In Flow:

```text
Block geometry
  -> region clips
  -> one Pane compositor
     -> completed history range pixels inside completed Block clips
     -> running live-tail pixels inside running Block clip
     -> no unrelated primary-grid pixels outside those clips
```

There is no traditional terminal viewport behind the transcript. Empty canvas
belongs to Seyal UI. It must not reveal a terminal background/cursor or route
clicks/typing through to a raw terminal input target.

This preserves both goals:

- one canonical terminal authority / no terminal engine per Block;
- native Block-first presentation rather than a conventional terminal wrapped
  in Block chrome.

### Raw transition

If structured command entry is unavailable/unsafe or arbitrary character-level
terminal interaction is required:

```text
Flow
  -> fence/revoke Flow input, IME, focus and mouse route
  -> validate Raw against current execution/attachment/canonical state
  -> Raw fills the Pane and acquires direct input
  -> same ExecutionId / PTY / TerminalState
```

When Raw can end, eligibility is re-evaluated. Existing trusted Block history is
preserved; raw terminal text is not scraped to fabricate Block boundaries.

### TUI takeover

When canonical alternate/full-screen state becomes active:

```text
Flow or Raw
  -> fence/revoke previous input/focus/IME/mouse route
  -> validate current alternate/full-screen state
  -> TUI fills the Pane and acquires application input
  -> same ExecutionId / PTY / VT / alternate state
  -> TUI exits
  -> fence TUI route
  -> re-evaluate Flow vs Raw
```

"Same execution" is the continuity requirement. The architecture does not
require one permanently installed/focusable AppKit terminal view underneath all
three modes.

## Input/focus model

### Flow

- composer owns supported structured command text;
- Block controls own only their explicit interactions;
- empty transcript space does not become raw terminal input;
- arbitrary character-level shell/application interaction requires Raw unless a
  typed Flow action explicitly preserves the required semantics;
- accessibility exposes Block/composer structure rather than a competing
  full-Pane terminal focus target.

### Raw/TUI

- terminal presentation owns first responder, IME/direct key routing, mouse
  reporting where enabled, cursor and terminal selection semantics;
- no Flow Block/composer control may intercept application input.

### Transition fence

The source mode stops admission and releases first responder, IME/preedit and
mouse routing before the destination mode is enabled. Presentation-sensitive
eligibility/admission is valid only for the current `ExecutionId`, fresh
attachment/controller authority, presentation epoch and relevant canonical /
trusted-integration generation. Stale callbacks fail closed and one native event
may reach at most one mode.

## Measurement/token starting point

The image has no trusted scale-factor metadata. Preserve the 1448×1086 source
dimensions for comparison and derive reusable tokens only after native captures
confirm repeated relationships. Initial observable relationships are: sidebar
and inspector are fixed-width context regions, Pane transcript is dominant,
Block borders are thin with rounded corners, command metadata is compact, and
the composer is Pane-local and bottom anchored.

Normal Flow screenshots must not contain a separate black/raw terminal rectangle
that is not itself a Block output region.

## Visual regression states

1. one completed command Block plus empty/available composer;
2. two sequential completed Blocks;
3. one running Block with live output confined to its Block;
4. long-output Block with bounded/scrollable output;
5. command failure Block;
6. Flow empty-canvas hit test: no raw-terminal focus/input;
7. Flow→Raw with pending key/IME: source route revoked before destination opens;
8. stale Flow eligibility after reconnect/controller change: fails closed;
9. full-Pane Raw fallback with Flow interaction yielded;
10. TUI takeover with composer/Flow yielded;
11. TUI exit returning to Flow or Raw based on current eligibility;
12. light/dark and keyboard-focus/accessibility states;
13. splits/tabs where each Pane independently owns one active presentation.

## Implementation dependency graph

```text
accepted ADR-009 baseline + proposed #858 amendment + SPEC-008
  -> explicit Pane Flow|Raw|TUI state + input-route epoch contract
  -> trusted shell integration / bounded command events
  -> Runtime/Workspace BlockTimeline + completed/live-tail projection
  -> Pane compositor with Block-region clipping
  -> Pane composer eligibility/submit/failure/focus
  -> Raw replacement input/render path
  -> TUI takeover integration
  -> accessibility + headed UI tests
  -> screenshot convergence + renderer performance regression benchmark
```

Do not continue UI work by adding more chrome around the current coexisting
Pane-wide interactive terminal surface. Correct the presentation boundary first.
