# Approved UI visual references

These images are the user-approved visual references for Seyal UI work. They are **visual authority only**: the product constitution, accepted architecture/ADRs, specifications and milestone contracts remain higher authority. Before native implementation, agents must also follow `.agents/skills/image-to-code/SKILL.md` and applicable macOS/Metal/accessibility/testing skills.

## UI-REF-001 — Multiline command composer

![Multiline command composer](UI-REF-001-MULTILINE-COMPOSER.png)

Visual authority for the composer geometry and controls when composer mode is eligible: multiline auto-growing editor, prompt/input area, utility row, working-directory/shell controls and Run action.

Runtime correctness still wins: when an application takes terminal ownership (for example `nvim`), the **entire terminal pane becomes the live TUI** and the composer yields/retracts. Unsupported/ambiguous shell states, raw interaction, secrets/password prompts and interactive REPL states use direct terminal input rather than forcing the composer.

## UI-REF-002 — Light workspace composition

![Light workspace composition](UI-REF-002-APPROVED-LIGHT-WORKSPACE.png)

Broader workspace direction showing the approved relationships between larger workspace navigation, tabs, Block-based terminal panes, a composer inside each terminal pane, pane split controls, collapsible utility surfaces and the actionable attention popover.

Features outside the active M001 scope remain future work; this image does not authorize implementing them early.

## UI-REF-003 — Light component reference board

![Light component reference board](UI-REF-003-APPROVED-LIGHT-COMPONENT-BOARD.png)

Component-level visual reference for adaptive/scrollable tabs, workspace navigation, Command Palette entry, pane headers/split controls, adaptive execution Blocks, per-terminal-pane multiline composer, attention stack and utility pane.

## Approved clarifications

- There is **no common/global command composer**. Each terminal pane has its own composer presentation when composer eligibility is proven.
- Non-terminal surfaces such as inspector, activity and artifact views do **not** receive a composer by default.
- Normal Flow panes present command/output as **Blocks**, not raw unstructured text panes.
- Block visual style uses lightweight Block boundaries/surfaces with **no persistent execution rail/gutter** and no decorative left timestamp rail.
- Blocks grow from result content up to the configured maximum height, then scroll internally.
- Block actions include clean **Copy command**, **Copy output**, **Copy both**, in-block search and navigation/jump to persisted Blocks. Clipboard output must not include UI borders/box-line artifacts.
- The Attention Stack is a **global structured attention model**. Agent approval/question is one scenario, not the entire model. Other typed items may include failures, completions, disconnects, security/policy decisions and review-ready results.
- Seyal should expose a typed API/protocol so shells/tools designed for Seyal can publish supported structured attention items instead of relying on scraped terminal text.
- Tabs are comfortably sized, then progressively compress; once minimum width is reached the tab strip scrolls.
- Workspace rows are larger and multi-line; global navigation/search belongs in the Command Palette rather than a dedicated workspace-search field.
- Split controls belong to each terminal pane.
- Light and dark appearances are both first-class.
- Shell/zsh/bash/fish theme configuration owns prompt/ANSI/terminal-content appearance.
- Seyal application chrome, Blocks, composer, layout and appearance are configured through typed **TOML** settings, with future optional **Lua** customization through a separately approved capability/security design.
- TOML/Lua parsing or callbacks must never enter PTY → VT → damage → render/input hot paths.
- Visual references do not authorize fake/POC UI, duplicate terminal state, a temporary renderer, a second VT/grid, or parallel old/new production paths.

If a textual design artifact conflicts with these approved visual details, reconcile the owning artifact **before implementation** rather than guessing or creating another implementation path.
