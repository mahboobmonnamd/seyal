# SPEC-007 — M001 minimal Block metadata and logical anchors

- **Status:** Proposed for M001 Pass 8; refinement only, production implementation blocked
- **Date:** 2026-08-28
- **Issue:** #708
- **Architecture authority:** Foundation Architecture + Runtime/Workspace Continuity + ADR-001 + ADR-004 + ADR-005 + ADR-006 + ADR-007
- **Depends on:** SPEC-001, SPEC-003, SPEC-004 and accepted SPEC-006; production implementation additionally requires Pass 7 implementation #706 / PR #707 to be merged and accepted

## 1. Purpose

This specification defines the smallest production Block contract required by M001.

Pass 8 proves that Seyal can attach durable Workspace-owned Block identity and stable logical-history anchoring to a real terminal execution without creating a second terminal engine, copied transcript, guessed command model, or synchronous dependency in terminal progress.

The M001 proof is deliberately coarse:

```text
WorkspaceId
→ real TerminalExecution / ExecutionId
→ canonical primary-screen LineId
→ Workspace-owned TerminalActivity Block
→ durable BlockId + BeforeLine(LineId)
→ Current
→ real terminal execution continues normally
→ accepted final PTY drain
→ final terminal display projection queued
→ BlockState::Completed queued when negotiated
→ Lifecycle::Finalized queued
→ M001 in-memory Block record retired with execution registry retirement
```

This is not the final command-Block product model. Trusted shell integration, per-command boundaries, transcript virtualization and the multiline composer remain later product work.

## 2. Authority and precedence

This specification is subordinate to:

- `docs/architecture/SEYAL-ARCH-FOUNDATION-RD-001.md`;
- `docs/architecture/SEYAL-RUNTIME-WORKSPACE-CONTINUITY-RD-001.md`;
- accepted ADRs;
- `docs/milestones/MILESTONE-001.md`;
- the current frozen Core Terminal UI reference set.

The Runtime/Workspace continuity contract is normative for identity lifetime: `BlockId` is a durable metadata identity owned by the Workspace Block timeline and is never reused within that Workspace. M001 does not yet persist/reload Block records, but it must not redefine the identity as Runtime-incarnation-scoped.

The current UI reference supersedes the earlier fixed-height/nested-output-scroll design. Normal transcript output has one Pane-level scroll owner, long-running normal-screen output grows with the Block, and full-screen TUI state uses the same execution with Block/composer chrome yielding.

If implementation evidence requires any of the following, stop and run architecture review before coding:

- moving `BlockTimeline` authority into `TerminalExecution`;
- adding a Block-owned PTY, VT, grid, alternate grid or child process;
- creating a second canonical transcript/history model;
- making PTY/VT/damage progress synchronously wait for Block work;
- adding another Runtime/daemon/process authority for Blocks.

No new ADR is required by this refinement because accepted architecture already fixes those ownership decisions.

## 3. M001 scope

Pass 8 implements only:

- a real typed, durable-semantics `BlockId`;
- one coarse `TerminalActivity` Block for each M001 terminal execution for which Block metadata is successfully admitted;
- exact `WorkspaceId` and `ExecutionId` association;
- one stable primary-screen logical start anchor;
- monotonic `Current → Completed` state;
- Workspace/Runtime ownership of the live Block timeline;
- deterministic mandatory retirement of the M001 in-memory record when the execution registry record retires;
- a bounded, capability-gated, read-only local Block metadata projection;
- disposable client Block metadata state sufficient to prove the native presentation seam;
- deterministic correctness, failure, fuzz, resource and performance evidence.

The terminal remains fully usable when Block metadata is unavailable.

## 4. Non-negotiable invariants

