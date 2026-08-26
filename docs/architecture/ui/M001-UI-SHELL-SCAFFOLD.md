# M001 UI Shell Scaffold Boundary

**Status:** Implementation boundary for the pre-Pass-6 native shell scaffold  
**Authority:** subordinate to `M001-CORE-TERMINAL-REFERENCE-SCREEN.md`, its companion UI specs, M001 pass ordering, and accepted Runtime/TerminalExecution ownership

## Purpose

Allow native macOS shell/component work to proceed without pretending that Pass 6 renderer or live Runtime integration is complete.

The scaffold may establish:

- shared native design tokens;
- the Core Terminal shell regions;
- a data-driven Block presentation container;
- one Pane-owned transcript scroll surface per preview Pane;
- a Pane-scoped multiline composer editor that stores preview drafts only;
- contextual left panel and inspector presentation seams;
- attention-popover presentation seam;
- a `TerminalSurfaceHostView` that contains the already-established permanent `MetalSurfaceView` boundary;
- deterministic preview fixtures behind an explicit preview-only launch path;
- ephemeral preview-only UI interaction state for Workspace/Tab/Pane selection, Pane focus, tab creation/close, split geometry, attention navigation, and Pane-local drafts.

## Hard boundary

This scaffold must not implement or invent:

- Pass 6 terminal rendering;
- live Runtime attachment;
- an alternate PTY, VT, grid, terminal model, or copied terminal authority;
- per-cell Swift/Rust callbacks;
- a temporary `NSTextView`/SwiftUI **terminal renderer**;
- shell integration or command intelligence;
- fake command execution or fake Block output;
- fabricated process/PID/CPU/memory/resource telemetry;
- agent approval mutations;
- production notification state;
- final input/keybinding behavior;
- command submission to a shell before Runtime/native-input wiring exists;
- multiple-pane Runtime semantics.

`NSTextView` is permitted for the Pane-local multiline **composer editor** only. It must never become a terminal or Block rendering surface.

The normal application path remains the existing minimal Metal surface until the M001 dependency frontier authorizes live UI integration.

## Preview interaction contract

The preview must not be a static picture made from labels. Controls that are visible in the preview must perform their UI-level behavior against isolated preview state when that behavior does not require Runtime authority.

Required preview interactions:

- selecting a top tab or left-panel tab changes the active Workspace-local Tab;
- `+` creates and selects a new preview Tab;
- close removes a preview Tab when another Tab remains;
- split-right and split-down modify the preview Tab-owned Pane tree and focus the new Pane;
- focusing another Pane updates the focused Pane and Inspector context;
- each Pane owns an independent composer draft that survives Tab/Pane navigation;
- Workspace selection swaps the Workspace-scoped tab inventory;
- selecting an attention item navigates to its preview target and consumes that preview attention item;
- unsupported Runtime/PTY actions are omitted or clearly unavailable rather than simulated.

This interactive state is disposable presentation state. It must never be reused as production Runtime/TerminalExecution state or become authoritative for PTY geometry, process lifecycle, terminal cells, or execution results.

## Frozen visual contract

The shell preview must follow the frozen Core Terminal visual direction together with the current functional specifications. The visual target is a dense, dark developer workspace designed for a 15-inch display, not a generic AppKit form using adaptive system colors.

At the canonical `1280x800` preview size:

- the workspace-scoped tab strip spans the full content width beneath native window chrome;
- a compact left context panel occupies approximately 236 points and shows Workspaces, current-Workspace Agents, and Tabs;
- the active Tab/Panes own the dominant center width;
- a contextual Inspector occupies approximately 292 points on the right;
- each terminal Pane contains compact Pane chrome, one Pane-owned transcript surface, and its own minimal composer at the bottom;
- the permanent Metal terminal host is visible in the Pane, but before Pass 6 no fabricated terminal output is drawn into it;
- the palette is low-contrast charcoal/navy with restrained purple focus and semantic status accents;
- typography is dense;
- decorative navigation, metrics, process data, Block output, or inspector modes from concept art are omitted until backed by accepted behavior/data.

Where visual concept art conflicts with the current Block sizing, busy-composer, TUI takeover, ownership, or functional-only rules, the current specifications win.

The XCTest layout contract and XCUIAutomation suite are part of this visual contract. XCUI launches the actual `Seyal.app` preview, asserts the visible hierarchy, exercises navigation/split/attention controls, and records a rendered screenshot in the test result bundle for design review. A true pixel-golden comparison must not be claimed until the approved source reference image is deliberately added to the repository as a test asset.

## Preview-only path

The UI shell may be launched explicitly for design/decomposition review using a debug-only preview path. Preview state:

- is deterministic at launch;
- may mutate locally in response to UI interactions;
- is never Runtime authority;
- must be clearly isolated from production state;
- must not be reused as terminal output/state storage;
- is discarded when the preview process exits.

Test-only fixtures that would otherwise look like production state, such as an attention item, must be opt-in through the UI-test environment rather than shown in the normal developer preview.

## Scroll ownership

Normal transcript mode has exactly one output scroll owner **per Pane**.

`BlockView` must not own an `NSScrollView` or another nested output-scroll surface. Blocks size intrinsically from their presented body. The Pane transcript may scroll and later virtualize off-screen content without changing this interaction model.

The composer editor may use its own bounded editing scroll behavior. That is input editing, not terminal/output scrolling, and must remain Pane-local.

## Block ownership

`BlockView` accepts a presentation model and a body view. It does not own terminal cells, VT state, PTY state, execution lifecycle, or a copied output transcript.

The future renderer can provide a terminal-surface body without changing Block ownership. The default pre-Pass-6 shell preview does not fabricate completed/running Blocks merely to fill the screen.

## TUI seam

The shell architecture must allow canonical TUI takeover to replace normal Block/transcript presentation with the same execution's terminal surface and hide/disable the Pane composer. The scaffold does not implement the canonical mode transition itself.

## Functional-only rule

Production UI must not show unsupported controls or fabricated metrics merely because they exist in a mockup. The preview follows the same rule: controls with meaningful local UI semantics may operate against isolated preview state; Runtime-dependent operations must not pretend to succeed.

## Acceptance

The scaffold is acceptable only if:

- default M001 app behavior is unchanged;
- the shell is native Swift/AppKit;
- Metal remains the permanent terminal-surface direction;
- no SwiftUI terminal implementation exists;
- no `NSTextView` terminal implementation exists;
- `NSTextView` usage is confined to the Pane composer editor;
- `BlockView` contains no nested output scroll view;
- each preview Pane owns one normal transcript scroll surface;
- top/left tab selection, new-tab, close-tab, split-right, split-down, Workspace selection, Pane focus, and attention navigation are real preview interactions;
- Pane composer drafts are independent preview state and are not presented as executed shell input;
- no fabricated terminal output or Runtime telemetry appears in the normal preview;
- deterministic shell construction participates in the native smoke test;
- the canonical `1280x800` preview satisfies the frozen three-column layout contract;
- XCUIAutomation launches the real preview and validates both hierarchy and interactive controls;
- a rendered screenshot is attached to the XCUI result for design review;
- all existing repository tests/checks remain green.
