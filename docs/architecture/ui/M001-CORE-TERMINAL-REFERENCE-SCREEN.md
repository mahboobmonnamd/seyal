# M001 Core Terminal Reference Screen

**Status:** Frozen reference for design/spec alignment  
**Scope:** Core terminal screen only  
**Reference image:** `docs/architecture/ui/assets/m001-core-terminal-reference.jpg`

## 1. Purpose

This document freezes the canonical M001 Core Terminal screen so design and implementation can proceed consistently.

This is a reference screen spec, not permission to implement out-of-order milestone work.

It defines the screen hierarchy, visible regions, functional meaning of each region, interaction rules, and constraints needed for future implementation.

## 2. Canonical object hierarchy

```text
App
└── Workspace
    └── Tab
        └── Pane tree
            └── Pane
                └── TerminalExecution
```

- **Workspace**: durable working context, typically bound to a repo/project/environment.
- **Tab**: task-oriented view within the active workspace.
- **Pane**: split region inside a tab.
- **TerminalExecution**: actual shell/process owner for a pane.

Rules:
- one app can contain many workspaces;
- one workspace can contain many tabs;
- one tab can contain one or more panes;
- each pane owns its own execution context and pane-scoped composer state.

## 3. Screen layout

The frozen screen has four primary regions:

```text
[Top tab row]
[Left contextual navigation] [Center active tab/pane] [Right inspector]
```

### 3.1 Top row
Contains active-workspace tabs, new-tab affordance, split/layout controls, and notifications.

Tabs are not placed inside the macOS title bar.

### 3.2 Left panel
Contains workspace inventory, active-workspace agents, and active-workspace tab inventory.

### 3.3 Center
Contains the focused pane with Block history/live output and a pane-scoped composer anchored at the bottom.

### 3.4 Right inspector
Contextual details for current workspace/tab/pane/process/selection. It complements rather than duplicates the left panel.

## 4. Top row

### 4.1 Workspace-scoped tabs
Shows tabs belonging to the active workspace.

A tab may show:
- title;
- compact activity/attention status.

Example titles:
- Core Terminal
- Agent Development
- Logs & Monitoring
- PR Review

### 4.2 New tab
`+` creates a new tab in the active workspace.

### 4.3 Split/layout controls
Minimum intended actions:
- split right;
- split down.

Additional layout controls may be added only when backed by real behavior. Do not add decorative layout icons.

### 4.4 Notifications
A notification/attention icon remains visible in the top-right area.

It exists to surface actionable attention without forcing tab switching, including future:
- agent approvals;
- command completion/failure;
- long-running execution attention;
- background task completion.

## 5. Left panel

The left panel is intentionally dense for terminal power users and must remain practical on a 15-inch display.

### 5.1 Workspaces
Shows known workspaces.

Each row may show:
- workspace name;
- root/project path;
- tab count;
- compact status where useful.

### 5.2 Agents
Shows agent sessions in the current workspace by default, with future ability to switch to an all-agents view.

Examples:
- Claude Code;
- Codex;
- OpenCode;
- Seyal Agent.

Each row may show:
- provider/session name;
- state such as Running, Waiting, Online, Attention.

This is the primary visible agent list on this screen. The inspector must not repeat a full duplicate agent list.

### 5.3 Tabs — active workspace
Shows tabs belonging to the active workspace as a persistent inventory/navigation aid.

This complements the top tab strip:
- top strip = immediate active tab switching;
- left list = persistent workspace-local inventory/status.

Each row may show:
- tab title;
- compact activity/attention marker.

## 6. Center pane and Blocks

### 6.1 Block model
Normal command execution is represented as a durable **Block**, not one undifferentiated scroll surface.

A standard Block contains:

#### Header
- command text;
- execution state;
- elapsed time;
- timestamp where useful;
- supported Block actions.

#### Body
- actual terminal output.

#### Optional structured summary
Command-aware summaries may appear only when backed by real product logic or integration.

Examples:
- Git file-change summary;
- additions/deletions;
- process/service state;
- Kubernetes/resource summary.

Fallback is always correct command + output. No plugin/integration is required for basic Block correctness.