1. `TerminalExecution` remains the sole owner of PTY, primary child lifecycle and canonical `TerminalState`.
2. `TerminalState` remains the sole owner of VT/parser modes, primary/alternate screens, logical line identity and damage.
3. `BlockTimeline` is Workspace metadata composed/scheduled by Runtime; it is not terminal infrastructure.
4. Every Block belongs to exactly one owning `WorkspaceId` and references exactly one `ExecutionId` in M001.
5. `BlockId` has durable Workspace metadata semantics and is never reused within its owning Workspace, including across Runtime incarnations if the record is later persisted/restored.
6. A Block never owns or copies PTY bytes, a terminal grid, an alternate grid, full output, renderer state or child-process state.
7. PTY read → VT mutation → canonical state → damage publication never synchronously waits for Block creation, mutation, delivery, persistence, semantics or rendering.
8. M001 Block creation does not parse terminal text, prompts, shell output, Enter presses or timing heuristics to infer command boundaries.
9. A durable Block start anchor is a canonical logical line identity, never viewport row, projection row, pixel position, renderer row or current cursor coordinates.
10. Resize, projection resync, renderer recreation, client detach/reattach and backing-scale changes do not rewrite Block identity or start anchor.
11. Alternate-screen entry/update/exit does not create another M001 Block and does not move the primary-screen Block anchor.
12. Client Block state is disposable and rebuildable from Runtime/workspace metadata while that metadata remains available.
13. Block metadata failure degrades presentation to raw terminal behavior; it never changes terminal execution correctness.
14. M001 Block storage and delivery are bounded and create no persistent timer/poll loop.
15. If a client receives `Lifecycle::Finalized`, all final terminal display state for that execution has already been ordered before it; if a negotiated M001 Block exists, its `BlockState::Completed` is also ordered before `Lifecycle::Finalized` on that connection.

## 5. Canonical M001 data model

Conceptually:

```text
Block {
    id: BlockId,
    workspace_id: WorkspaceId,
    execution_id: ExecutionId,
    kind: TerminalActivity,
    start: BeforeLine(LineId),
    state: Current | Completed,
    revision: nonzero monotonic u64,
}
```

### 5.1 `BlockId` durability semantics

`BlockId` is an opaque typed 128-bit Workspace metadata identity.

Requirements:

- nonzero;
- owned by exactly one Workspace Block timeline;
- never reused within the owning Workspace after allocation, including after completion/retirement;
- not derived from `RuntimeId`, `ExecutionId`, `LineId`, viewport position or renderer coordinates;
- stable across GUI detach/reattach;
- not regenerated merely because Runtime restarts if a future persistence layer restores the same durable Block record;
- serialized as 16 little-endian raw bytes on the local protocol, consistent with other Seyal opaque IDs;
- no `Default` constructor that silently creates authority identity.

M001 does **not** implement disk persistence or Runtime-restart restoration of Blocks. If Runtime terminates in M001, the in-memory record may be lost. That implementation limitation does not change the identity contract: a replacement Block is a new record with a new `BlockId`; an old durable record must never be intentionally reidentified under a new ID if later persistence restores it.

An implementation may use a process-generated globally unique 128-bit value, but it must not use a Runtime-local deterministic counter that can knowingly reuse `BlockId` values within the same durable Workspace after restart.

### 5.2 `kind`

M001 defines exactly one Block kind:

```text
1 = TerminalActivity
```

`TerminalActivity` means only coarse terminal-execution activity. It must not be presented as a proven shell command boundary.

### 5.3 `state` and revision

M001 defines:

```text
Current   = state 1, revision 1
Completed = state 2, revision 2
```

The only legal semantic transition is:

```text
Current → Completed
```

It is monotonic and happens at most once. Revision zero is invalid; a revision may never regress.

## 6. Logical start anchor

### 6.1 Anchor form

M001 defines one anchor form:

```text
BeforeLine(LineId)
```

`LineId` is the canonical `u64` logical line identity owned by `seyal-terminal`. `0` is invalid.

The anchor means “the Block begins immediately before this canonical logical primary-screen line.”

### 6.2 Anchor source

After a `TerminalExecution` has been successfully created and associated with its owning Workspace, Runtime/workspace metadata captures the primary-screen logical line identity representing the start of the execution's canonical terminal surface.

