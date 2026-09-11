# ADR-009 — Command Blocks, Pane Composer, and Presentation Takeover

- **Status:** Accepted 2026-08-28; proposed presentation-mode amendment under #858, accepted only on merge
- **Date:** 2026-08-28; proposed amendment 2026-09-11
- **Scope:** Post-Pass-7 command/Block presentation and Flow/Raw/TUI mode ownership
- **Supersedes for this behavior:** the Pass 8 minimal-only boundary in `SPEC-007`; historical M001 presentation wording in SPEC-006/SPEC-009 and M001 UI design documents only where it assumes a permanently visible/focusable terminal surface while Flow is active
- **Depends on:** ADR-004, ADR-005, ADR-006, ADR-007, ADR-008, SPEC-001, SPEC-003, SPEC-004, SPEC-005, SPEC-006

## Decision

Seyal's normal structured-shell presentation is **Flow/Blocks**. Each accepted
command submitted through the Pane's unique composer creates one logical command
Block containing that command's output range and lifecycle metadata. The
composer belongs to the Pane, not to an individual Block and not to the
application globally.

This is a logical projection over the same authoritative `ExecutionId`, PTY,
VT state and terminal history. A Block never owns a PTY, terminal grid, copied
transcript, renderer or child process.

A `TerminalExecution` has one canonical terminal authority but may have different
**mutually exclusive user-visible presentations**:

```text
TerminalExecution
  ├─ PTY / child
  ├─ authoritative TerminalState
  │    ├─ primary grid + canonical retained history
  │    └─ alternate grid / terminal modes
  └─ derived presentation
       ├─ Flow → native Block transcript + Pane composer
       ├─ Raw  → full-Pane primary terminal grid
       └─ TUI  → full-Pane alternate/full-screen terminal grid
```

The identity/state continuity is the `ExecutionId` + PTY + `TerminalState`.
It does **not** require a permanently visible/focusable terminal `NSView`,
`CAMetalLayer`, or conventional terminal viewport underneath Flow.

## 2026-09-11 proposed correction — authority is not viewport

The original wording correctly rejected PTY/grid/renderer-per-Block designs, but
it did not state strongly enough that Flow, Raw and TUI must not be presented at
the same time. The current macOS implementation consequently reused the Pane's
interactive Metal terminal surface as a permanent full-transcript backing/input
surface while also placing Block chrome over it.

That interpretation is rejected by this proposed amendment.

### Flow

Flow is the primary user-visible presentation when trusted integration proves
that structured command entry is safe.

In Flow:

- terminal output is visible only inside the appropriate Block output regions;
- the current/running command's live tail is rendered inside its running Block,
  not as an independent full-Pane primary-grid viewport;
- the Pane composer owns supported structured command entry;
- empty transcript/canvas space does not expose a terminal cursor, terminal
  background, or click-through raw-terminal input surface;
- character-level terminal interaction that cannot be represented safely by the
  structured Flow contract causes a transition to Raw rather than being routed
  through a hidden/coexisting terminal viewport.

A Pane-owned Metal compositor/renderer **may** be reused to draw many visible
Block regions for efficiency. In Flow that renderer is a presentation
implementation detail only: it is not a conventional terminal viewport and is
not terminal-input authority. This does not create a renderer per Block.

### Raw

Raw is a full-Pane replacement presentation over the same primary terminal
state. Use it when trusted structured entry is absent/uncertain, a child requires
character-level terminal semantics, the user explicitly selects Raw, or a
failure/quarantine path cannot safely preserve Flow.

Raw never appears beside or underneath Flow. Entering Raw yields/hides the Flow
transcript/composer interaction surface; leaving Raw re-evaluates current
eligibility before Flow resumes.

### TUI

When canonical terminal state enters alternate-screen/full-screen mode, TUI is
a full-Pane takeover of the same execution. Flow/Raw chrome and the Pane
composer yield. Keyboard, mouse, focus, cursor and resize semantics belong to
the terminal application. On canonical exit, presentation is re-evaluated and
returns to Flow or Raw without recreating the execution.

TUI continuity requires the same `ExecutionId`/PTY/VT state; it does not require
that one AppKit view object remain permanently installed underneath every mode.
Renderer resources may be safely reused/reconfigured when that is the best
implementation, provided authority and latency invariants are preserved.

## Presentation transition and input fencing

Mode exclusivity is not only visual. A transition must change presentation and
input ownership atomically from the user's point of view.

Every `Flow ↔ Raw`, `Flow ↔ TUI`, and `Raw ↔ TUI` transition follows this order:

