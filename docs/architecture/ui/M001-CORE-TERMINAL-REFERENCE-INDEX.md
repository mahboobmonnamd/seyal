# M001 Core Terminal Reference Index

**Status:** Frozen design-reference index  
**Authority:** Companion to `M001-CORE-TERMINAL-REFERENCE-SCREEN.md`

This index maps the current Core Terminal design into implementation-oriented specifications. Additional generated state mockups are represented by specification where appropriate.

## Exact frozen visual source

The visual explicitly selected and frozen during M001 design review is **`Seyal Developer Workspace Dashboard.png`**.

Do not substitute an older neon/core-experience/generated concept merely because it is visually similar. The approved image itself is not added by this shell PR. When the approved asset is supplied manually to the repository, it must occupy the reference path declared by `M001-CORE-TERMINAL-REFERENCE-SCREEN.md` and become the source for pixel-level visual regression.

Until that asset exists in the repository, the shell must be checked against the visual composition recorded in `M001-UI-SHELL-SCAFFOLD.md`, the XCTest geometry contract, the XCUI hierarchy/ordering assertions, and the rendered screenshot retained in the XCUI result bundle. Do not claim pixel-golden equivalence without the approved source asset.

## Reference specifications

| Mockup/state | Specification | Purpose |
|---|---|---|
| Core Terminal | `M001-CORE-TERMINAL-REFERENCE-SCREEN.md` | Canonical shell, Workspaces/Agents/Tabs, Blocks, composer, inspector, placement rules |
| Earlier first-UI reconciliation | `M001-FIRST-UI-DESIGN-AMENDMENT.md` | Explicitly supersedes stale fixed-height/nested-scroll and old placement details |
| Sessions | `M001-SESSIONS-VIEW.md` | Attached/detached session inventory, reconnect and lifecycle inspection |
| Agents | `M001-AGENTS-VIEW.md` | Cross-workspace agent inventory, status, approvals and jump targets |
| Resources | `M001-RESOURCES-VIEW.md` | Hosts/process/resource inventory driven by real integrations |
| Multipane | `M001-MULTIPANE-VIEW.md` | Tab-owned split tree, focus, one composer state per terminal Pane, Pane-level scrolling |
| Notifications | `M001-NOTIFICATIONS-ATTENTION-POPOVER.md` | Global attention stack and inline approval/action handling |
| Full-screen TUI | `M001-TUI-TAKEOVER.md` | Same-PTY/VT full-Pane takeover with composer hidden/disabled |
| Block details | `M001-BLOCK-DETAILS-INSPECTOR.md` | Selected Block metadata, enrichments and actions |
| Composer history | `M001-COMPOSER-HISTORY-FUZZY-SEARCH.md` | Multiline Pane composer and contextual fuzzy history retrieval |
| Live tail | `M001-LIVE-TAIL-BEHAVIOR.md` | Long-running output with growing Block + Pane-level follow/scroll-away/return-to-live |
| Pre-Pass-6 shell scaffold | `M001-UI-SHELL-SCAFFOLD.md` | Native shell decomposition boundary that preserves M001 pass ordering and terminal ownership |

## Cross-screen invariants

1. `TerminalExecution` remains terminal execution authority; UI never owns a second PTY/VT/grid.
2. Workspace → Tab → Pane is the product/navigation hierarchy; a Tab owns its split tree.
3. Every available terminal Pane owns independent multiline composer state; there is no shared/global composer.
4. **Normal Block/transcript output has one scroll owner: the Pane.** Blocks grow intrinsically; long output does not create nested Block output scrollbars.
5. A foreground-busy Pane retracts/disables its composer; a full-screen TUI hides/disables it and occupies the Pane viewport.
6. Inspector content is contextual: explicit selection → focused object → active Pane → active Tab → active Workspace.
7. Agents/resource/Block enrichments are additive and asynchronous; terminal progress never waits for them.
8. Full-screen TUI/raw presentation uses the same terminal execution and canonical terminal state.
9. Structured Blocks are presentation/history over real execution; enrichments never replace terminal correctness.
10. Every visible control/status/metric/action requires a real backing behavior/data contract. No decorative fake UI.
11. Tabs remain one row: compress to a documented minimum, then scroll/overflow horizontally.
12. Split actions target the focused Pane; primary persistent controls live with Tab/layout chrome rather than being repeated in every Pane for decoration.
13. Global command palette is keyboard-first; a permanent palette button is optional.
14. Runtime/connection status is surfaced when meaningful, especially remote/detached/reconnecting/degraded states.
15. Design is optimized for power users on a 15-inch display: dense, contextual, minimal permanent chrome.
16. These documents freeze design/information architecture only and do not authorize Pass 6+ implementation ahead of M001 ordering.
17. Before Pass 6, any native shell preview is fixture-only and must not become an alternate live terminal/runtime path.

## Old-reference precedence

The images under `references/` are historical design inputs. Their useful non-conflicting capabilities are incorporated into the current specs.

If an old screenshot or `M001-FIRST-UI-DESIGN.md` conflicts with current behavior, follow:

1. accepted architecture / ADR / implementation spec / milestone authority;
2. `M001-CORE-TERMINAL-REFERENCE-SCREEN.md` and companion state specs;
3. `M001-FIRST-UI-DESIGN-AMENDMENT.md` for explicit older-document reconciliation;
4. the frozen `Seyal Developer Workspace Dashboard.png` for visual composition, density, palette and hierarchy where it does not conflict with 1–3;
5. historical visual references only as non-binding inspiration.

Important superseded old details include:

- fixed/configurable maximum Block height for terminal output;
- internal Block output scrolling and parent-scroll chaining;
- active composer while the foreground shell is occupied;
- permanent per-Pane split-control clusters;
- duplicated full agent inventories;
- decorative utility controls.

## Mockup-vs-spec rule

The frozen mockup is the visual reference. If it or an older generated mockup contains decorative, stale, or architecturally invalid behavior, the corresponding current specification is authoritative. The visual reference drives composition and presentation; it does not override terminal correctness or invent product behavior.
