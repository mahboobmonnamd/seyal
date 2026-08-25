# M001 Full-Screen TUI Takeover

**Status:** Frozen UI reference specification  
**Parent:** `M001-CORE-TERMINAL-REFERENCE-SCREEN.md`  
**Scope:** Full-screen alternate-screen/raw terminal presentation inside a pane

## 1. Purpose

TUI takeover defines how applications such as `htop`, Vim/Neovim, tmux and other full-screen terminal programs take control of the focused pane without creating a second terminal engine or another PTY.

## 2. Authority

The same pane keeps the same:

- `TerminalExecution`;
- PTY;
- child process lineage;
- canonical Seyal VT/`TerminalState`;
- input path.

TUI takeover is a presentation/state transition, not a new session.

## 3. Entering TUI mode

When canonical terminal state indicates the supported full-screen/alternate-screen mode, Seyal switches that pane from Block/composer presentation to direct terminal-surface presentation.

Rules:

- Block chrome is not drawn over the application surface;
- pane composer is hidden/disabled for that pane;
- keyboard and mouse terminal input go directly through the normal terminal input path;
- resize targets the same terminal execution;
- no output scraping is used to detect TUI mode.

## 4. Input ownership

While takeover is active, the application owns terminal semantics.

Seyal must not intercept ordinary terminal keys for Block/composer features.

Global app shortcuts may remain available only where they do not violate raw terminal semantics and are intentionally documented.

## 5. Exit behavior

The normal way to leave a TUI is the application's own terminal interaction and canonical mode transition.

If UI presents an `Exit TUI` affordance, it must **not fake an alternate-screen exit** by mutating client presentation state. It may only:

- send a documented user-equivalent signal/key sequence with clear semantics; or
- terminate/interrupt the foreground process after explicit user intent.

Canonical VT state remains authoritative for when the pane actually returns to Block/raw presentation.

## 6. Inspector

The inspector can continue to show non-invasive context for the focused pane, such as:

- active TUI program;
- process ID;
- start/runtime;
- working directory;
- process/resource metrics.

Inspector must not consume keyboard focus or alter terminal behavior unless the user explicitly interacts with it.

## 7. Block history relationship

Entering a full-screen TUI does not convert its live alternate-screen contents into a durable Block grid copy.

Before/after execution history may remain in logical terminal history according to terminal semantics, but Seyal must not snapshot every TUI frame into Blocks.

## 8. Multipane

A TUI can occupy one pane while other panes continue running Blocks, streaming commands or other TUIs independently.

The focused pane receives keyboard input; switching focus changes the input target without altering the TUI process.

## 9. Rendering/performance

TUI redraws may be high-frequency and damage-heavy.

Requirements:

- use canonical terminal damage;
- no per-frame full-terminal copies unless damage truly requires it;
- no synchronous Block/agent/persistence work;
- renderer/client backpressure must not block PTY → VT progress.

## 10. Functional-only rule

Any TUI labels, process data or exit controls must correspond to real state/behavior. Do not add explanatory chrome that permanently reduces usable terminal area merely to make the mode visually obvious.