### 6.2 Block actions
Visible actions in the reference have intended functional meaning:
- **Copy** — copy command/output according to target;
- **Rerun** — rerun in the appropriate execution context;
- **Pin** — retain/promote a Block for quick return/reference;
- **Expand** — focus/enlarge the Block for inspection.

Do not add actions purely for visual balance.

### 6.3 Pin semantics
Pinning must not duplicate terminal authority or copy the live grid. A pinned Block is a presentation/navigation state over existing execution/history identity.

Exact persistence/retention policy is deferred to its owning implementation spec, but the UI meaning is: “keep this Block easy to find and intentionally retained in the user’s workflow.”

## 7. Long-running commands and live output

The reference `npm run dev` example is a **long-running streaming command**, not a full-screen alternate-screen TUI.

While it runs:
- its Block remains live and continues receiving output;
- new output stays associated with that running Block;
- the underlying pane shell is busy;
- the user cannot execute an unrelated new shell command in that same shell until the running foreground process exits or is interrupted.

### 7.1 Visibility when more Blocks exist
Live output must remain discoverable even if later Blocks/history exist.

Expected behavior:
- while the process is foreground-active, its live Block is the active execution focus;
- the viewport may follow the live tail by default;
- if the user scrolls away, Seyal must not forcibly snap back on every update;
- provide a lightweight “return to live/tail” affordance/state when the user is away from the live end;
- after the command exits, the Block becomes normal completed history.

This avoids hiding live logs beneath newly added content.

### 7.2 Busy-shell guidance
When a pane’s foreground shell is occupied, Seyal may show compact guidance such as:
- current running command;
- `Ctrl+C` to stop;
- split pane or open another tab to run something concurrently.

This must remain informational and compact.

## 8. Composer

The composer lives **inside each pane** and is pane-scoped.

Therefore:
- a tab with four panes has four pane composers;
- each pane preserves its own composer state;
- only the focused pane composer is visually/interaction dominant;
- inactive pane composers may be visually subdued.

### 8.1 Minimal visible controls
The composer should remain compact and avoid unnecessary controls.

Core behavior:
- type shell command;
- history/fuzzy search trigger;
- agent invocation trigger;
- action trigger;
- submit/execute.

### 8.2 History fuzzy search
History search is a composer capability, not a permanently open panel.

When invoked, it opens contextually above the composer and supports fuzzy command-history retrieval.

### 8.3 Agent/action discovery
Agent and action suggestions also open contextually from the composer.

The helper surface is **not shown permanently by default**.

## 9. Raw terminal and TUI distinction

Seyal must distinguish three presentation cases over the same execution authority:

### 9.1 Normal Block mode
Command/output is grouped into Blocks.

### 9.2 Long-running streaming command
Foreground process produces continuously updating output in its active Block.

### 9.3 Full-screen TUI / alternate-screen takeover
Applications such as Vim/Neovim/htop/tmux-like TUIs take over the pane surface using the same `TerminalExecution` and canonical terminal state.

In TUI takeover:
- Block chrome and composer must not intercept terminal semantics;
- the pane behaves as a direct terminal surface;
- when the application exits alternate-screen mode, Seyal returns to the appropriate normal presentation.

## 10. Sessions, Agents, Resources bottom/global navigation

The frozen direction favors minimizing stacked permanent navigation on a 15-inch screen.

Where global category navigation is used, it should remain compact and avoid duplicating the content already visible in the left panel.

### 10.1 Sessions
A Sessions view should show runtime/session inventory rather than workspace structure.

Expected content:
- active attached terminal sessions;
- detached but surviving sessions;
- session host/runtime identity;
- workspace association;
- foreground process summary;
- last activity;
- reconnect/jump target.

### 10.2 Agents
A dedicated Agents view provides the complete cross-workspace agent inventory.

Expected content:
- provider/session;
- workspace/tab/pane association;
- running/waiting/attention state;
- current task/activity summary;
- jump-to-agent target;
- approvals/attention where applicable.

The left-panel Agents section is the compact current-workspace version of this broader inventory.

### 10.3 Resources
A Resources view provides operational/runtime resources and hosts, not decorative metrics.

Possible backed-by-real-data content:
- local/remote hosts;
- active Runtime instances;
- containers;
- Kubernetes contexts/resources when explicitly integrated;
- environment/runtime resources;
- CPU/memory/network/process summaries.