The implementation may obtain that identity only through a narrow read-only terminal-history identity seam. It must not expose mutable `TerminalState`, grid storage or PTY ownership to workspace code.

### 6.3 Stability

Once committed, the M001 start anchor is immutable.

The following must not change it:

- cursor movement;
- line wrapping or physical row movement;
- primary-screen scroll;
- terminal resize;
- Candidate-D snapshot/delta replacement;
- client resync;
- renderer recreation;
- GUI detach/reattach;
- alternate-screen entry/exit.

If a future bounded-history policy makes referenced content unavailable, the record remains anchored to the same `LineId`. Consumers report content unavailable; they never silently retarget the Block.

### 6.4 Alternate screen

The M001 Block anchor is always a primary-screen logical identity. Alternate-screen lines, frames and redraws are not promoted into durable Block anchors or snapshots.

## 7. M001 Block lifecycle

### 7.1 Creation

After a `TerminalExecution` is published into Runtime with one owning `WorkspaceId`, the Block subsystem may admit exactly one `TerminalActivity` Block.

Successful admission commits atomically:

```text
workspace_id = exact owning WorkspaceId
execution_id = exact existing ExecutionId
block_id     = new durable Workspace metadata identity
kind         = TerminalActivity
start        = BeforeLine(initial primary LineId)
state        = Current
revision     = 1
```

Block creation failure does not roll back or terminate an otherwise valid terminal execution. The execution continues in raw-terminal mode with no Block record.

### 7.2 Exactly one coarse M001 Block

M001 creates at most one `TerminalActivity` Block per `ExecutionId`.

The following do not create additional Blocks:

- shell command submission;
- Enter/Return;
- prompt redraw;
- output bursts;
- primary scroll;
- long-running output;
- resize;
- TUI entry/exit;
- attach/detach/resync.

Later trusted shell integration may introduce command Blocks through a later accepted contract.

### 7.3 Completion truth

`Current` must not become `Completed` merely because:

- PTY EOF occurs while the primary child is still alive;
- the kernel reports primary-process exit before final output drain completes;
- a Controller detaches;
- the GUI exits/crashes;
- alternate screen enters/exits;
- input is unavailable;
- termination was requested but is incomplete;
- Runtime is in recoverable `TerminationFailed`.

The Block becomes `Completed` only after Runtime reaches the existing accepted final execution-drain point: primary-child terminal truth is established and no further canonical terminal bytes for that execution will be admitted.

At that point Runtime commits exactly once:

```text
state = Completed
revision = 2
```

Block completion is metadata truth about the already-completed terminal execution lifecycle; it cannot make terminal drain/process cleanup wait for Block mutation success.

### 7.4 Mandatory bounded retirement

Pass 8 retains **no completed Block history in Runtime after execution registry retirement**.

The retirement rule is mandatory:

1. while an execution is live or finalization is being emitted, at most one M001 Block record exists for that `ExecutionId`;
2. after accepted final drain, Runtime performs the ordered finalization sequence in section 10;
3. after each attached client's finalization frames are either admitted to its existing bounded output mechanism or that client is disconnected under existing backpressure policy, Runtime retires the execution registry record;
4. the M001 in-memory Block record for that execution is retired in the same bounded finalization/registry-retirement turn and **must not remain in `BlockTimeline` after the execution registry no longer contains that `ExecutionId`**;
5. retirement never waits for a client acknowledgement/read;
6. no timer, grace-period cache or optional completed-history list is allowed in M001.

Therefore M001 Block memory is bounded by the Runtime execution population plus bounded entries currently inside the existing finalization turn. It cannot grow with the number of historically completed executions.

Future durable completed Block history belongs to the later persistence/history milestone and must preserve the same durable `BlockId` semantics.

## 8. Ownership and scheduling

Conceptual ownership:

```text
seyal-terminal
  LineId / canonical terminal history identity

seyal-exec / TerminalExecution
  PTY + child + canonical TerminalState
  read-only logical-anchor seam only

seyal-workspace logical/physical boundary
  Workspace-owned BlockId / Block / BlockTimeline metadata

seyal-runtime
  execution/workspace composition
  bounded Block lifecycle observation
  local Block metadata projection producer

seyal-client / macOS host
  disposable Block metadata cache/presentation only
```

