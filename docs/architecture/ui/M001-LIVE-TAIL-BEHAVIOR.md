# M001 Live Tail Behavior

**Status:** Frozen UI reference specification  
**Parent:** `M001-CORE-TERMINAL-REFERENCE-SCREEN.md`  
**Scope:** Long-running streaming Block viewport behavior

## 1. Purpose

Live-tail behavior keeps a long-running command's newest output easy to follow without trapping users at the bottom of the stream.

The canonical example is a foreground command such as `npm run dev` that continuously emits logs in one live Block.

## 2. Running Block model

While the process is active:

- output remains part of the same running Block;
- the Block has a visible live/running state;
- new terminal output appends through the normal PTY → VT → canonical state path;
- the shell in that pane remains occupied by the foreground process.

No second log process, duplicate PTY, or copied terminal engine is created to provide live tail.

## 3. Default follow behavior

When the user is already at the live end of the Block, new output keeps the viewport following the tail.

This should feel like a normal terminal following current output.

## 4. User scroll-away behavior

If the user scrolls upward to inspect older output:

- stop automatic viewport movement;
- continue receiving and storing/displaying new output in the running Block;
- clearly indicate that the user is viewing older output;
- do not forcibly snap back to the bottom for every incoming update.

The terminal execution continues unaffected.

## 5. Return-to-live affordance

When the user is away from the live end, show one compact functional affordance such as:

- `Jump to live`;
- `Return to live tail`.

It should appear only while useful and disappear once the live end is restored.

Do not render duplicate top and bottom controls merely for visual balance unless usability testing proves both are required.

## 6. New-output indication

While scrolled away, Seyal may show a compact count/status of unseen new output where that state can be tracked cheaply and correctly.

Examples:

- `Live` indicator;
- `new output` marker;
- bounded unseen-line/update count.

The indicator must not require per-line semantic processing.

## 7. Relationship to new Blocks

A foreground long-running process owns the shell, so unrelated shell commands cannot create later Blocks in the same pane until the process exits/is interrupted.

Therefore live logs cannot simply be "pushed above" by arbitrary new same-shell command Blocks while the foreground process remains active.

Parallel work should happen in another pane/tab/execution.

If future non-shell/agent/activity Blocks can appear in the same presentation, they must not obscure the live execution focus or change terminal authority.

## 8. Completion

When the foreground process exits:

- the Block transitions from Running to Completed/Failed according to exit state;
- live-tail state is cleared;
- the Block becomes ordinary navigable history;
- the pane shell/composer becomes available for the next command where shell lifecycle permits.

## 9. Pin interaction

Pinning a running Block keeps it easy to find but does not freeze output or disable live-tail behavior.

Pinned state is presentation/workspace metadata and must not duplicate the live terminal contents.

## 10. Multipane behavior

Each pane manages its own live-tail presentation state.

Scrolling away in one pane must not pause or alter another pane's live output.

Focus changes do not stop a background pane from receiving terminal output.

## 11. Performance

Live-tail UI state must remain extremely cheap.

Requirements:

- no full-history relayout on every update;
- no synchronous semantic parsing of every line;
- no renderer acknowledgement needed for PTY/VT progress;
- bounded viewport/history work;
- damage-driven redraw where possible.

## 12. Functional-only rule

Only show `Live`, `paused view`, unseen-output counts, or jump controls when the corresponding state is real. The UI should not manufacture progress indicators merely to make streaming output look richer.
