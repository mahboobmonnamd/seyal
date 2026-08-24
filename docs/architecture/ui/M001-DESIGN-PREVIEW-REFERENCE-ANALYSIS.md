# M001 Design Preview Reference Analysis

**Status:** Design-review reference inventory  
**Scope:** Preview-only branch `design/ui-review-v1`  
**Authority:** Approved UI reference assets under [`references/`](references/)

## 1. Source reference inventory

| ID | File | Dimensions | Appearance | Role |
| --- | --- | --- | --- | --- |
| UI-REF-001 | [UI-REF-001-MULTILINE-COMPOSER.png](references/UI-REF-001-MULTILINE-COMPOSER.png) | 1024×768 | Dark | Annotated multiline composer reference board |
| UI-REF-002 | [UI-REF-002-APPROVED-LIGHT-WORKSPACE.png](references/UI-REF-002-APPROVED-LIGHT-WORKSPACE.png) | 1228×768 | Light | Annotated workspace composition reference |
| UI-REF-003 | [UI-REF-003-APPROVED-LIGHT-COMPONENT-BOARD.png](references/UI-REF-003-APPROVED-LIGHT-COMPONENT-BOARD.png) | 1228×768 | Light | Annotated component inventory / detail board |

## 2. Why PNG copies exist in this branch

The earlier worktree only had WebP documentation copies because the approved references were checked in as documentation-optimized assets. For the native design-review harness, the PNG attachments are now copied into [`references/`](references/) and treated as the exact review baseline for this branch.

The PNG copies are preview-only assets for this design-review branch. They do **not** change production architecture, terminal ownership, or the public runtime/UI contracts.

## 3. Visual inventory

### UI-REF-001 — multiline composer board

- top-level dark presentation board with title, explanatory subtitle, and small product wordmark;
- primary multiline composer centered horizontally, with:
  - left prompt chip;
  - large multi-line text entry area;
  - utility controls row along the bottom edge;
  - strong primary Run action on the trailing edge;
- inset mini-window preview on the right;
- bottom key-state strip showing default, focused, with-command, and running variants;
- behavior strip describing expanded vs retracted live-TUI behavior;
- keyboard shortcut legend.

### UI-REF-002 — light workspace composition

- native macOS-like window with traffic lights and centered title;
- large workspace navigator on the left with descriptive rows and grouped sections;
- adaptive tabs across the top of the central workspace;
- two-column terminal pane grid;
- each terminal pane contains:
  - pane header with split/menu controls;
  - adaptive execution block(s);
  - pane-local multiline composer docked at the bottom;
- global agent-attention popover stack near the upper-right portion of the central area;
- utility sidebar on the right for activity, files, and agents;
- bottom legend / principles strip in the reference image only.

### UI-REF-003 — light component board

- reference-board format with numbered component sections;
- tabs, workspace navigator, pane header, block, composer, popover stack, utility pane;
- mini all-pane workspace composition at the bottom;
- implementation notes panel.

## 4. Behavior and ownership mapping

- The reference boards define **visual authority**, not runtime behavior.
- Pane-local composers remain conditional presentations over the same `ExecutionId`.
- Agent attention remains global and actionable, not pane-owned.
- Utility sidebar surfaces remain non-terminal by default.
- TUI takeover must retract composer chrome rather than nesting a TUI inside a Block.

## 5. Unknowns and non-inferable details

- exact fonts beyond the visible Apple-style sans and monospace mix;
- explicit backing scale used when the references were exported;
- exact macOS version and control metrics used by the mockup author;
- hidden hover/focus/pressed states not shown in the still images;
- final production interaction behavior for elements that remain future milestone scope.

These unknowns must not be invented and later claimed as reference fidelity.

## 6. Preview harness decision

This branch’s design-review harness now uses the approved PNG boards directly inside the native preview target so visual review uses the exact approved source rather than a guessed redraw.

That is intentional because the prior preview invented a substantially different layout. Exact asset presentation is the safest preview-only correction while the future production-intended AppKit component pass is decomposed into smaller issues.