A physical `seyal-workspace` crate is justified only if Pass 8 creates a real ownership/testing boundary. Do not add an empty diagram-mirroring package.

Block-specific work must not execute as required synchronous work inside:

- PTY read dispatch;
- VT feed/parser mutation;
- canonical grid mutation;
- damage generation/publication;
- Candidate-D display extraction/encoding;
- renderer preparation/presentation;
- Pass 7 input admission/PTY write.

Execution/lifecycle code may emit bounded coarse observations after the terminal/lifecycle authority transition has committed. No per-output-line Block event is allowed.

## 9. Local Block metadata projection

Pass 8 adds a narrow read-only extension to the existing SPEC-004 framing. It does not create another transport.

### 9.1 Capability

Allocate capability bit 4:

```text
CAP_BLOCK_METADATA = 1 << 4
```

The client advertises support in `ClientHello.client_capabilities`; Runtime advertises it only when the Pass 8 producer is available. `BlockState` is sent only when both sides negotiated the capability.

### 9.2 Message type and payload

Allocate:

```text
20  R→C  BlockState
```

`BlockState` uses the existing 24-byte frame header and has exactly 56 payload bytes:

```text
u128 ExecutionId
u128 BlockId
u64  revision
u64  start_line_id
u8   kind
u8   state
u16  reserved0 = 0
u32  reserved1 = 0
```

Validation rules:

- exact payload length 56;
- nonzero `ExecutionId`, `BlockId`, revision and `start_line_id`;
- M001 `kind == TerminalActivity`;
- state is Current or Completed;
- reserved fields are zero;
- message is valid only R→C for the execution attached to that connection.

`WorkspaceId` is not duplicated on the wire because attachment already binds `ExecutionId` to Runtime's authoritative Workspace association. The Runtime-side Block record nevertheless stores and validates the exact owning `WorkspaceId`; protocol production must refuse to emit a record whose `(WorkspaceId, ExecutionId)` association disagrees with Runtime composition.

The payload deliberately contains no terminal cells/output, command/prompt text, cwd/environment, committed input, exit code, timestamps or renderer coordinates.

### 9.3 Attach and update behavior

On successful Block-capable attachment, Runtime queues the latest state for that attached execution if a Block record exists.

A normal admitted Block emits at most:

```text
revision 1 / Current
revision 2 / Completed
```

There is no Block-specific acknowledgement. Delivery uses existing bounded connection output accounting and never backpressures terminal progress.

### 9.4 Projection separation

```text
TerminalState → DisplaySnapshot/DisplayDelta → disposable DisplayCache
BlockTimeline → BlockState                   → disposable BlockCache
```

Neither projection owns the other. Block metadata never carries terminal content and does not create a second transcript.

## 10. Final display / Block / lifecycle ordering

This section closes the finalization ambiguity and is normative.

The existing M001 runtime contract already requires final terminal output to be delivered before a client observes `Lifecycle::Finalized`. Pass 8 extends that same ordered stream.

For an execution with final canonical generation/state `Gfinal`, Runtime finalization is logically:

```text
1. finish accepted PTY final drain
2. commit all resulting canonical TerminalState mutation/damage
3. ensure the final display snapshot/delta needed for the attached client's disposable display state at Gfinal (or a later equivalent final state) is admitted ahead of finalization
4. if an M001 Block record exists, commit Workspace Block state Current→Completed / revision 2
5. for each attached Block-capable client, admit BlockState(Completed, revision 2) after its final display state
6. admit Lifecycle::Finalized after the final display state and, when applicable, after BlockState::Completed
7. retire attachment/execution resources according to existing Runtime semantics
8. retire the M001 in-memory Block record with execution registry retirement
```

Observable per-connection order when a Block exists and `CAP_BLOCK_METADATA` was negotiated:

