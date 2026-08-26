# M001 Agents View

**Status:** Frozen UI reference specification  
**Parent:** `M001-CORE-TERMINAL-REFERENCE-SCREEN.md`  
**Scope:** Cross-workspace agent inventory and agent-session inspection

## 1. Purpose

The Agents view is Seyal's complete inventory of running/waiting agent sessions across workspaces. The compact agent list in the Core Terminal left panel remains a current-workspace shortcut; this view is the broader management surface.

The view must remain **agent-native but not agent-dependent**. Opening it must not affect terminal execution, PTY progress, rendering, or shell input.

## 2. Primary user goals

Users should be able to:

- see Claude Code, Codex, OpenCode and other supported agent sessions together;
- understand which workspace/tab/pane each agent belongs to;
- see current task/activity and attention state;
- identify approvals waiting for user action;
- jump directly to the agent's owning surface;
- inspect one selected agent in detail.

## 3. Inventory fields

A dense table/list may show only backed-by-real-data fields:

- agent/provider/session identity;
- workspace;
- tab/pane location;
- current task/activity summary;
- status;
- pending approval count;
- last activity;
- quick jump target.

Useful status vocabulary:

- Running;
- Waiting;
- Attention;
- Idle/Completed where supported;
- Disconnected where lifecycle semantics require it.

Do not infer status by scraping terminal text when a provider/session adapter exposes authoritative state.

## 4. Scope and filters

Default dedicated view: `All Workspaces`.

Useful filters may include:

- provider;
- status;
- workspace;
- attention required.

The compact left-panel Agents section in Core Terminal defaults to current-workspace agents and must not become a duplicate of this full inventory.

## 5. Inspector

Selecting an agent switches the inspector to that agent only.

Recommended sections:

### Agent details
- provider;
- session identity;
- status;
- workspace;
- tab/pane location.

### Execution/activity
- current task/activity summary;
- started/runtime where available;
- pending approvals;
- error/failure state;
- provider-specific capability summary only when real.

### Context
- working directory;
- context-window usage if the provider exposes it reliably;
- files/resources in context where explicitly known;
- memory/session references where supported.

### Actions
Only actions supported by the owning adapter/session model may appear, for example:
- jump/open in pane;
- review pending approval;
- pause/resume if provider supports it;
- stop/terminate agent session.

Do not invent generic pause/stop semantics for providers that cannot support them correctly.

## 6. Approval behavior

Approvals are part of Seyal's global attention system.

An approval shown here must be the same underlying attention item that can appear in the notification popover. Resolving it in either place resolves the single authoritative approval state.

Do not duplicate approval authority in UI state.

## 7. Agent placement

Agents may run in terminal-backed or non-terminal agent surfaces depending on provider/integration, but an agent must not silently acquire ownership of a pane's PTY/VT/grid.

When an agent is terminal-backed, the pane's `TerminalExecution` remains the terminal authority.

## 8. Dedicated view is not a shell

The Agents inventory is a management surface. Merely opening it does **not** make any shell busy and does not require a pane composer.

If a composer is visually present because the overall app shell exposes one, it must remain scoped to an actual focused terminal pane and must not imply the Agents table itself owns a shell.

## 9. Performance

Agent inventory/state updates are asynchronous cold/warm-path metadata. They must never synchronously participate in PTY output, VT mutation, rendering, input, or projection delivery.

Large numbers of agent sessions must not create one polling thread/process per agent merely for UI status.

## 10. Functional-only rule

Do not add provider badges, token counters, context percentages, approval counts, task labels or actions unless a real adapter/data source exists.

Where provider capability differs, the UI should degrade by omission rather than show fake parity.
