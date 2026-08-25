# M001 Core Terminal Reference Index

**Status:** Frozen design-reference index  
**Authority:** Companion to `M001-CORE-TERMINAL-REFERENCE-SCREEN.md`

This index maps the frozen Core Terminal design exploration into implementation-oriented specifications. Only the original Core Terminal reference image is committed in this PR; additional mockup images are intentionally not added here.

## Reference specifications

| Mockup/state | Specification | Purpose |
|---|---|---|
| Core Terminal | `M001-CORE-TERMINAL-REFERENCE-SCREEN.md` | Canonical screen shell, Workspaces/Agents/Tabs, Blocks, composer, inspector |
| Sessions | `M001-SESSIONS-VIEW.md` | Attached/detached session inventory, reconnect and lifecycle inspection |
| Agents | `M001-AGENTS-VIEW.md` | Cross-workspace agent inventory, status, approvals and jump targets |
| Resources | `M001-RESOURCES-VIEW.md` | Hosts/process/resource inventory driven by real integrations |
| Multipane | `M001-MULTIPANE-VIEW.md` | Tab-owned split tree, focus and one composer per terminal pane |
| Notifications | `M001-NOTIFICATIONS-ATTENTION-POPOVER.md` | Global attention stack and inline approval/action handling |
| Full-screen TUI | `M001-TUI-TAKEOVER.md` | Same-PTY/VT alternate-screen takeover with direct terminal semantics |
| Block details | `M001-BLOCK-DETAILS-INSPECTOR.md` | Selected Block metadata, enrichments and actions |
| Composer history | `M001-COMPOSER-HISTORY-FUZZY-SEARCH.md` | Contextual fuzzy command-history retrieval from the pane composer |
| Live tail | `M001-LIVE-TAIL-BEHAVIOR.md` | Long-running streaming output follow/scroll-away/return-to-live behavior |

## Cross-screen invariants

All screens preserve these rules:

1. `TerminalExecution` remains the terminal execution authority; UI never owns a second PTY/VT/grid.
2. Workspace → Tab → Pane is the product/navigation hierarchy; a tab owns its split tree.
3. A terminal pane has its own pane-scoped composer; multipane means one composer per terminal pane.
4. Inspector content is contextual: explicit selection → focused object → active pane → active tab → active workspace.
5. Agents and resource metadata are additive and asynchronous; terminal progress never waits for them.
6. Full-screen TUI/raw presentation uses the same terminal execution and canonical terminal state.
7. Blocks are presentation/history objects over real terminal execution; rich summaries never replace terminal correctness.
8. Every visible control, status, metric and action requires a real backing behavior/data contract. No decorative fake UI.
9. Design is optimized for power users on a 15-inch display: dense, contextual, minimal permanent chrome.
10. These documents freeze design/information architecture only. They do not authorize Pass 6+ implementation ahead of M001 dependency ordering.

## Mockup-vs-spec rule

Generated mockups are visual references. If a generated mockup contains a decorative or architecturally invalid element, the corresponding specification is authoritative.

Examples already clarified by the specs:

- opening the Resources view does not consume/busy a shell;
- opening the Agents inventory does not itself consume/busy a shell;
- TUI exit cannot be faked by client-only state mutation;
- unsupported inspector fields/actions are omitted rather than simulated;
- live-tail UI should use one useful return-to-live affordance rather than duplicate controls for visual symmetry.