```text
freeze new source-mode input admission
→ invalidate source-mode route/presentation epoch
→ cancel/discard source-mode marked/preedit state without PTY submission
→ revoke source first-responder/text-input context and mouse route/capture
→ reject or ignore stale source-mode native callbacks that were not already admitted
→ validate destination eligibility against current Runtime authority
→ install destination presentation/input route
→ acquire destination first responder/IME/mouse semantics
→ resume destination input admission
```

No destination route becomes eligible while the source route can still admit
input. One native event may be admitted to at most one presentation route.
Anything already atomically admitted before the fence retains normal FIFO
semantics; unadmitted stale callbacks are rejected rather than replayed.

Eligibility and admission for Flow/composer or direct-terminal input must be
bound to the exact current authority tuple, conceptually:

```text
ExecutionId
+ AttachmentId / Controller authority
+ presentation/input epoch
+ relevant canonical TerminalState generation/mode state
+ trusted shell-integration generation/state
```

The concrete protocol representation may differ, but it must provide equivalent
fencing. A reconnect, controller change, execution replacement, canonical mode
change, integration generation change, or presentation transition makes stale
eligibility/admission evidence unusable. Stale or uncertain evidence never
widens authority and never causes one event to reach the previous route.

Flow command admission must likewise be correlated to the current eligible
execution/attachment/presentation generation. A delayed result from an older
presentation epoch cannot authorize or clear state in a newer epoch.

## Problem and conflict

The pre-ADR implementation exposed one `connect_first_running()` surface inside
one coarse Block. The composer either did nothing or wrote directly to that raw
shell. That could not provide one Block per command and made the default
experience look like a raw terminal.

The later production shell corrected command identity/lifecycle but introduced a
different presentation defect: the Pane-wide `InteractiveMetalSurfaceView`
remained the live full-frame renderer and input target while Block bodies were
laid out over that same surface. Hit testing deliberately fell through empty
Flow regions to the terminal surface. The result was effectively:

```text
Flow chrome / Block geometry
        over
permanent Raw terminal viewport + input surface
```

That is not the selected architecture. The correct model is one terminal
authority feeding one active presentation mode and one active input route at a
time.

## Alternatives considered

### A. Keep one coarse Block and write composer text to the shell

Rejected. Command boundaries are absent, output cannot be assigned reliably to
Blocks, and the composer becomes an unsafe raw-shell proxy.

### B. Create one PTY, terminal grid, or independent renderer per Block

Rejected. PTY/grid ownership would compete with the execution and break shell/TUI
lifecycle semantics. Independent renderer-per-Block also scales GPU/display
resources with history rather than visible Pane demand.

### C. Infer Blocks by scraping prompts or output in AppKit

Rejected. Prompt/output heuristics are untrusted, shell-specific and can expose
secrets or misclassify interactive programs. They also move terminal semantics
into the GUI.

### D. Permanent Pane-wide interactive terminal viewport with Blocks layered over it

Rejected by this proposed amendment. It conflates canonical terminal authority
with visible presentation, permits Raw interaction to leak through Flow, and
makes Blocks decorative chrome rather than the primary execution presentation.

### E. Trusted shell integration + logical history anchors + explicit presentation modes

Selected. Runtime-owned integration emits bounded command-boundary metadata
associated with the same `ExecutionId` and canonical primary-history `LineId`s.
The GUI consumes read-only Block metadata and terminal display projection while
an explicit Flow/Raw/TUI state determines how those projections are presented
and where input is routed.

## Normative invariants

1. One Pane owns exactly one composer state and one focused execution route.
2. Each accepted composer command maps to exactly one logical command Block.
3. Command Block identity, state and anchors are Runtime/Workspace metadata.
4. The terminal authority remains one `TerminalState` per `ExecutionId`.
5. Blocks contain no PTY, VT parser, terminal grid, copied output, child process
   or independent terminal renderer authority.
6. Command-boundary observation is bounded and asynchronous; PTY → VT → damage
   never waits for Block mutation, persistence, rendering or GUI acknowledgement.
7. Composer input is enabled only while trusted integration proves supported
   structured command-entry state and no active secret/raw/interactive/TUI state.
8. Flow, Raw and TUI are mutually exclusive user-visible presentations of the
   same execution. No mode transition recreates the PTY/VT/ExecutionId.
9. In Flow, terminal pixels are clipped/composed into Block output regions;
   there is no independent full-Pane primary-grid viewport or raw cursor behind
   the transcript.
10. In Flow, empty/noninteractive Block space must not route arbitrary keyboard,
    IME or mouse input to a hidden/coexisting terminal surface.
