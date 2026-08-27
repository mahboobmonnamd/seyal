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
- native AppKit menu/key-equivalent routing for presentation-only Workspace/Tab/window navigation;
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
- agent approval mutations;
- production notification state;
- terminal-input keybinding behavior that forwards or transforms PTY/TUI input;
- multiple-pane Runtime semantics.

The normal application path remains the existing minimal Metal surface until the M001 dependency frontier authorizes live UI integration.

## Frozen visual contract

The shell preview must follow the frozen Core Terminal visual direction together with the current functional specifications. The visual target is a dense, dark developer workspace designed for a 15-inch display, not a generic AppKit form using adaptive system colors.

At the canonical `1280x800` preview size:

- the workspace-scoped tab strip spans the full content width beneath native window chrome;
- a compact left context panel occupies approximately 236 points;
- the left panel uses a **Workspaces / Tabs** switcher instead of stacking both inventories permanently;
- **Workspaces** shows all known Workspaces plus agent sessions for the active Workspace;
- **Tabs** shows the active Workspace's tab inventory and new-tab action;
- the focused terminal Pane owns the dominant center width;
- a contextual Inspector occupies approximately 292 points on the right and is pinned to the content trailing edge with no unused strip beside it;
- the Inspector retains the frozen design's slim vertical tool rail on its far-right edge;
- the Inspector rail is functional rather than decorative: current preview modes filter existing context into **Context**, **Workspace**, **Tab**, and **Pane** views only;
- both the left context panel and right Inspector can be hidden and reopened; when either is hidden the center Pane reclaims that width rather than preserving an empty gutter;
- persistent top-chrome toggles reopen a hidden side panel, while each visible side panel also exposes its own collapse control;
- each Pane has compact, functional Pane-local split and close controls in its header;
- the Pane contains its own minimal composer at the bottom;
- the palette is low-contrast charcoal/navy with restrained purple focus and semantic status accents;
- typography is dense and terminal output remains monospaced;
- unsupported process/resource/history inspector modes from concept art remain omitted until backed by accepted behavior/data.

Where visual concept art conflicts with the current Block sizing, busy-composer, TUI takeover, ownership, or functional-only rules, the current specifications win.

The XCTest layout contract and XCUIAutomation suite are part of this visual contract. XCUI launches the actual `Seyal.app` preview, asserts the visible hierarchy and left-center-right ordering, exercises the Workspaces/Tabs switcher, Inspector rail, side-panel hide/reopen behavior, native navigation shortcuts, and Pane-local lifecycle controls, and records a rendered screenshot in the test result bundle for design review. A true pixel-golden comparison must not be claimed until the approved source reference image is deliberately added to the repository as a test asset.

## Preview-only path

The UI shell may be launched explicitly for design/decomposition review using a debug-only preview path. Preview fixtures:

- are deterministic;
- may model disposable Workspace/Tab/Pane/focus/draft interaction state;
- are never Runtime authority;
- must not fabricate PTY/process/terminal/resource telemetry;
- must not be reused as terminal output/state storage.

The normal preview therefore leaves the permanent Metal terminal host unattached rather than filling it with fake command output.

## Workspace and Tab navigation

The frozen compact navigation model is:

```text
left context
├── Workspaces
│   ├── Workspace inventory
│   └── active-Workspace agent sessions
└── Tabs
    └── active-Workspace tab inventory
```

The same Tab identity is used by the top strip and the left Tabs view. Switching Workspace replaces the Workspace-scoped tab inventory rather than showing tabs from multiple Workspaces together.

The left context panel is presentation chrome, not execution authority. Hiding it must not change Workspace, Tab, Pane, draft, focus, or execution identity. Reopening it restores the same navigation state.

## Native navigation shortcuts

macOS shell navigation uses native `NSMenuItem` key equivalents rather than `keyDown` interception. Shortcuts therefore remain discoverable in the menu bar, participate in normal AppKit command routing/accessibility, and do not consume plain Control/Option terminal input that a future PTY/TUI path may require.

The preview shortcut contract is:

| Scope | Previous / Next | Direct selection |
| --- | --- | --- |
| Workspace | `⌃⌘[` / `⌃⌘]` | `⌃⌘1` … `⌃⌘9` |
| Tab | `⇧⌘[` / `⇧⌘]` | `⌘1` … `⌘9` |
| Window | `⇧⌘\`` / `⌘\`` | `⌥⌘1` … `⌥⌘9` |

Additional native UI commands:

