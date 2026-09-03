# Seyal Reference Screen Contracts

**Status:** Proposed visual/reference authority  
**Parent:** `SEYAL-UNIVERSAL-COMPONENT-CONTRACT.md`  
**Historical functional inputs:** `docs/architecture/ui/references/1-9`  
**Theme requirement:** every new reference is produced in dark and light using identical geometry/components

## 1. Rule of construction

The nine historical reference images define product coverage, not the new visual language.

Every replacement reference must:

1. preserve the useful functionality represented by its historical counterpart;
2. obey current M001 behavioral/architecture specifications when historical behavior is stale;
3. use only components C01-C17 from `SEYAL-UNIVERSAL-COMPONENT-CONTRACT.md` unless a new component is specified first;
4. keep shared components visually identical across all nine references;
5. apply Zero-Chrome Adaptive Depth + Semantic Seams + Focus Gravity;
6. show dark and light variants without structural redesign.

## 2. Canonical shared geometry

Unless the screen is a full-screen management view or TUI takeover, use this stable frame:

```text
C01 UI Container
├── C02 Global Utility Rail
├── C03 Left Context Panel
├── center work surface
│   ├── C05 Top Tab Strip when Workspace/Tab scoped
│   └── Pane tree / management data surface
└── C10 Inspector when contextual inspection is useful
```

Terminal panes use C06 + C07 + C08 + C09. Multipane adds C13. Attention overlays use C11. Search uses C12. TUI uses C14. Management views use C15.

## 3. Reference 01 — Core Terminal / full terminal view

Historical source: `references/1-full view terminal.png`

Purpose: canonical daily Seyal terminal workspace.

Required components:

- C01 UI Container
- C02 Global Utility Rail
- C03 Left Context Panel
- C04 Context Rows for Workspaces, current sessions/agents where useful
- C05 Top Tab Strip
- C06 one focused Terminal Pane
- C07 multiple Semantic Blocks showing at minimum successful, normal and running/streaming command states
- C08 Block seams
- C09 fixed Pane Composer
- C10 right Inspector with Context/Files or equivalent backed modes
- C16 status indicators

Required visual behavior:

- terminal transcript is dominant continuous truth surface;
- Blocks are visible through headers/seams/spacing, never card stacks;
- Inspector is receded at rest;
- focused composer has modest D2 material;
- left panel is thin and visually secondary;
- no permanent large toolbar.

Suggested content coverage: shell command, test command, git status, Kubernetes/Docker-style command, agent activity in context.

Image target: `adaptive-depth/01-core-terminal-dark-light.png`.

## 4. Reference 02 — Sessions dashboard / reconnect

Historical source: `references/2-session dashboard.png`
Current behavior authority: `M001-SESSIONS-VIEW.md`

Purpose: inventory attached/detached terminal executions and reconnect safely.

Required components:

- C01, C02
- C03 with Sessions navigation/filter context where needed
- C04 Session rows
- C15 Management Data Surface for dense session inventory
- C10 Inspector or details region using Inspector grammar for selected session
- C16 state indicators for Running/Idle/Detached/Attention/Exited where supported
- C17 actions for Reconnect/Open/Terminate only when semantically valid

Rules:

- do not render every session as a card;
- attached/detached distinction must be instantly scanable;
- reconnect is prominent only for selected detached session;
- termination is separated from ordinary navigation;
- no terminal composer unless an actual terminal Pane is focused.

Image target: `adaptive-depth/02-sessions-dark-light.png`.

## 5. Reference 03 — Agents dashboard / orchestration

Historical source: `references/3-agent dashboard.png`
Current behavior authority: `M001-AGENTS-VIEW.md`

Purpose: cross-workspace agent inventory and detailed selected-agent context.

Required components:

- C01, C02
- C03 optional agent/workspace filters
- C04 Agent rows using the exact same Context Row anatomy
- C15 dense agent inventory
- C10 selected-agent detail/Inspector grammar
- C16 Running/Waiting/Attention/Idle/Disconnected state
- C17 only backed actions such as Jump, Review approval, Pause/Resume/Stop where provider supports them
- C11 may appear only when demonstrating a global approval event

Rules:

- provider branding is subordinate to state/context;
- no token/context percentage unless authoritative data exists;
- agent inventory must not look like a kanban/SaaS card board;
- selected agent gains Focus Gravity; others recede without losing legibility.

Image target: `adaptive-depth/03-agents-dark-light.png`.