11. If character-level terminal semantics are required and not safely modeled by
    Flow, presentation transitions to full-Pane Raw before that input is routed.
12. A running command Block receives a bounded derived live-output projection
    anchored to that Block. The full current grid is not used as a visual
    shortcut behind the transcript.
13. The Pane remains the only normal Flow transcript scroll owner.
14. Alternate-screen/TUI state suppresses Flow/Raw chrome and composer and owns
    the full Pane until canonical exit.
15. Completion is based on Runtime lifecycle/final-drain truth, never a GUI
    timeout or display heuristic.
16. Block projections contain metadata and logical anchors, not terminal content
    authority. Metal/GPU state remains disposable derived presentation.
17. A Pane may reuse one Metal compositor across visible Flow regions and
    Raw/TUI takeover, but that reuse does not grant the compositor PTY/VT
    authority and must not expose multiple presentation modes simultaneously.
18. A presentation transition revokes the old first responder, IME/preedit,
    mouse route and input-admission route before enabling the new route.
19. Eligibility/admission is fenced to current execution, attachment/controller,
    presentation epoch and relevant canonical/integration generation; stale
    evidence fails closed.
20. One native input event is admitted through at most one presentation route.

## Relationship to SPEC-006 and SPEC-009

SPEC-006 remains authoritative for direct-terminal native event classification,
IME composition semantics, bounded input queues, Runtime-owned key encoding and
resize transactions. Under this amendment, wording that assigns those duties to
a permanent terminal surface is scoped to the **active Raw/TUI/direct-terminal
presentation endpoint**. It does not authorize a focusable terminal surface
under Flow.

SPEC-009 remains authoritative for Runtime/PTY survival, fresh AttachmentId and
Controller reacquisition, state reconstruction and reconnect fencing. Under this
amendment, reconnect restores interaction to the **newly selected current
presentation owner** after validating fresh execution/attachment/canonical state:
Flow composer/Blocks when trusted Flow eligibility is current, Raw otherwise,
or TUI when canonical full-screen/alternate state requires it. Reconnect does
not recreate or focus a raw terminal target underneath Flow.

These scoping rules supersede only conflicting presentation-target wording; they
do not weaken the accepted protocol, security, latency, resize, IME or reconnect
correctness contracts.

## Required implementation seams

- trusted shell integration capability and lifecycle events;
- Runtime/Workspace `BlockTimeline` command records;
- protocol messages for command start/end metadata with capability negotiation;
- bounded projection for completed Block history ranges and the running Block's
  current live tail;
- disposable client Block cache keyed by `ExecutionId` and `BlockId`;
- explicit Pane presentation state: `Flow | Raw | TUI`;
- presentation/input epoch or equivalent stale-callback fence;
- Flow compositor that clips terminal-derived pixels to Block output regions;
- composer eligibility/focus state and explicit execute action;
- Raw full-Pane input/render path without coexisting Block interaction;
- TUI takeover transition driven by canonical alternate/full-screen state;
- mode-aware accessibility and focus identities;
- failure/quarantine path that transitions to Raw rather than exposing a hidden
  terminal input surface through Flow;
- native UI, accessibility, conformance, security and performance evidence.

## Migration impact for the current macOS shell

The terminal engine is not being replaced. The reusable foundation includes PTY
ownership, VT/`TerminalState`, history, Block lifecycle metadata, bridge,
renderer/glyph/damage infrastructure and detach/reconnect identity.

The presentation layer must be corrected so:

- `PaneTranscriptView` no longer treats a permanent interactive terminal surface
  as the transcript's underlying input viewport;
- Flow hit testing no longer falls through to raw terminal interaction;
- full current-frame and Block-region rendering cannot leak into one visible
  presentation;
- first-responder/IME/mouse/input ownership follows the active presentation and
  transitions through the required fence;
- tests assert mode exclusivity, stale-event rejection and the absence of
  terminal pixels/input outside Flow Block output regions.

Issue #858 owns this architecture correction. Production implementation follows
in separate TDD implementation PRs after this amendment is accepted.

## Reopen conditions

Reopen this decision if trusted shell integration cannot preserve required shell
semantics, if command boundaries require scraping, if Block metadata must carry
copied terminal output, if one Pane compositor cannot meet required
virtualization/resource bounds, or if performance/security evidence shows that
Flow projection or transition fencing blocks terminal progress.

Originally approved by product authority on 2026-08-28. Presentation-mode
clarification requested by product authority on 2026-09-11 under #858. This
amendment remains proposed until its PR is explicitly approved and merged.
Independent architecture/security review and explicit merge confirmation remain
required.
