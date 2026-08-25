# M001 Full-Screen TUI Takeover

**Status:** Frozen UI reference specification  
**Parent:** `M001-CORE-TERMINAL-REFERENCE-SCREEN.md`  
**Scope:** Full-screen alternate-screen/raw terminal presentation inside a Pane

## 1. Purpose

TUI takeover defines how applications such as `htop`, Vim/Neovim, tmux and other full-screen terminal programs take control of the focused Pane without creating a second terminal engine or another PTY.

## 2. Authority

The same Pane keeps the same:

- `TerminalExecution`;
- PTY;
- child process lineage;
- canonical Seyal VT/`TerminalState`;
- input path.

TUI takeover is a presentation/state transition, not a new session.

## 3. Entering TUI mode

When canonical terminal state enters the supported alternate/full-screen mode, Seyal switches that Pane from Block/transcript presentation to direct terminal-surface presentation.

Rules:

- the TUI expands to the available Pane viewport;
- normal Block chrome is not drawn over the application surface;
- the Pane composer is hidden/disabled, not merely left active underneath;
- keyboard/mouse terminal input goes through the normal terminal input path;
- resize targets the same terminal execution;
- no output scraping is used to guess TUI mode.

## 4. Scrolling model

TUI takeover does **not** use the normal Pane transcript scrollbar to emulate application scrolling.

- the application/terminal mode owns its own scrolling/navigation semantics;
- Seyal does not wrap the TUI in a fixed-height Block;
- Seyal does not create an additional nested output scrollbar around the TUI.

When TUI mode exits, the Pane returns to the normal transcript/Block scroll model.

## 5. Input ownership

While takeover is active, the application owns terminal semantics.

Seyal must not intercept ordinary terminal keys for Block/composer features.

Global app shortcuts may remain available only where they are intentionally documented and do not violate raw terminal semantics.

## 6. Exit behavior

The normal way to leave a TUI is the application's own terminal interaction and canonical mode transition.

If UI presents an `Exit TUI` affordance, it must not fake an alternate-screen exit through client-only presentation state. It may only perform a documented user-equivalent action or explicit process interruption/termination.

Canonical terminal state remains authoritative for the actual return to Block/transcript presentation.

## 7. Composer restoration

On canonical TUI exit:

- restore normal Pane presentation;
- restore the Pane composer only when shell/process state permits new input;
- preserve any pane-local composer draft that existed before takeover where safe and meaningful.

## 8. Inspector

Inspector may continue showing non-invasive context such as:

- active TUI program;
- process ID;
- runtime;
- working directory;
- process/resource metrics.

Inspector must not consume terminal focus unless the user explicitly interacts with it.

## 9. Block/history relationship

Entering a TUI does not snapshot each frame into durable Blocks and does not create a second grid copy.

Before/after history follows canonical terminal semantics. Alternate-screen frames are not converted into a stream of persisted Block snapshots.

## 10. Multipane

A TUI can occupy one Pane while other panes independently show Blocks, long-running streaming commands, raw terminals, or other TUIs.

Focus determines keyboard target without altering the background TUI process.

## 11. Rendering/performance

TUI redraws may be high-frequency and damage-heavy.

Requirements:

- use canonical terminal damage;
- avoid full-terminal copies unless actual damage requires it;
- no synchronous Block/agent/persistence work;
- renderer/client backpressure must not block PTY → VT progress.

## 12. Functional-only rule

Any TUI labels, process data, or exit controls must correspond to real state/behavior. Do not add explanatory chrome that permanently reduces terminal area merely to make TUI mode visually obvious.