## 6. Reference 04 — Resources / metrics / operations

Historical source: `references/4-metrics dashboard.png`
Current behavior authority: `M001-RESOURCES-VIEW.md`

Purpose: inspect real hosts/processes/containers/Kubernetes/runtime resources without manufacturing dashboard telemetry.

Required components:

- C01, C02
- C03 source navigation for Local/Remote/Docker/Kubernetes/etc. only where real
- C15 management data surface
- C10 selected-resource Inspector/detail surface
- C16 resource health/state
- compact charts/metrics only for real data

Rules:

- avoid a generic observability card dashboard;
- prefer dense tables, inline sparklines and semantic seams;
- charts never dominate the terminal/product identity;
- resource view must remain visually related to Sessions/Agents through the same rows, type hierarchy and Inspector treatment;
- no hidden PTY/composer dependency.

Image target: `adaptive-depth/04-resources-dark-light.png`.

## 7. Reference 05 — Multipane working view

Historical source: `references/5-multi pane view.png`
Current behavior authority: `M001-MULTIPANE-VIEW.md`

Purpose: demonstrate a real Tab-owned split tree with independent terminal executions and composers.

Required components:

- C01, C02, C03, C05
- at least three C06 Terminal Panes in one nested split tree
- C13 Split Dividers
- C07/C08 Blocks in normal transcript panes
- C09 one composer per available terminal Pane, using identical component geometry
- at least one busy Pane where C09 retracts/disables appropriately
- C10 Inspector follows focused Pane
- optional C14 TUI in one Pane while other panes remain normal

Rules:

- no Pane cards;
- only focused Pane gets strong Focus Gravity;
- split seams are nearly invisible at rest;
- inactive composer is the same component in receded state, not a different design;
- panes may show shell Blocks, logs, tests, TUI simultaneously without visual inconsistency.

Image target: `adaptive-depth/05-multipane-dark-light.png`.

## 8. Reference 06 — Notifications / attention

Historical source: `references/6-notifications.png`
Current behavior authority: `M001-NOTIFICATIONS-ATTENTION-POPOVER.md`

Purpose: show actionable background events without moving the user away from current work.

Required components:

- same underlying Core Terminal frame as Reference 01
- C11 Attention Stack anchored to the stable attention entry point
- three example item types: agent approval, command/process failure, completed background work/reconnect
- C17 action buttons using shared action geometry
- C16 severity/state indicators

Rules:

- Attention Stack is overlay, not permanent fourth column;
- only unresolved actionable item receives D3 prominence;
- notification previews minimize secrets/raw output;
- resolving an item should visually return it to quiet state;
- Core Terminal underneath must not be redesigned to accommodate the popover.

Image target: `adaptive-depth/06-attention-dark-light.png`.

## 9. Reference 07 — Full-screen TUI takeover

Historical source: `references/7-TUI.png`
Current behavior authority: `M001-TUI-TAKEOVER.md`

Purpose: show Vim/Neovim/htop/tmux/ncurses taking over the same Pane correctly.

Required components:

- same outer C01/C02/C03/C05 frame where appropriate
- one C06 Terminal Pane transformed into C14 TUI Takeover Surface
- C10 Inspector may remain receded with process/runtime context
- C09 composer must be absent from the takeover Pane
- C07/C08 Block chrome absent inside TUI viewport

Rules:

- TUI is visually absolute truth surface;
- do not add a heavy `TUI MODE` card/banner;
- no fake exit control that suggests client-side alternate-screen ownership;
- if another Pane exists, it may retain normal Blocks/composer independently.

Image target: `adaptive-depth/07-tui-takeover-dark-light.png`.

## 10. Reference 08 — TUI interaction within multipane/history context

Historical source: `references/8-TUI within block.png`
Current authority: `M001-TUI-TAKEOVER.md`, `M001-MULTIPANE-VIEW.md`, Core Terminal Block rules.

Historical wording/image must not override the current rule that a full-screen alternate-screen TUI is not a growing Block with nested scrolling.

Purpose of replacement reference: demonstrate the coexistence/transition relationship rather than preserving the stale visual implication.

Required components:

- one C06 Pane showing prior normal C07 Blocks/history before/around the TUI transition where semantically meaningful;
- C14 TUI owning the active Pane viewport during takeover;
- no C09 composer while takeover active;
- optional sibling terminal Pane retaining C07/C09;
- C10 Inspector showing active TUI/process context.

