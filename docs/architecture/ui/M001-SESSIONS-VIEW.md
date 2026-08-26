# M001 Sessions View

**Status:** Frozen UI reference specification  
**Parent:** `M001-CORE-TERMINAL-REFERENCE-SCREEN.md`  
**Scope:** Sessions inventory, reconnect, and session inspection only

## 1. Purpose

The Sessions view answers one question quickly: **what terminal executions/sessions exist, where are they, and can I jump/reconnect to them?**

It is not a workspace browser and must not duplicate the Workspaces screen.

## 2. Primary user goals

A power user must be able to:

- see active attached sessions;
- see detached but still-alive sessions;
- identify workspace/tab/pane ownership;
- see host and foreground-process context;
- jump to an attached session;
- reconnect to a detached session;
- deliberately terminate a session when appropriate.

## 3. Main layout

The existing Seyal shell remains stable:

```text
[top workspace-scoped tab row]
[left navigation/context] [sessions inventory] [contextual inspector]
```

Do not introduce a second unrelated navigation system for this view.

## 4. Session inventory

The center surface is a dense table/list optimized for a 15-inch display.

Recommended columns:

- session identity/name;
- workspace;
- tab / pane;
- host/runtime;
- last activity;
- foreground process;
- state;
- row actions.

Useful states:

- `Running` — execution has a live foreground process;
- `Idle` — live shell/execution with no active foreground command;
- `Detached` — execution survives without a currently attached GUI view;
- `Attention` — execution needs user attention;
- `Exited` — execution is no longer live and is shown only where retained history makes that useful.

The exact status vocabulary must be backed by Runtime/lifecycle state rather than inferred from terminal text.

## 5. Attached vs detached

The reference distinguishes attached and detached sessions.

### Attached

An attached session already has an active presentation/controller/observer relationship. Selecting it should jump/focus its existing workspace/tab/pane rather than create another terminal authority.

### Detached

A detached session remains alive in Runtime with its PTY/child/`TerminalExecution` intact. Reconnect attaches a new client presentation to the existing execution.

Reconnect must never replay terminal bytes into a new VT engine or create a replacement PTY.

## 6. Search and filters

Search/filter affordances are functional and may cover:

- name;
- workspace;
- host;
- process;
- state.

Useful compact filters include:

- All;
- Attached;
- Detached;
- Attention.

Do not add filters without a real queryable field.

## 7. Inspector behavior

Selecting a session changes the right inspector to session context.

Recommended sections:

### Context
- workspace;
- tab;
- pane.

### Runtime
- shell;
- execution/session ID where useful;
- PID / process group where available;
- foreground process;
- lifecycle state;
- last activity;
- duration.

### Host
- host identity;
- OS/platform where known;
- connection/runtime location.

### Actions
For a live detached session:
- reconnect;
- open/jump in a tab where supported.

Destructive termination must be clearly separated from ordinary navigation/reconnect actions.

## 8. Termination semantics

`Terminate Session` is not equivalent to closing the GUI or detaching.

Termination means deliberately ending the `TerminalExecution`/child lifecycle according to Runtime policy.

If another controller/attachment makes termination unsafe, Runtime policy decides whether the request is rejected or requires confirmation. UI must not bypass lifecycle authority.

## 9. Performance and data-source rules

The list must be driven by Runtime inventory/state, not terminal scraping.

Refreshing the Sessions view must not enter PTY → VT → damage hot paths or create per-session polling threads.

Idle hidden sessions must remain cheap.

## 10. Functional-only rule

Every visible field/action must have a defined data source and behavior.

Do not add decorative:

- host metadata;
- process labels;
- reconnect buttons;
- health badges;
- session counts.

If Seyal cannot obtain a field reliably, omit it until the owning capability exists.
