# M001 Block Details Inspector

**Status:** Frozen UI reference specification  
**Parent:** `M001-CORE-TERMINAL-REFERENCE-SCREEN.md`  
**Scope:** Selected Block inspection and Block-specific actions

## 1. Purpose

When a user selects a Block, the right inspector should switch from generic pane/process context to that Block's details.

The inspector must reveal useful execution metadata without duplicating terminal output or creating another terminal model.

## 2. Selection behavior

Inspector context priority is:

```text
explicit Block selection
→ focused process/pane
→ active tab
→ active workspace
```

Selecting a Block highlights/focuses that Block and binds inspector content to its stable identity.

Deselecting returns the inspector to focused-pane context.

## 3. Core Block fields

Only real metadata should be shown:

- command;
- execution state;
- exit code when complete;
- duration;
- start/finish time;
- workspace;
- tab/pane/execution association;
- cwd;
- shell/runtime where known.

## 4. Structured summary

A summary section may contain command-aware information when a supported recognizer/integration produced it.

Examples:

- server URL/port;
- file-change summary;
- Kubernetes resource counts/status;
- test pass/fail summary;
- build diagnostics.

The summary is additive derived data. The Block remains correct without it.

Do not infer high-confidence structured actions from arbitrary terminal text alone.

## 5. Related artifacts

The inspector may link to related durable objects when they exist, such as:

- changed files;
- diff/artifact;
- logs;
- agent activity;
- resource object.

A related item must be backed by a stable product identity/data source, not a decorative filename chip.

## 6. Pin semantics

Pinning means intentionally keeping the Block easy to return to/reference.

Pin must not:

- clone terminal output into a second canonical store;
- keep a PTY alive solely because a presentation is pinned;
- alter terminal execution semantics.

Pin state is presentation/workspace metadata over existing Block/history identity.

Exact persistence and retention limits belong to the owning Block/history implementation spec.

## 7. Actions

Supported Block actions may include:

- Copy;
- Rerun;
- Pin / Unpin;
- Expand / Focus.

Destructive `Delete Block` should not be shown unless Seyal has a precise definition of what is being deleted.

If durable terminal/history data cannot safely be deleted independently, omit this action. UI must not suggest deletion semantics that architecture does not support.

## 8. Rerun

Rerun must create a new real execution attempt through the appropriate pane/shell command path. It does not mutate the historical Block into a new result.

The old Block remains historical evidence; the new command result receives its own Block identity.

## 9. Running Blocks

For a running Block, inspector fields update asynchronously as the process progresses.

Possible live fields:

- running state;
- duration;
- foreground PID/process;
- detected port/resource summary where reliably available.

The inspector must never require acknowledgement for terminal progress.

## 10. Performance and ownership

Block inspection is a cold presentation path.

Opening the inspector must not:

- reparse terminal history;
- copy the full visible grid;
- stall live output;
- synchronously invoke agents/plugins;
- change VT state.

## 11. Functional-only rule

Omit unsupported fields/actions. The inspector should be smaller and truthful rather than visually full but architecturally fake.
