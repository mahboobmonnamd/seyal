# ADR-009 — Command Blocks, Pane Composer, and Presentation Takeover

- **Status:** Accepted 2026-08-28; presentation amendment accepted 2026-09-11 by #858 / PR #859 (`8d08f2f`); trusted shell-integration injection mechanism accepted 2026-09-16 by #968
- **Date:** 2026-08-28; presentation amendment 2026-09-11; shell-integration injection amendment 2026-09-16
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

## 2026-09-11 accepted correction — authority is not viewport

The original wording correctly rejected PTY/grid/renderer-per-Block designs, but
it did not state strongly enough that Flow, Raw and TUI must not be presented at
the same time. The then-current macOS implementation consequently reused the
Pane's interactive Metal terminal surface as a permanent full-transcript
backing/input surface while also placing Block chrome over it.

That interpretation is rejected by this accepted amendment.

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
1. Rust freezes the old route and increments the epoch.
2. Native revokes input, mouse capture, first responder, AX focus, and marked text.
3. Stale callbacks fail closed.
4. Rust validates current destination eligibility.
5. Native realizes only that destination.
6. The destination route becomes active.
```

No destination route becomes eligible while the source route can still admit
input. One native event may be admitted to at most one presentation route.
Anything already atomically admitted before the fence retains normal FIFO
semantics; unadmitted stale callbacks are rejected rather than replayed.
Host completion may gate only local route activation. It must never stall
PTY, VT, or output progress.

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

## Composer ownership

Rust owns the authoritative committed draft, revision, mode, and submission
correlation for the Pane composer.

Native `NSTextView` / IME owns only bounded marked text and a disposable
derived editor cache. Native committed edits are revisioned against the Rust
draft. Stale edits fail closed and rehydrate from the current Rust snapshot.

Return during marked text remains IME-owned. Unmarked Return requests a Rust
execute action. The host must not submit composer text to the PTY, invent
command identity, or clear the authoritative draft.

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

Rejected by this accepted amendment. It conflates canonical terminal authority
with visible presentation, permits Raw interaction to leak through Flow, and
makes Blocks decorative chrome rather than the primary execution presentation.

### E. Trusted shell integration + logical history anchors + explicit presentation modes

Selected. Runtime-owned integration emits bounded command-boundary metadata
associated with the same `ExecutionId` and canonical primary-history `LineId`s.
The GUI consumes read-only Block metadata and terminal display projection while
an explicit Flow/Raw/TUI state determines how those projections are presented
and where input is routed.

## 2026-09-16 accepted amendment — silent shell-integration injection

Alternative E selected "trusted shell integration" without specifying how hook
installation and command-boundary correlation reach the shell without becoming
visible terminal content. The current zsh implementation (`zsh_composer_command` in
`crates/seyal-runtime/src/runtime/shell_integration.rs`) injects an entire
hook-install-and-marker script as literal interactive PTY input on every
composer submission, prefixed to the user's command with a
fresh random per-submission token typed into the line. Live-PTY evidence
collected for #968 shows this is not a suppressible detail: zsh's line editor
(ZLE) enables bracketed paste and explicitly redraws whatever is written to
the PTY while it is reading interactively, independent of kernel TTY echo
state. That redraw made Runtime's own instrumentation script visible inside
Block output regions, violating invariant 9 below and the Flow output-region
promise, and is exactly the condition named in this ADR's original reopen
conditions ("if trusted shell integration cannot preserve required shell
semantics"). The injected script is also recorded in the user's zsh history,
a second defect independent of the visibility one.

An earlier draft of this amendment proposed correlating
`CommandStarted`/`CommandFinished` events to pending composer submissions by
strict FIFO submission order, reasoning that SPEC-004 §5 bounds `attachments
per connection` and `controllers per execution` to one and therefore
serializes input. Review for #968 rejected that half: SPEC-004's bound
serializes input *bytes*, not shell readiness. Concrete failure: the user runs
`python` interactively, then submits `pwd` from the composer; `python`
consumes the `pwd\r` bytes as its own stdin; `preexec` never fires for it; the
pending composer item is left outstanding; when `python` later exits and the
user types `ls` directly, the stale pending `pwd` item is misattributed to
`ls`'s `preexec`/`precmd` pair. Seyal's parser today recognizes only
`133;C;<token>` and `133;D;<token>;<status>`
(`crates/seyal-terminal/src/terminal.rs`, around line 2034); there is no
"shell is at a prompt" signal to gate on, so FIFO order alone cannot
distinguish a live prompt from a busy foreground program. The FIFO draft is
rejected.

**Accepted mechanism:**

1. **Static bootstrap via `ZDOTDIR`; no runtime file writes, no logs.**
   Runtime (product composition, in the spawn path owned by
   `TerminalExecution`/`seyal-exec`; the PTY layer stays policy-neutral, the
   same split as ADR-008) launches zsh with `ZDOTDIR` pointing at a directory
   of integration scripts shipped statically inside the Seyal bundle. Nothing
   is written to disk at spawn or per command. The user's original `ZDOTDIR`
   (or its absence) is passed through the spawn environment so it can be
   restored. This mirrors the `ZDOTDIR`-wrapper technique used by kitty and
   VS Code shell integration and the temporary-`ZDOTDIR` bootstrap used by
   Warp; Seyal's specifics are constrained by this ADR, not by those products.

   The bundled scripts must, at minimum:

   - source the user's own `.zshenv`, `.zprofile`, `.zshrc` and `.zlogin`,
     from the user's real `ZDOTDIR`/`HOME`, in zsh's standard startup order;
   - register hooks only for interactive shells (`[[ -o interactive ]]`), so a
     script invoked as `zsh file.sh` pays zero cost;
   - register hooks after the user's interactive rc runs, so user
     configuration cannot silently remove them; if it does, no trusted
     prompt-start marker ever arrives and the execution fails closed to
     `Unsupported` (mechanism 5);
   - restore `ZDOTDIR` to the user's value before the first prompt is drawn,
     so nested `zsh`, `ssh` and other child processes inherit the user's real
     configuration and are never re-bootstrapped;
   - complete hook installation during ordinary shell startup, before any
     prompt is drawn, so installation is never presented to ZLE as typed
     input, is never redrawn, and never enters shell history.

   File layout inside the bundle is an implementation detail for the
   implementing Issue, not part of this decision.

2. **Per-execution secret delivered invisibly.** Runtime generates one random
   16-byte nonce per `TerminalExecution` (the same size as today's
   `ShellIntegrationToken`) and passes it in the spawn environment. The
   bundled startup script copies it into a non-exported shell parameter and
   removes it from the exported environment, so child processes cannot read
   it. The nonce is never typed, never drawn on screen, and never enters
   shell history. Every marker in mechanism 3 carries this nonce; a marker
   with a missing or mismatched nonce is untrusted and is ignored (accounted
   through the existing deferred/malformed counters) and never affects Block
   state. This preserves today's anti-spoofing property: a program cannot
   forge a `D` to end its own Block early, and cannot forge an `A`/`C` to
   trick the composer into sending input to a program it does not own.

3. **Markers: OSC 133, BEL- or ST-terminated, bounded.**

   - `A;<nonce>` — emitted by `precmd` immediately before the prompt is drawn:
     prompt start. Runtime transitions the execution to `AtPrompt`.
   - `C;<nonce>;<cmdline>` — emitted by `preexec` with the command line zsh is
     about to execute (`$1`, the text as typed). `cmdline` is the final field
     and is taken verbatim to the terminator; it is bounded to a fixed byte
     cap not exceeding SPEC-004 §5's 65,536-byte per-`Input` bound and the
     existing `MAX_COMMAND_BYTES`; trailing whitespace is trimmed; and
     control characters — including any 7-bit or 8-bit byte that could
     terminate or escape the OSC string — are replaced with a fixed
     placeholder, using zsh parameter expansion only. Runtime applies the
     identical bounding/sanitization to its own pending command text before
     comparing.
   - `D;<nonce>;<exit-status>` — emitted by `precmd` when a `C` is open:
     command finished. When `precmd` has an open `C`, it emits `D` before `A`,
     in that order, so Runtime always observes completion before the next
     prompt start.

   Hooks may use only zsh builtins (`print`/`printf`, parameter expansion,
   `add-zsh-hook`). No forks, no external commands, no subshells, no file
   reads or writes per command. This is a hard requirement, not a performance
   preference.

4. **Prompt-gated, single-in-flight, exact-match admission.**

   - Composer submissions write only the literal command bytes plus `\r` to
     the PTY — never a wrapper, marker, token or hook text.
   - Admission requires: execution state is `AtPrompt` (a trusted `A`
     observed since the last `D`, or the first trusted `A` after spawn); no
     pending composer command; no active Block; primary screen (not
     alternate/TUI). Otherwise the result is the existing correlated
     `Busy`/`Unsupported` admission result, never a transport error, and the
     draft is kept (unchanged from current ADR-009 composer semantics).
   - On send: state becomes `Pending{command}`; at most one composer command
     is ever in flight.
   - On trusted `C`: if `sanitize(cmdline) == sanitize(pending.command)`, the
     pending item becomes the Running Block (existing `BlockTimeline.start`);
     otherwise the pending item is dropped, no Block is created, and the
     event is treated as untrusted. Matching is by secret plus exact text,
     never by arrival order alone.
   - Any trusted `C`, with or without a pending item, leaves `AtPrompt`; the
     execution returns to `AtPrompt` only on the next trusted `A`. This keeps
     the composer disabled while a directly typed (Raw) command is running
     even if the user switches back to Flow.
   - On trusted `D`: complete the active Block with the exit status (existing
     `BlockTimeline.complete`).
   - Invalidation makes stale pending items impossible by construction: a
     trusted `A` arriving while `Pending` and before any `C` drops the
     pending item (the bytes were consumed by something that was not the
     shell prompt, or the command never ran); entering the alternate screen,
     primary-child exit, or PTY EOF drops any pending item and sets state to
     `NotAtPrompt`. A directly typed command (Raw presentation) that produces
     a `C`/`D` pair with no pending item creates no Block — unchanged
     behavior: no guessed Blocks.
   - Nested interactive shells (`zsh`, `ssh`) run inside the outer command's
     Running Block until they exit, because they carry no hooks and no
     nonce; the composer stays disabled (invariant 7) until the outer shell's
     next trusted `A`.
   - If the shell process is replaced (`exec zsh`, `exec bash`), no further
     trusted markers can arrive for that execution: the Running Block
     completes only through Runtime lifecycle truth (invariant 15), the
     execution never becomes composer-eligible again, and presentation
     follows the existing Raw rules while trusted eligibility is absent.
     This matches kitty/Ghostty behavior and is accepted.

5. **Failure fails closed to `Unsupported`, never half-installed.** A missing
   bundled script, inability to set `ZDOTDIR`/nonce in the spawn environment,
   or no trusted `A` observed after spawn transitions the execution to
   `ShellIntegrationMode::Unsupported`: composer disabled, raw terminal fully
   usable. Non-zsh shells remain `Unsupported` (unchanged; out of scope).
   Absence of a trusted `A` is not detected by a timer or retry loop: the
   execution simply never becomes composer-eligible (invariant 7) and Raw
   remains fully usable. Static program-path detection (`/bin/zsh`) remains
   a precondition for attempting installation, never proof that it
   succeeded.

6. **Performance invariants, non-negotiable for this amendment.**

   - Today's parser already handles two bounded OSC 133 sequences per command
     (`C`, `D`) with bounded queue capacity; this amendment adds one bounded
     sequence per prompt (`A`) and changes nothing else on the
     PTY → VT → `TerminalState` hot path.
   - Zero per-command file I/O, forks, subprocesses, synchronous IPC,
     per-attachment loops, or new allocations on the hot path beyond the
     existing bounded `shell_events` queue. Per-command byte cost decreases
     versus today: roughly 130 bytes of markers plus the command text once,
     instead of roughly 600 bytes of wrapper script written as input and
     echoed back as output on every submission.
   - Startup cost is one extra `source` of small static files during
     interactive-shell initialization only; non-interactive shells pay
     nothing.
   - The implementing Issue must measure shell-start-to-first-prompt time and
     per-command marker overhead with integration on versus off, and show no
     regression against the M002 performance contract
     (`scripts/check-m002-performance-contract.py`); `performance-gate`
     applies. A measurable regression is a blocking finding.

7. **Comparative performance gate.** Product authority requires this
   mechanism's performance to be equal to or better than Warp, Ghostty, and
   terminals embedding libghostty (cmux), not merely non-regressive against
   Seyal's own prior baseline.

   - Structural parity with kitty, Ghostty, and libghostty-based terminals
     (cmux) — static bundled `ZDOTDIR` bootstrap, builtin-only hooks, and a
     fixed number of bounded OSC markers per command — is asserted by
     construction above. Parity by construction is not evidence; it must be
     measured before #967 can close.
   - Required comparative evidence for the implementing Issue, all on the
     same physical Apple Silicon host, same zsh binary, same user dotfiles,
     same workload, each terminal running its own current shell integration,
     reported as nearest-rank p50/p95/p99 with sample counts, commit hash and
     host description in the repository's existing evidence style under
     `docs/evidence/`:
     - (a) shell spawn to first trusted prompt (integration startup cost);
     - (b) prompt-to-prompt latency for a trivial command (e.g. `true`) — the
       per-command integration overhead;
     - (c) throughput and CPU for a bulk-output command (e.g. tens of MB via
       `cat`/`yes | head`) with integration active — proving markers add
       nothing to the PTY → VT hot path;
     - (d) Seyal with integration on versus off (own-baseline regression).
   - Acceptance rule: Seyal must be equal to or better than each of Warp,
     Ghostty, and cmux on (a) and (b) at p50 and p95, within stated
     measurement noise, and show no regression on (c) and (d) beyond noise.
     "Equal to" means within the reported noise band; anything worse is a
     blocking finding for #967 and for merge, per `performance-gate`.
     Measurements of third-party terminals are black-box (for example
     shell-side `EPOCHREALTIME` timestamps in `precmd`/`preexec`, or an
     external PTY harness) and must be reproducible from a documented
     script.
   - Result (c) is also reported for the same three terminals. A shortfall
     on (c) alone is whole-terminal throughput owned by the milestone
     performance contract, not by this amendment; it must be filed as a
     performance Issue against that contract rather than ignored, and it
     blocks #967 only if (d) shows the integration itself caused it.
   - The comparative evidence is re-run whenever the bundled integration
     scripts or the marker parser change.

### Rejected alternatives

- **Inline per-command injection with echo suppression** (for example
  `stty -echo` around the write). Rejected: ZLE's redraw is application-layer
  behavior independent of kernel echo state, confirmed by live-PTY capture;
  no such suppression flag exists.
- **Per-command token typed into the command line** (the mechanism this
  amendment replaces). Rejected: visible on screen inside Block output and
  recorded in shell history.
- **Blind FIFO/arrival-order correlation.** Rejected during #968 review for
  the stdin-consumer race described above (the `python`/`pwd` example).
- **Out-of-band per-command token via a file read in the hook.** Rejected:
  reintroduces per-command file I/O on the hot path, forbidden by
  mechanism 6.
- **Editing the user's `~/.zshrc` or other dotfiles directly.** Rejected:
  modifies user-owned files.

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
18. A presentation transition follows the Rust-freeze / native-revoke /
    fail-closed / Rust-eligibility / native-realize / destination-active order.
    Host completion may gate only local route activation, never PTY/VT/output
    progress.
19. Eligibility/admission is fenced to current execution, attachment/controller,
    presentation epoch and relevant canonical/integration generation; stale
    evidence fails closed.
20. One native input event is admitted through at most one presentation route.

## Relationship to SPEC-006 and SPEC-009

SPEC-006 remains authoritative for direct-terminal native event classification,
IME composition semantics, bounded input queues, Runtime-owned key encoding and
resize transactions. Under this accepted amendment, wording that assigns those
duties to a permanent terminal surface is scoped to the **active
Raw/TUI/direct-terminal presentation endpoint**. It does not authorize a
focusable terminal surface under Flow.

SPEC-009 remains authoritative for Runtime/PTY survival, fresh AttachmentId and
Controller reacquisition, state reconstruction and reconnect fencing. Under this
accepted amendment, reconnect restores interaction to the **newly selected
current presentation owner** after validating fresh execution/attachment/canonical
state:
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
- statically bundled zsh integration scripts as build artifacts, plus
  spawn-environment composition of `ZDOTDIR`, the user's original `ZDOTDIR`
  and the per-execution nonce (Runtime product composition; the PTY layer
  stays policy-neutral);
- `A` prompt-start marker parsing, an explicit per-execution
  `AtPrompt`/`Pending`/`Running`/`NotAtPrompt` integration state, and bounded
  sanitize-and-compare of `C` command text;
- comparative and own-baseline shell-integration performance evidence per
  amendment mechanisms 6–7;
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

Issue #858 / PR #859 (`8d08f2f`) accepted this architecture correction.
Production presentation implementation follows in separate TDD Issues after the
Rust/native host contract is frozen; this ADR does not restore a headed host.

Issue #968 accepted the silent shell-integration injection mechanism above.
Issue #967 (composer wrapper text visible in Block output regions) returns to
Ready implementation against that accepted mechanism.

## Reopen conditions

Reopen this decision if trusted shell integration cannot preserve required shell
semantics, if command boundaries require scraping, if Block metadata must carry
copied terminal output, if one Pane compositor cannot meet required
virtualization/resource bounds, or if performance/security evidence shows that
Flow projection or transition fencing blocks terminal progress. Reopen also if
a supported shell cannot host the hooks using builtins only (no fork or
external command), if the measured per-command or startup overhead is not
negligible under the performance evidence required above, or if `ZDOTDIR`
restoration is found to break user configuration or nested-shell behavior.
Reopen also if the comparative evidence shows Seyal cannot meet equal-or-better
against Warp, Ghostty, or cmux with this mechanism.

Originally approved by product authority on 2026-08-28. Presentation-mode
clarification requested by product authority on 2026-09-11 under #858 and
accepted on merge of PR #859 as `8d08f2f`. Silent shell-integration injection
mechanism approved by product authority on 2026-09-16 under #968.