```text
final DisplaySnapshot/DisplayDelta batch
→ BlockState(Current? latest state already known, then Completed revision 2)
→ Lifecycle::Finalized
```

The required final event is specifically:

```text
final display bytes for the execution
< BlockState::Completed
< Lifecycle::Finalized
```

where `<` means ordered earlier on that connection's byte stream. A multi-chunk final display update must be completely ordered before `BlockState::Completed`; metadata must not appear between chunks of one atomic display batch.

When no Block was admitted or the client did not negotiate Block metadata:

```text
final display bytes
< Lifecycle::Finalized
```

Failure/backpressure rules:

- Runtime never waits for a client read/acknowledgement;
- if the final `BlockState::Completed` cannot be admitted within the existing bounded mandatory-output policy, that client is disconnected/failed under the existing connection policy rather than being allowed to observe `Lifecycle::Finalized` ahead of the missing completion metadata;
- execution finalization and Block retirement continue independently after bounded client cleanup;
- a client that actually receives `Lifecycle::Finalized` must therefore never observe an older Current Block state for the same negotiated admitted Block as its final authoritative metadata state;
- Block encode/storage failure cannot suppress final terminal display or hold the execution open. If no trustworthy Completed metadata can be produced, the client follows section 12 recovery/fallback and terminal lifecycle still completes.

No synchronous renderer acknowledgement is added by this ordering.

## 11. Client Block cache

A Block-capable client keeps at most one disposable latest M001 Block record for its attached execution.

Normal revision rules:

- higher revision for the same `BlockId` replaces lower revision;
- identical same-revision/same-payload duplicate is idempotent;
- lower revision is stale and ignored;
- a different `BlockId` for the same still-attached execution after one authoritative Block has already been accepted is a semantic conflict in M001;
- same `BlockId` + same revision with conflicting payload is a semantic conflict;
- `Completed → Current`, anchor change, execution-id change or unknown kind/state is a semantic conflict, even if framing itself is valid.

No conflicting update may partially mutate committed `BlockCache` or terminal `DisplayCache`.

## 12. Malformed/conflicting metadata recovery

Block metadata is optional presentation metadata, but corruption must fail closed deterministically.

### 12.1 Framing corruption

A malformed frame/header/length that violates SPEC-004 follows SPEC-004's protocol-fatal connection handling. The terminal execution remains alive because connection failure is not execution failure.

### 12.2 Valid frame with invalid Block semantics

Examples include:

- same Block/revision with conflicting payload;
- different BlockId for the same M001 execution after one Block was accepted;
- Completed→Current regression;
- anchor mutation;
- mismatched attached ExecutionId;
- invalid revision transition.

On the first such conflict for a `(RuntimeId, ExecutionId)` in the current client process:

1. do not mutate the committed Block cache;
2. mark Block metadata for that attachment as `Quarantined`;
3. immediately suppress all Block chrome/semantics and continue presenting the correct raw terminal from the unaffected DisplayCache;
4. close/detach the affected local connection using existing bounded cleanup; the terminal execution remains live;
5. reconnect through the normal attachment path **without advertising `CAP_BLOCK_METADATA`** for that `(RuntimeId, ExecutionId)` quarantine epoch;
6. rebuild display/input authority normally; do not reconstruct Blocks by scraping terminal output;
7. do not automatically re-enable Block metadata because socket writability, output progress, resize, focus or time changed.

The quarantine may be cleared only by an explicit user/developer recovery action or by observing a new `RuntimeId` incarnation and performing a fresh normal attach. Either event permits one fresh capability-negotiation attempt. Repeated failure reinstates quarantine. There is no retry timer or reconnect loop driven solely by Block failure.

If the client cannot reconnect, ordinary disconnected behavior applies; execution remains under Runtime authority.

### 12.3 Runtime-side Block metadata failure

Allocation/admission/observation/encode failure:

- must not fail an otherwise valid execution;
- must not block final drain/process cleanup;
- must not create retry spin/timer work;
- marks that execution's Block metadata unavailable for the current in-memory record lifecycle;
- suppresses Block projection rather than fabricating replacement metadata;
- terminal display/input/lifecycle continue normally.