Do not expose a resource category until Seyal has a real data source/integration for it.

## 11. Multipane behavior

A tab may contain one or more panes in a nested split tree.

### 11.1 Pane rules
Each terminal pane owns:
- one `TerminalExecution`;
- one pane-local presentation state;
- one pane-scoped composer.

### 11.2 Focus
Only one pane is focused at a time.

Focus determines:
- active composer;
- keyboard target;
- inspector default context;
- which execution receives pane-scoped actions.

### 11.3 Left-panel pane tree
Do **not** show a full pane tree by default simply because a tab has multiple panes.

For power-user efficiency:
- keep workspace/tab inventory stable;
- indicate pane count/activity compactly;
- expose a pane-tree navigator only when useful or explicitly invoked.

This prevents the sidebar from becoming a deeply nested tree on small screens.

## 12. Inspector

The inspector is contextual.

### 12.1 Resolution priority

```text
explicit selection
→ focused object
→ active pane
→ active tab
→ active workspace
```

Examples:
- selected Block → Block details;
- selected process → process details;
- selected agent → agent details;
- otherwise focused pane/execution details;
- then tab/workspace context.

### 12.2 Stable sections for focused terminal pane
Useful data includes:

#### Workspace
- workspace name;
- path/repo context;
- branch where known;
- last activity.

#### Active tab
- tab identity;
- pane count/layout summary where useful.

#### Active pane
- pane identity;
- focused state;
- shell;
- cwd;
- execution duration/state.

#### Active process
Seyal should surface the foreground/running process created from the terminal.

Useful fields:
- command/process name;
- PID/PPID where available;
- running/exited state;
- started time;
- duration;
- CPU;
- memory;
- port when reliably detected;
- process-tree affordance.

This is a major value of the inspector and should be driven by actual OS/runtime information rather than shell-output scraping.

#### Resources
Only backed-by-real-data metrics:
- CPU;
- memory;
- disk/network where available and meaningful.

### 12.3 Inspector mode icons
The compact inspector mode icons from earlier approved visual exploration may be retained as the right-pane mode switcher if each maps to a defined functional inspector mode.

Potential modes include:
- info/context;
- process/runtime;
- artifacts/files/diff;
- activity/attention/history.

Do not add an icon until its corresponding view and data contract are documented.

### 12.4 No duplicate agent list
Do not show the same full agent inventory simultaneously in both left navigation and inspector.

When an agent is explicitly selected, the inspector may show **that agent’s details**.

## 13. Structured Block enrichments

Rich Block summaries are not guaranteed for every command.

Three levels are expected:

1. **Default** — command + terminal output, always available.
2. **Built-in recognizers/integrations** — selected high-value commands can produce structured summaries asynchronously.
3. **Future extension/plugin/adaptor** — additional command-specific presentations may be contributed without changing terminal authority.

Rules:
- terminal correctness never depends on enrichment;
- enrichment never blocks PTY/VT/render progress;
- unknown commands remain fully correct raw/Block output;
- no second VT/grid/output authority is created.

## 14. Functional-only UI rule

Every visible region, icon, button, metric, status, and panel in implementation must have:
- a real product behavior;
- a real backing state/data contract;
- or an explicitly accepted implementation issue/spec.

Forbidden:
- fake buttons for appearance;
- duplicate navigation purely for symmetry;
- placeholder metrics presented as production data;
- speculative inspector modules with no backing source;
- controls that cannot be implemented when the screen ships.

## 15. Frozen design intent

The frozen screen should communicate that Seyal is:
- workspace-first;
- terminal-correct;
- Block-native;
- pane/composer scoped;
- process-aware;
- agent-native without making agents terminal authority;
- information-dense but efficient on a 15-inch screen;
- contextual rather than cluttered;
- functional rather than decorative.

## 16. Authority boundary

This reference freezes the **visual/information architecture direction** for the Core Terminal screen.

It does not override:
- M001 dependency order;
- accepted Runtime/TerminalExecution/TerminalState ownership;
- Pass-specific implementation scope;
- future specs required for Sessions, Agents, Resources, multipane navigation, notifications, inspector modes, or rich Block integrations.

Implementation work must remain subordinate to the milestone/ADR/spec authority active at the time it is scheduled.
