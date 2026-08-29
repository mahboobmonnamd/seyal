# M003 Command Blocks and Pane Composer — Reference Design

**Status:** Accepted design authority for ADR-009 / SPEC-008

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
| window | full window | dark Seyal shell with native traffic lights and centered title |
| workspace-sidebar | left column | Workspace list, active state, path/detail and tab counts |
| tab-strip | top content | active tab plus sibling tabs and new-tab affordance |
| pane | center content | focused Pane with one command/output transcript |
| completed-block | center transcript | command header, status, elapsed time, actions and output |
| running-block | center transcript | active command, running indicator and live terminal output |
| busy-strip | below transcript | real foreground-process state and stop/split guidance |
| pane-composer | bottom of Pane | one command editor with prompt, hint and execute affordance |
| inspector | right column | Workspace, Tab, Pane, Process and resource context |
| separators | region boundaries | thin, low-contrast structural dividers |
| focus | active controls | accent border/indicator for active tab, Pane and Block |

Unknown from the image: shell-integration protocol, command identity format,
exact scroll behavior, accessibility tree, persistence, failure semantics,
multi-pane execution ownership, and whether the source is native. These must be
defined by SPEC-008 and runtime evidence, not inferred from pixels.

## Component and state model

```text
SeyalWindow
└─ Workspace / Tab / Pane
   ├─ PaneTranscript (single scroll owner)
   │  └─ CommandBlock × N
   │     └─ canonical Metal terminal-history projection
   ├─ BusyStrip (when real foreground state exists)
   └─ PaneComposer (exactly one per Pane)

TUI takeover
└─ same Pane / ExecutionId / Metal surface
   ├─ Block chrome hidden
   └─ composer hidden
```

## Measurement/token starting point

The image has no trusted scale-factor metadata. Preserve the 1448×1085 source
dimensions for comparison and derive reusable tokens only after native captures
confirm repeated relationships. Initial observable relationships are: sidebar
and inspector are fixed-width context regions, Pane transcript is dominant,
Block borders are thin with rounded corners, command metadata is compact, and
the composer is Pane-local and bottom anchored.

## Visual regression states

1. one completed command Block plus empty/available composer;
2. two sequential completed Blocks;
3. one running Block plus busy strip;
4. command failure Block;
5. TUI takeover with composer/chrome yielded;
6. TUI exit returning to the same Pane transcript;
7. light/dark and keyboard-focus/accessibility states.

## Implementation dependency graph

```text
ADR-009 + SPEC-008 acceptance
  → trusted shell integration / bounded command events
  → Runtime/Workspace BlockTimeline + protocol projection
  → client Block cache and Pane transcript layout
  → Pane composer eligibility/submit/failure/focus
  → TUI takeover integration
  → accessibility + headed UI tests
  → screenshot convergence + Pass 7 regression benchmark
```
