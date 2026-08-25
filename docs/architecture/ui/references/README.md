# Historical UI visual references

The images in this directory are **historical design inputs**, not current implementation visual authority.

The current frozen Core Terminal direction is defined by:

- `../M001-CORE-TERMINAL-REFERENCE-SCREEN.md`
- `../M001-CORE-TERMINAL-REFERENCE-INDEX.md`
- the companion state specifications linked from that index
- the frozen Core Terminal reference image owned by the current design PR

Product constitution, accepted architecture/ADRs/specifications, milestone contracts, terminal correctness, and pass ordering remain higher authority than any mockup.

## Why these references remain

The earlier references helped establish useful interaction ideas, but the later Core Terminal design changed the information architecture and removed some earlier visual/behavioral choices.

Do **not** implement these screenshots literally. Non-conflicting capabilities have been carried forward into the current textual specifications.

## Retained capabilities from the earlier references

### Multiline Pane composer

Retained:

- one composer state per terminal Pane;
- multiline command/script/pipeline editing;
- `Shift+Return` as intended newline behavior;
- explicit execute action/shortcut;
- composer retracts/disables when the foreground shell cannot accept a new command;
- full-screen TUI takeover hides/disables the composer and uses the same `TerminalExecution`.

The current design intentionally keeps default composer chrome smaller than the old component mockup. cwd/shell/utility controls appear only when they are genuinely actionable and not redundant with Pane context.

### Workspace / Tab / Pane structure

Retained:

- multiple Workspaces;
- Workspace-scoped Tabs;
- Tab-owned nested Pane layout;
- one pane-local composer per available terminal Pane;
- adaptive/scrollable top Tab strip;
- Block-based normal command/output presentation.

### Global command palette

Retained as a keyboard-first global navigation/action surface. A permanent command-palette toolbar button is not required.

### Agent attention

Retained as the structured notification/attention model. Approval/question is one typed attention case among failures, completions, disconnects, policy/security decisions, and review-ready events.

### Right-side utility concepts

Earlier Activity/Files/Agent utility concepts are reconciled into the **contextual Inspector modes**:

- Context / Info;
- Process / Runtime;
- Files / Diff / Artifacts;
- Activity / Attention / History.

Do not duplicate the full agent inventory in both the left panel and inspector.

### Connection/runtime state

Retained as real product state. Healthy local state may be subtle; remote/detached/reconnecting/degraded state must be surfaced clearly.

## Explicitly superseded behavior

The following old-reference details are **not current authority**:

- fixed maximum Block height with internal output scrolling;
- nested scrolling inside a long-running Node/log Block;
- a global/shared command composer;
- a fully active composer while its foreground shell is occupied;
- permanent split-control clusters repeated inside every Pane;
- duplicate full agent lists in left and right sidebars;
- large utility/control rows added only for visual completeness.

### Current Block scrolling rule

Normal Block/transcript presentation has **one scroll owner: the Pane**.

- Blocks grow intrinsically with output;
- long-running normal-screen output grows its Running Block;
- users scroll the Pane to inspect earlier Blocks/output;
- no fixed max-height output box is introduced just to constrain command output;
- implementation may virtualize off-screen transcript content without exposing nested scroll regions.

### Current TUI rule

A full-screen alternate-screen TUI is not a growing Block with an internal scrollbar.

- the TUI takes over the Pane viewport;
- Block chrome yields;
- composer is hidden/disabled;
- canonical terminal/application input and scrolling semantics win;
- exiting canonical TUI state returns to normal Pane transcript presentation.

### Current split-control rule

Primary split/layout controls live with the active Tab/layout chrome and target the focused Pane. Pane context menus/keyboard shortcuts may expose equivalent actions, but permanent duplicate split controls are not required in every Pane header.

## Configuration and performance

- light and dark appearances remain first-class;
- shell themes own prompt/ANSI terminal-content appearance;
- application chrome/layout/composer settings use typed configuration;
- future optional Lua/customization requires separate capability/security authority;
- configuration parsing, semantic enrichment, agents, persistence, and UI metadata must never synchronously block PTY → VT → damage → render/input paths.

If an old screenshot conflicts with the current frozen specification, **the current specification wins**. Do not create a parallel implementation path to preserve the old mockup.
