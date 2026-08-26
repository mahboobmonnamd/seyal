# M001 UI Shell Scaffold Boundary

**Status:** Implementation boundary for the pre-Pass-6 native shell scaffold  
**Authority:** subordinate to `M001-CORE-TERMINAL-REFERENCE-SCREEN.md`, its companion UI specs, M001 pass ordering, and accepted Runtime/TerminalExecution ownership

## Purpose

Allow native macOS shell/component work to proceed without pretending that Pass 6 renderer or live Runtime integration is complete.

The scaffold may establish:

- shared native design tokens;
- the Core Terminal shell regions;
- a data-driven Block presentation container;
- one Pane-owned transcript scroll surface;
- a Pane-scoped composer presentation seam;
- contextual left panel and inspector presentation seams;
- attention-popover presentation seam;
- a `TerminalSurfaceHostView` that contains the already-established permanent `MetalSurfaceView` boundary;
- deterministic preview fixtures behind an explicit preview-only launch path.

## Hard boundary

This scaffold must not implement or invent:

- Pass 6 terminal rendering;
- live Runtime attachment;
- an alternate PTY, VT, grid, terminal model, or copied terminal authority;
- per-cell Swift/Rust callbacks;
- a temporary `NSTextView`/SwiftUI terminal renderer;
- shell integration or command intelligence;
- fake split/layout actions;
- agent approval mutations;
- production notification state;
- final input/keybinding behavior;
- final multiline editor implementation;
- multiple-pane Runtime semantics.

The normal application path remains the existing minimal Metal surface until the M001 dependency frontier authorizes live UI integration.

## Frozen visual contract

The shell preview must follow the frozen Core Terminal visual direction together with the current functional specifications. The visual target is a dense, dark developer workspace designed for a 15-inch display, not a generic AppKit form using adaptive system colors.

At the canonical `1280x800` preview size:

- the workspace-scoped tab strip spans the full content width beneath native window chrome;
- a compact left context panel occupies approximately 236 points and shows Workspaces, current-Workspace Agents, and Tabs;
- the focused terminal Pane owns the dominant center width;
- a contextual Inspector occupies approximately 292 points on the right;
- the Pane contains a compact context header, intrinsically sized Blocks inside one Pane-owned transcript scroll surface, and its own minimal composer at the bottom;
- completed Blocks visibly read as command-plus-output units and expose the accepted Block actions in the preview fixture;
- the palette is low-contrast charcoal/navy with restrained purple focus and semantic green/orange/red status accents;
- typography is dense and terminal output remains monospaced;
- decorative navigation, metrics, split controls, or inspector modes from concept art are omitted until backed by accepted behavior/data.

Where visual concept art conflicts with the current Block sizing, busy-composer, TUI takeover, ownership, or functional-only rules, the current specifications win.

The XCTest layout contract and XCUIAutomation suite are part of this visual contract. XCUI launches the actual `Seyal.app` preview, asserts the visible hierarchy and left-center-right ordering, and records a rendered screenshot in the test result bundle for design review. A true pixel-golden comparison must not be claimed until the approved source reference image is deliberately added to the repository as a test asset.

## Preview-only path

The UI shell may be launched explicitly for design/decomposition review using a debug-only preview path. Preview fixtures:

- are deterministic;
- are read-only presentation data;
- are never Runtime authority;
- must be clearly isolated from production state;
- must not be reused as terminal output/state storage.

## Scroll ownership

Normal transcript mode has exactly one output scroll owner: the Pane.

`BlockView` must not own an `NSScrollView` or another nested output-scroll surface. Blocks size intrinsically from their presented body. The Pane transcript may scroll and later virtualize off-screen content without changing this interaction model.

The composer editing surface is a separate future native-input concern and may eventually have bounded editor scrolling as allowed by the composer specification.

## Block ownership

`BlockView` accepts a presentation model and a body view. It does not own terminal cells, VT state, PTY state, execution lifecycle, or a copied output transcript.

The future renderer can provide a terminal-surface body without changing Block ownership.

## TUI seam

The shell architecture must allow canonical TUI takeover to replace normal Block/transcript presentation with the same execution's terminal surface and hide/disable the Pane composer. The scaffold does not implement the canonical mode transition itself.

## Functional-only rule

Production UI must not show unsupported controls or fabricated metrics merely because they exist in a mockup. Preview-only fixtures may demonstrate structure, but production wiring must hide or omit capabilities until backed by real state and behavior.

## Acceptance

The scaffold is acceptable only if:

- default M001 app behavior is unchanged;
- the shell is native Swift/AppKit;
- Metal remains the permanent terminal-surface direction;
- no SwiftUI terminal implementation exists;
- no `NSTextView` terminal implementation exists;
- `BlockView` contains no nested output scroll view;
- one Pane transcript scroll surface owns normal Block navigation;
- deterministic shell construction participates in the native smoke test;
- the canonical `1280x800` preview satisfies the frozen three-column layout contract;
- XCUIAutomation launches the real preview and validates the visible Core Terminal hierarchy;
- a rendered screenshot is attached to the XCUI result for design review;
- all existing repository tests/checks remain green.