Rules:

- never draw a rounded Block frame around a live full-screen TUI;
- no nested TUI output scrollbar;
- the image should clearly demonstrate that Block presentation yields to canonical TUI presentation.

Image target: `adaptive-depth/08-tui-transition-dark-light.png`.

## 11. Reference 09 — Search / command surface

Historical source: `references/9-search.png`
Current behavior authority: `M001-COMPOSER-HISTORY-FUZZY-SEARCH.md` plus Core Terminal global command-palette rule.

Purpose: show fast keyboard-first discovery across application actions and terminal history without turning search into a dashboard.

Required components:

- same Core Terminal frame beneath overlay/helper
- C12 Search / Command Surface
- consistent row anatomy based on C04 density/typography
- scope modes clearly separated: global application search vs pane-local History/Agents/Actions helper
- C09 composer remains unchanged underneath when pane-local helper is used
- keyboard-selection state uses Focus Gravity rather than card elevation

Rules:

- one dominant input field;
- dense result rows;
- no result cards;
- global search may span Workspaces/Tabs/Panes/Sessions/Agents/Blocks/Files/Commands where implemented;
- pane-local history inserts into the focused C09 composer rather than becoming a different composer UI.

Image target: `adaptive-depth/09-search-dark-light.png`.

## 12. Shared component consistency matrix

| Component | 01 Core | 02 Sessions | 03 Agents | 04 Resources | 05 Multipane | 06 Attention | 07 TUI | 08 TUI transition | 09 Search |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| C01 UI Container | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| C02 Utility Rail | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| C03 Left Context | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | optional | ✓ | ✓ |
| C04 Context Row | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | optional | ✓ | ✓ |
| C05 Tab Strip | ✓ | optional | optional | optional | ✓ | ✓ | ✓ | ✓ | ✓ |
| C06 Terminal Pane | ✓ | optional | optional | optional | ✓ | ✓ | ✓ | ✓ | ✓ |
| C07 Semantic Block | ✓ | — | — | — | ✓ | ✓ | hidden in TUI | transition only | ✓ beneath overlay |
| C08 Semantic Seam | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | outer relationships only | ✓ | ✓ |
| C09 Composer | ✓ | only with focused terminal | only with focused terminal | only with focused terminal | per available pane | unchanged underneath | hidden in TUI pane | hidden in active TUI pane | unchanged |
| C10 Inspector | ✓ | ✓ | ✓ | ✓ | ✓ | underlying screen | optional/receded | ✓ | underlying screen |
| C11 Attention Stack | optional | optional | optional | optional | optional | ✓ | optional | optional | optional |
| C12 Search Surface | optional | optional | optional | optional | optional | optional | optional | optional | ✓ |
| C13 Split Divider | — | — | — | — | ✓ | as underlying layout | optional | optional | underlying layout |
| C14 TUI Takeover | — | — | — | — | optional | — | ✓ | ✓ | — |
| C15 Management Surface | — | ✓ | ✓ | ✓ | — | — | — | — | — |
| C16 Status Indicator | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ where real | ✓ | ✓ |
| C17 Action Button | contextual | ✓ | contextual | contextual | contextual | ✓ | rare | rare | contextual |

## 13. Image generation consistency rules

When producing the visual references:

- build Reference 01 first as the canonical component source;
- derive References 02-09 from the same visual kit;
- do not change composer height/radius/padding/icons between images;
- do not change sidebar row geometry between Workspaces/Sessions/Agents merely because content differs;
- do not change Inspector width/type hierarchy between Core and Multipane;
- do not introduce new card styles in Sessions/Agents/Resources;
- keep top tab treatment identical anywhere C05 appears;
- use the same semantic seam thickness/state language in every image;
- light/dark pair uses identical coordinates/geometry wherever content is the same;
- if a new component is required, specify it in `SEYAL-UNIVERSAL-COMPONENT-CONTRACT.md` before generating it.

## 14. Visual distinctiveness acceptance

The new set is rejected if removing the Seyal wordmark makes it indistinguishable from a conventional terminal/IDE dashboard.

The set should be recognizable through:

- continuous terminal/work canvas;
- semantic seams rather than boxes;
- receded utility material at rest;
- focus gravity around the active work object;
- one stable composer language;
- context/attention emerging only when operationally relevant;
- consistent cross-surface density across terminal, sessions, agents and resources.
