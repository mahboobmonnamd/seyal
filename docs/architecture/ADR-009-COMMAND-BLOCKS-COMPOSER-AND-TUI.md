# ADR-009 — Command Blocks, Pane Composer, and TUI Takeover

- **Status:** Proposed architecture change
- **Date:** 2026-08-28
- **Scope:** Post-Pass-7 command/Block presentation
- **Supersedes for this behavior:** the Pass 8 minimal-only boundary in `SPEC-007`
- **Depends on:** ADR-004, ADR-005, ADR-006, ADR-007, ADR-008, SPEC-001, SPEC-003, SPEC-004, SPEC-005, SPEC-006

## Decision requested

Seyal's normal macOS presentation is Flow/Blocks. Each accepted command submitted
through the Pane's unique composer creates one logical command Block containing
that command's output and lifecycle metadata. The composer belongs to the Pane,
not to an individual Block and not to the application globally.

This is a logical projection over the same authoritative `ExecutionId`, PTY,
VT state and terminal history. A Block never owns a PTY, terminal grid, copied
transcript, renderer or child process.

Full-screen applications are the only raw-surface exception. When canonical
terminal state enters alternate-screen/full-screen mode, the focused Pane
temporarily yields Block chrome and composer to the same terminal surface. On
canonical exit, the Pane returns to its Block transcript.

## Problem and conflict

The current PR 707 implementation exposes one `connect_first_running()` surface
inside one coarse Block. The composer either does nothing or writes directly to
that raw shell. That cannot provide one Block per command and makes the default
experience look like a raw terminal.

The accepted M001 Pass 8 `SPEC-007` deliberately excludes trusted shell
integration, per-command boundaries and command submission. The requested
product behavior therefore changes Block semantics and requires this ADR before
substantial implementation.

## Alternatives considered

### A. Keep one coarse Block and write composer text to the shell

Rejected. It is the current defect: command boundaries are absent, output cannot
be assigned to Blocks, and the composer becomes an unsafe raw-shell proxy.

### B. Create one PTY or terminal grid per Block

Rejected. This creates competing execution ownership, breaks shell/TUI lifecycle
semantics and violates the one-authoritative-terminal-state rule.

### C. Infer Blocks by scraping prompts or output in AppKit

Rejected. Prompt/output heuristics are untrusted, shell-specific and can expose
secrets or misclassify interactive programs. They also move terminal semantics
into the GUI.

### D. Trusted shell integration with logical history anchors

Selected for implementation. A Runtime-owned integration emits bounded command
boundary metadata associated with the same `ExecutionId` and canonical primary
history `LineId`s. The GUI consumes read-only Block metadata and terminal display
projection separately.

## Normative invariants

1. One Pane owns exactly one composer state and one focused execution route.
2. Each accepted composer command maps to exactly one logical command Block.
3. Command Block identity, state and anchors are Runtime/Workspace metadata.
4. The terminal authority remains one `TerminalState` per `ExecutionId`.
5. Blocks contain no PTY, VT parser, terminal grid, copied output or renderer.
6. Command-boundary observation is bounded and asynchronous; PTY → VT → damage
   never waits for Block mutation, persistence, rendering or GUI acknowledgement.
7. Composer input is enabled only while trusted integration proves supported
   line-oriented command-entry state and no active TUI/secret/raw interaction.
8. Unsupported or uncertain integration falls back to direct terminal input; it
   must not manufacture command Blocks from guesses.
9. A running command Block grows in the Pane transcript. The Pane is the only
   normal transcript scroll owner.
10. Alternate-screen/TUI state suppresses the composer and Block chrome without
    creating another execution or Block stream.
11. Completion is based on Runtime lifecycle/final-drain truth, never a GUI
    timeout or display heuristic.
12. The Block projection contains metadata and logical anchors, not terminal
    content. Terminal pixels remain rendered by Metal from canonical projection.

## Required implementation seams

- trusted shell integration capability and lifecycle events;
- Runtime/Workspace `BlockTimeline` command records;
- protocol messages for command start/end metadata with capability negotiation;
- disposable client Block cache keyed by `ExecutionId` and `BlockId`;
- Pane transcript that lays out N Blocks over logical history ranges;
- composer eligibility/focus state and explicit execute action;
- TUI takeover transition driven by canonical alternate-screen state;
- failure/quarantine path that returns to raw direct-terminal behavior;
- native UI, accessibility, conformance, security and performance evidence.

## Reopen conditions

Reopen this decision if trusted shell integration cannot preserve shell semantics,
if command boundaries require scraping, if Block metadata must carry copied
terminal output, or if performance/security evidence shows that the projection
blocks terminal progress.

This ADR is not accepted until an independent architecture/security review
approves the selected boundary and the affected specification is updated.