If a partially trusted Runtime Block record violates its own invariant, it must be quarantined/removed from projection and treated as an internal metadata failure; it must never be repaired by changing its anchor or generating a replacement `BlockId` for the same live execution.

## 13. Minimal native presentation seam

Pass 8 may expose minimal non-interactive Block identity/state treatment around the existing Metal terminal surface solely to prove consumption of real Runtime-owned metadata.

Requirements:

- terminal rendering remains the permanent Metal path;
- no `NSTextView`, SwiftUI text renderer or copied transcript renders terminal output;
- no client-generated BlockId;
- no fake command header/status derived from scraping;
- missing/quarantined metadata shows correct raw terminal;
- alternate-screen/TUI takeover suppresses normal Block chrome and uses the same terminal surface/input path;
- richer transcript scrolling follows the frozen single Pane-scroll-owner design when later implemented.

A multiline composer, command Block cards, Block actions and transcript virtualization are not Pass 8 acceptance requirements.

## 14. Performance and resource constraints

Pass 8 is expected to be effectively invisible to steady terminal performance.

Forbidden on terminal input/output/render hot paths:

- JSON/general serialization;
- semantic command/prompt parsing;
- transcript copying;
- per-line/per-cell Block allocation;
- persistence;
- agent/cloud/licensing/telemetry work;
- synchronous client acknowledgement;
- per-Block thread/process/task loop;
- blocking lock acquisition;
- busy polling or periodic timers.

M001 bounds:

- at most one admitted Block per Runtime-known live/finalizing execution;
- zero retained completed Block records after execution registry retirement;
- Block records scale O(Runtime execution population), not O(output bytes/lines/history);
- one client Block cache record per attachment;
- one 56-byte payload per state emission;
- existing bounded connection output capacity is reused.

Implementation evidence uses the final accepted Pass 7 exact-head measurements as comparison baseline on the same controlled Apple-Silicon methodology where applicable.

Targets:

- any >5% controlled p99 movement attributable to Pass 8 in input/output/renderer boundaries requires root-cause explanation; >10% is blocking absent explicit re-review;
- no persistent idle CPU wake/timer source;
- fixed-size BlockState encode + client apply p99 <= 250 µs on controlled host;
- 512 simultaneous admitted M001 Block records add <= 1 MiB attributable retained Runtime RSS, excluding terminal/history/render payload;
- completing and retiring repeated executions returns Block metadata memory to the steady population bound rather than growing with historical count.

## 15. Security and privacy

Block metadata uses the same local attachment trust boundary as SPEC-004.

Rules:

- Runtime sends metadata only for the execution attached to that authenticated connection after capability negotiation;
- Observer and Controller may read metadata; `BlockState` grants no mutation authority;
- no C→R Block mutation message exists in M001;
- malformed metadata never triggers terminal mutation;
- metadata contains no terminal text, command, prompt, environment, cwd, committed input or secret-bearing content;
- diagnostics must not add terminal content to explain Block failures;
- opaque IDs/LineIds are not authorization by themselves;
- client metadata cannot bypass attachment/controller authorization.

## 16. Required tests and evidence

### 16.1 Identity / ownership / durability semantics

- BlockId generation uniqueness and wire round-trip;
- BlockId is owned by exact WorkspaceId and associated with exact ExecutionId;
- generator does not derive BlockId from RuntimeId and cannot intentionally reuse IDs after a simulated Runtime incarnation change for the same Workspace;
- restored-record fixture preserves the same BlockId semantics even though production persistence is not implemented in M001;
- Block ownership exists outside TerminalExecution/seyal-terminal;
- layering guard rejects terminal/exec dependency on workspace Block authority;
- OSS contains no commercial dependency.

### 16.2 Anchors

- expected canonical primary LineId is stored;
- viewport rows are never durable anchors;
- primary scroll can move physical position without changing LineId;
- resize away/back preserves anchor;
- display resync preserves anchor;
- content eviction/unavailability never retargets anchor;
- line-identity exhaustion/error cannot create duplicate/invalid anchors.