- `⌘T` creates a preview Tab;
- `⌘W` closes the active preview Tab when another Tab remains;
- `⌘0` toggles the left navigation sidebar;
- `⌥⌘0` toggles Inspector.

`⌘1…9` for Tabs and `⌥⌘1…9` for windows intentionally follow established macOS terminal conventions. Workspace shortcuts occupy a separate Control+Command layer so Workspace navigation never conflicts with Tab/window numbering.

Window switching is presentation-level AppKit behavior over existing visible titled windows. It does not create or own Runtime sessions. Keyboard navigation must route through the same shell state/actions as mouse navigation; it must not create a duplicate Workspace/Tab model or reconstruct the shell in a way that loses Pane drafts, focus, Inspector mode, or sidebar visibility.

## Inspector navigation

The frozen Inspector includes a narrow vertical mode rail on the far-right edge. The rail switches the detail surface immediately to its left.

For the pre-Pass-6 preview, only modes backed by existing deterministic UI context are allowed:

- **Context** — all currently available Inspector context;
- **Workspace** — Workspace rows only;
- **Tab** — active Tab rows only;
- **Pane** — focused Pane rows only.

No process, resource, metrics, history, or other Runtime-dependent Inspector item may appear merely to match concept-art icons. Those modes are introduced only after accepted data ownership and behavior exist.

Hiding the Inspector hides the detail surface and its mode rail together. Its top-chrome toggle remains available to reopen it, and the selected Inspector mode is retained across hide/reopen operations.

## Pane-local layout controls

The top layout controls continue to target the focused Pane. Every Pane also exposes compact local controls so multipane layouts remain understandable without relying on implicit focus:

- **Split** opens Split Right / Split Down for that Pane;
- **Close Pane** removes that Pane when the Tab has more than one Pane;
- the last Pane cannot be closed through the preview UI;
- closing a Pane collapses its parent split and preserves surviving Pane draft/focus state.

These controls manipulate preview layout metadata only. They do not create a PTY or claim Runtime resize/lifecycle authority.

## Scroll ownership

Normal transcript mode has one output scroll owner per Pane.

`BlockView` must not own an `NSScrollView` or another nested output-scroll surface. Blocks size intrinsically from their presented body. The Pane transcript may scroll and later virtualize off-screen content without changing this interaction model.

The Pane-local multiline composer may use its own bounded editor scroll surface as allowed by the composer specification; that editor is not terminal output scrolling.

## Block ownership

`BlockView` accepts a presentation model and a body view. It does not own terminal cells, VT state, PTY state, execution lifecycle, or a copied output transcript.

The future renderer can provide a terminal-surface body without changing Block ownership.

## TUI seam

The shell architecture must allow canonical TUI takeover to replace normal Block/transcript presentation with the same execution's terminal surface and hide/disable the Pane composer. The scaffold does not implement the canonical mode transition itself.

## Functional-only rule

Production UI must not show unsupported controls or fabricated metrics merely because they exist in a mockup. Preview interaction controls are acceptable only where they manipulate real preview UI state and are covered by tests. Runtime-dependent controls remain hidden until the Runtime contract exists.

## Acceptance

The scaffold is acceptable only if:

- default M001 app behavior is unchanged;
- the shell is native Swift/AppKit;
- Metal remains the permanent terminal-surface direction;
- no SwiftUI terminal implementation exists;
- `NSTextView` is used only for the Pane-local multiline composer and never as a terminal renderer;
- `BlockView` contains no nested output scroll view;
- one Pane transcript scroll surface owns normal output navigation per Pane;
- deterministic shell construction participates in the native smoke test;
- the canonical `1280x800` preview satisfies the frozen three-column layout contract with Inspector flush to the trailing edge;
- the Workspaces/Tabs switcher follows the frozen compact navigation model;
- native Workspace/Tab/window shortcuts use AppKit menu key equivalents and route through the same preview state/actions as pointer navigation;
- shortcut routing preserves existing shell presentation state such as sidebar visibility rather than replacing the shell with a second state owner;
- the Inspector exposes the frozen right-edge navigation rail using only functional modes backed by current preview context;
- left context and Inspector hide/reopen controls are functional and the center Pane reclaims hidden side-panel width;
- each Pane's split and close controls mutate the intended Pane's preview layout state;
- XCUIAutomation launches the real preview and validates those interactions;
- a rendered screenshot is attached to the XCUI result for design review;
- all existing repository tests/checks remain green.
