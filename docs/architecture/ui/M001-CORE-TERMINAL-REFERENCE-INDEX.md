# M001 Core Terminal Reference Index

**Status:** Frozen design-reference index  
**Authority:** Companion to `M001-CORE-TERMINAL-REFERENCE-SCREEN.md`

This index maps the current Core Terminal design into implementation-oriented specifications. Only the frozen Core Terminal reference image is committed by the original design PR; additional generated state mockups are represented by specification where appropriate.

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
4. historical visual references only as non-binding inspiration.

Important superseded old details include:

- fixed/configurable maximum Block height for terminal output;
- internal Block output scrolling and parent-scroll chaining;
- active composer while the foreground shell is occupied;
- permanent per-Pane split-control clusters;
- duplicated full agent inventories;
- decorative utility controls.

## Mockup-vs-spec rule

Generated mockups are visual references. If a mockup contains decorative, stale, or architecturally invalid behavior, the corresponding specification is authoritative.