### 16.3 TUI

- alternate-screen enter/update/leave keeps same BlockId/start/state;
- repeated redraws create no Block update storm;
- alternate-screen line identities never become M001 anchors;
- native TUI presentation overlays no normal Block chrome.

### 16.4 Lifecycle and exact final ordering

- creation emits one Current/revision-1 Block;
- repeated output/Enter/resize creates no additional Block;
- primary-exit indication before final drain does not complete Block;
- PTY EOF while child alive does not complete Block;
- termination request or `TerminationFailed` does not complete Block;
- accepted final drain commits exactly one Current→Completed/revision-2 transition;
- final display batch is completely ordered before BlockState::Completed;
- BlockState::Completed is ordered before Lifecycle::Finalized for negotiated admitted Block;
- client receiving Lifecycle::Finalized can already observe terminal final tail bytes and Completed Block state;
- no-Block/no-capability path preserves existing final-display-before-Finalized contract;
- inability to admit final BlockState causes bounded client failure/disconnect, not Lifecycle overtaking and not execution stall.

### 16.5 Mandatory retirement / bounded memory

- completed Block is absent immediately after its execution registry record retires;
- 10,000 sequential short executions do not accumulate completed Block records;
- client stall during finalization cannot retain Block records indefinitely;
- execution retirement + Block retirement is idempotent under duplicate lifecycle observations;
- 1/10/50/100/512 simultaneous populations stay within documented bounds.

### 16.6 Attach/reconnect

- Controller and Observer receive latest metadata when negotiated;
- client without capability receives none and remains correct;
- detach/reattach to same live Runtime/execution preserves BlockId/start/revision;
- reconnect rebuilds from Runtime/workspace state, not client persistence.

### 16.7 Protocol / malformed metadata / recovery

- exact 56-byte layout and little-endian fixtures;
- capability and R→C direction rules;
- malformed length/reserved/kind/state/zero-id/zero-revision/zero-line rejection;
- protocol fuzzer covers BlockState decode/state transitions;
- stale lower revision cannot regress state;
- identical duplicate is idempotent;
- conflicting same revision, anchor mutation, BlockId swap and Completed→Current leave both BlockCache and DisplayCache unchanged;
- first semantic conflict quarantines Block metadata and reconnects without capability rather than looping;
- quarantine persists for the current `(RuntimeId, ExecutionId)` epoch until explicit recovery or RuntimeId change;
- raw terminal remains usable after metadata quarantine;
- malformed Block metadata is never repaired by terminal scraping.

### 16.8 Failure / performance

Inject and prove allocation/admission/observation/encode/output-pressure/decode failures while terminal execution/input/output/rendering and cleanup remain correct.

Measure:

- controlled same-host Pass 7 input/output/render comparison;
- idle CPU/wakes;
- BlockState encode/client-apply latency;
- repeated completion/retirement memory high-water and return;
- sustained high-output and alternate-screen high-damage workloads proving no per-line/update amplification.

## 17. Acceptance criteria

Pass 8 implementation is complete only when all are evidenced on the final implementation head:

- [ ] BlockId has durable Workspace-owned identity semantics and never Runtime-scoped/reused semantics;
- [ ] exact WorkspaceId + ExecutionId association is enforced;
- [ ] one coarse TerminalActivity Block is created without command/prompt scraping;
- [ ] start is immutable BeforeLine(LineId) over canonical primary identity;
- [ ] viewport/projection/renderer coordinates are not durable anchors;
- [ ] Current→Completed occurs only after accepted final drain;
- [ ] final display < BlockState::Completed < Lifecycle::Finalized ordering is proven where Block capability applies;
- [ ] no-Block path preserves final display < Lifecycle::Finalized;
- [ ] completed M001 Block record is mandatorily retired with execution registry retirement;
- [ ] repeated completed executions cannot grow retained Block memory;
- [ ] resize/resync/detach/reattach/TUI preserve Block identity/anchor;
- [ ] alternate-screen frames create no durable Block snapshots;
- [ ] no second PTY/VT/grid/output transcript exists;
- [ ] PTY→VT→damage and Pass 7 input have no synchronous Block dependency;
- [ ] capability-gated BlockState is bounded, read-only and exact-layout;
- [ ] malformed/conflicting metadata has deterministic quarantine/raw-terminal recovery with no retry loop;
- [ ] client Block metadata is disposable and stale/conflicting updates cannot partially mutate state;
- [ ] required deterministic/integration/fuzz/failure tests pass;
- [ ] controlled performance/CPU/RSS targets pass or exception is independently re-reviewed;
- [ ] `make bootstrap`, `make build`, `make test`, `make check`, `make bench` pass;
- [ ] Foundation Quality and applicable native macOS smoke are green;
- [ ] independent architecture/security/performance review has no unresolved blocker;
- [ ] OSS remains independent of commercial code.

## 18. Explicit non-goals

Pass 8 does not implement or claim:

- trusted shell integration;
- per-command Block start/end truth;
- prompt detection/output scraping heuristics;
- command text, cwd, exit code or semantic status metadata;
- multiline composer/command submission;
- shell history/fuzzy search;
- Block copy/rerun/pin/expand/actions;
- rich Block inspector/cards;
- production transcript virtualization;
- production scrollback/reflow/million-line history;
- production disk Block persistence or Runtime-crash/reboot restoration;
- agent/artifact/DevOps enrichments;
- multiple panes/tabs/workspaces UI implementation;
- mouse/clipboard/M002 terminal breadth;
- remote Block transport;
- public Block/plugin API;
- commercial features.

The roadmap places coherent raw/Block/composer presentation and trusted shell integration in M003. Pass 8 establishes the permanent low-level identity/ownership seam those features will consume.

## 19. Dependency and implementation gate

### 19.1 Refinement authority

SPEC-006 was accepted by refinement PR #703. Its source/index status must say Accepted before this Pass 8 refinement is eligible to merge; stale “Proposed” wording is an authority defect, not evidence that Pass 7 implementation is complete.

Pass 7 production implementation remains independently incomplete while #706 / PR #707 is open/draft or lacks its required final evidence.

### 19.2 Separate Pass 8 implementation Issue

A separate Pass 8 production implementation Issue must exist before this refinement closes. It must remain explicitly **Blocked / NOT_READY** while any dependency below is unsatisfied. Creating it is dependency bookkeeping, not authorization to code.

### 19.3 Production start gate

Pass 8 production development must not start and the implementation Issue must not move to Ready until all are true simultaneously:

1. SPEC-006 source/index authority is Accepted with #703 provenance;
2. Pass 7 implementation #706 / PR #707 is merged and every Pass 7 required exit/evidence has passed;
3. this SPEC-007 refinement is independently re-reviewed after these blocker fixes, accepted and merged;
4. current master is revalidated after Pass 7 for Runtime/client/native seam changes;
5. the separate Pass 8 implementation Issue records the final accepted SPEC-007 revision and freezes the exact-head Pass 7 benchmark/evidence baseline.

Until then:

```text
SPEC-007 refinement may be reviewed
Pass 8 implementation Issue = Blocked / NOT_READY
Pass 8 production code = forbidden
```

## 20. Refinement Definition of Done

Issue #708 / this refinement PR may close only when:

- [ ] BlockId lifetime matches authoritative durable Workspace semantics;
- [ ] mandatory completed-record retirement makes the memory bound enforceable;
- [ ] malformed/conflicting metadata recovery is deterministic and loop-free;
- [ ] final display / Block Completed / Lifecycle Finalized ordering is exact;
- [ ] SPEC-006 source/index stale Proposed status is corrected to Accepted with #703 provenance;
- [ ] a separate blocked Pass 8 implementation Issue exists;
- [ ] this revised contract receives independent review with no unresolved blocker;
- [ ] repository/docs/CI validation is green on the revised exact PR head.

Passing CI alone does not satisfy these behavioral-contract gates.