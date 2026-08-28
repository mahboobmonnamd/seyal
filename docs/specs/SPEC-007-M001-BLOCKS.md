# SPEC-007 — M001 minimal Block metadata and logical anchors

- **Status:** Proposed for M001 Pass 8
- **Date:** 2026-08-28
- **Issue:** #708
- **Architecture authority:** Foundation Architecture + ADR-001 + ADR-004 + ADR-005 + ADR-006 + ADR-007
- **Depends on:** SPEC-001, SPEC-003, SPEC-004; Pass 8 implementation also requires accepted/completed Pass 7 behavior from SPEC-006

## 1. Purpose

This specification defines the smallest production Block contract required by M001.

Pass 8 proves that Seyal can attach stable Block metadata to a real terminal execution and canonical logical history identity without creating a second terminal engine, copied transcript, guessed command model, or synchronous dependency in terminal progress.

The M001 proof is intentionally coarse:

```text
real TerminalExecution
→ real ExecutionId
→ canonical primary-screen LineId
→ Runtime/workspace-owned TerminalActivity Block
→ stable BlockId + BeforeLine(LineId)
→ Current
→ real terminal execution continues normally
→ existing final-drain/lifecycle completion
→ Completed
```

This is not the final command-Block product model. Trusted shell integration, per-command boundaries and the multiline composer remain later product work.

## 2. Authority and precedence

This specification is subordinate to:

- `docs/architecture/SEYAL-ARCH-FOUNDATION-RD-001.md`;
- `docs/architecture/SEYAL-RUNTIME-WORKSPACE-CONTINUITY-RD-001.md`;
- accepted ADRs;
- `docs/milestones/MILESTONE-001.md`;
- the current frozen Core Terminal UI reference set.

The current UI reference explicitly supersedes the earlier fixed-height/nested-output-scroll design. Normal transcript output has one Pane-level scroll owner, long-running normal-screen output grows with the Block, and full-screen TUI state uses the same terminal execution with Block/composer chrome yielding.

If implementation evidence requires any of the following, this specification must stop and the architecture-change process must run first:

- moving `BlockTimeline` authority into `TerminalExecution`;
- adding a Block-owned PTY, VT, grid, alternate grid or child process;
- creating a second canonical transcript/history model;
- making PTY/VT/damage progress synchronously wait for Block work;
- adding another Runtime/daemon/process authority for Blocks.

No new ADR is required for the contracts defined here because accepted architecture already fixes those ownership decisions.

## 3. M001 scope

Pass 8 implements only:

- a real typed `BlockId`;
- one coarse `TerminalActivity` Block for each M001 terminal execution for which Block metadata is successfully admitted;
- `ExecutionId` association;
- one stable primary-screen logical start anchor;
- monotonic `Current → Completed` state;
- Runtime/workspace ownership of the Block timeline;
- a bounded, capability-gated, read-only local Block metadata projection for the attached client;
- disposable client-side Block metadata state sufficient to prove the native presentation seam;
- deterministic correctness, failure, fuzz, resource and performance evidence.

The terminal remains fully usable when Block metadata is unavailable.

## 4. Non-negotiable invariants

1. `TerminalExecution` remains the sole owner of PTY, primary child lifecycle and canonical `TerminalState`.
2. `TerminalState` remains the sole owner of VT/parser modes, primary/alternate screens, logical line identity and damage.
3. `BlockTimeline` is Runtime/workspace metadata keyed by `ExecutionId`; it is not terminal infrastructure.
4. A Block never owns or copies PTY bytes, a terminal grid, an alternate grid, full output, renderer state or child-process state.
5. PTY read → VT mutation → canonical state → damage publication never synchronously waits for Block creation, mutation, delivery, persistence, semantics or rendering.
6. M001 Block creation does not parse terminal text, prompts, shell output, Enter presses or timing heuristics to infer command boundaries.
7. A durable Block start anchor is a canonical logical line identity, never viewport row, projection row, pixel position, renderer row or current cursor coordinates.
8. Resize, projection resync, renderer recreation, client detach/reattach and backing-scale changes do not rewrite Block identity or start anchor.
9. Alternate-screen entry/update/exit does not create another M001 Block and does not move the primary-screen Block anchor.
10. Client Block state is disposable and rebuildable from Runtime/workspace metadata.
11. Block metadata failure degrades presentation to raw terminal behavior; it never changes terminal execution correctness.
12. M001 Block storage and delivery are bounded and create no persistent timer/poll loop.

## 5. Canonical M001 data model

Conceptually:

```text
Block {
    id: BlockId,
    execution_id: ExecutionId,
    kind: TerminalActivity,
    start: BeforeLine(LineId),
    state: Current | Completed,
    revision: nonzero monotonic u64,
}
```

### 5.1 `BlockId`

`BlockId` is an opaque typed 128-bit identity generated under Runtime/workspace authority.

Requirements:

- nonzero;
- unique for the live Runtime lifetime;
- not derived from `ExecutionId`, `LineId`, viewport position or renderer coordinates;
- stable across GUI detach/reattach while the same Runtime and Block record remain alive;
- serialized as 16 little-endian raw bytes on the local protocol, consistent with other Seyal opaque IDs;
- no `Default` constructor that silently creates authority identity.

M001 makes no Runtime-restart/reboot persistence claim for `BlockId`.

### 5.2 `kind`

M001 defines exactly one Block kind:

```text
1 = TerminalActivity
```

`TerminalActivity` means only “coarse terminal-execution activity represented by this Block.” It must not be presented as a proven shell command boundary.

Values other than the accepted M001 kind are reserved.

### 5.3 `state`

M001 defines:

```text
1 = Current
2 = Completed
```

The only legal transition is:

```text
Current → Completed
```

The transition is monotonic and happens at most once.

### 5.4 `revision`

Each committed Block state has a nonzero per-Block revision.

For M001:

```text
Current   = revision 1
Completed = revision 2
```

A later accepted specification may generalize revisioning, but M001 implementations must not emit zero or regress revision.

## 6. Logical start anchor

### 6.1 Anchor form

M001 defines one anchor form:

```text
BeforeLine(LineId)
```

`LineId` is the canonical `u64` logical line identity owned by `seyal-terminal`. `0` is invalid; current canonical allocation starts at `1` and never intentionally reuses an allocated identity.

The anchor means “the Block begins immediately before this canonical logical primary-screen line.”

### 6.2 Anchor source

For the M001 coarse `TerminalActivity` Block, Runtime/workspace metadata captures the primary-screen logical line identity that represents the start of the execution's canonical terminal surface when the execution is published into Runtime composition.

The implementation may obtain that identity through a narrow read-only history-identity seam. It must not expose mutable `TerminalState`, grid storage or PTY ownership to workspace code.

### 6.3 Stability

Once committed, the M001 start anchor is immutable.

The following must not change it:

- cursor movement;
- line wrapping or physical row movement;
- primary-screen scroll;
- terminal resize;
- Candidate-D snapshot/delta replacement;
- client resync;
- renderer surface recreation;
- GUI detach/reattach;
- alternate-screen entry/exit.

If a future bounded-history policy makes the referenced line content unavailable, the Block record remains anchored to the same `LineId`. Consumers report the content/range as unavailable; they must never silently retarget the Block to another visible row or line.

### 6.4 Alternate screen

The M001 Block anchor is always a primary-screen logical identity.

Alternate-screen line identities, frames and redraws are not promoted into durable Block anchors or Block snapshots. Entering a TUI is a presentation transition over the same execution, not a Block lifecycle transition.

## 7. M001 Block lifecycle

### 7.1 Creation

After a `TerminalExecution` is successfully created and associated with Runtime/workspace composition, the Block subsystem may admit one `TerminalActivity` Block for that `ExecutionId`.

Creation belongs to execution/workspace management, not the PTY output hot path.

Normal state after successful admission:

```text
BlockId = new stable identity
ExecutionId = exact existing execution
kind = TerminalActivity
start = BeforeLine(initial primary LineId)
state = Current
revision = 1
```

A Block creation failure must not roll back or terminate an otherwise valid `TerminalExecution`.

### 7.2 Exactly one coarse M001 Block

M001 creates at most one `TerminalActivity` Block per `ExecutionId`.

The following do not create more Blocks:

- each shell command;
- Enter/Return;
- prompt redraw;
- output burst;
- primary scroll;
- long-running output;
- resize;
- TUI entry/exit;
- client attach/detach/resync.

This rule is milestone-scoped. Later trusted shell integration may introduce command Blocks through an amended/new accepted contract.

### 7.3 Completion truth

`Current` must not become `Completed` merely because:

- the PTY reports EOF while the primary child is still alive;
- the kernel reports primary-process exit before final output drain completes;
- a Controller detaches;
- the GUI exits/crashes;
- the terminal enters/leaves alternate screen;
- input is temporarily unavailable;
- termination has been requested but is not complete;
- Runtime is in a recoverable `TerminationFailed` state.

The M001 Block becomes `Completed` only when Runtime reaches the existing final execution-drain completion point: primary-child terminal truth is established and Runtime has completed its accepted final PTY-drain semantics such that no further canonical terminal bytes for that execution will be admitted.

At that point:

```text
state = Completed
revision = 2
```

The transition must be published at most once.

### 7.4 Retirement

M001 does not introduce production Block-history persistence.

Block metadata retention must remain bounded to Runtime's M001 execution population. A completed Block may be retired when its owning execution record is retired after the completed state has been made available to currently attached Block-capable clients. No unbounded list of historical completed Blocks may accumulate in Pass 8.

Future command history/persistence/virtualization belongs to later milestones.

## 8. Runtime/workspace ownership and scheduling

### 8.1 Ownership

Conceptual ownership:

```text
seyal-terminal
  LineId / canonical terminal history identity

seyal-exec / TerminalExecution
  PTY + child + canonical TerminalState
  read-only logical-anchor seam only

seyal-workspace logical/physical boundary
  BlockId / Block / BlockTimeline metadata

seyal-runtime
  execution/workspace composition
  bounded Block observation scheduling
  local Block metadata projection producer

seyal-client / macOS host
  disposable Block metadata cache/presentation only
```

A physical `seyal-workspace` crate is justified in Pass 8 only if it forms the real ownership/testing boundary described by repository architecture. The implementation must not create empty mirror crates merely to match diagrams.

### 8.2 No terminal hot-path dependency

Block creation/completion work must not execute as required synchronous work inside:

- PTY read dispatch;
- VT byte feed/parser mutation;
- canonical grid mutation;
- damage generation/publication;
- Candidate-D display extraction/encoding;
- renderer preparation/presentation;
- Pass 7 input admission/PTY write.

Execution/lifecycle code may produce a bounded coarse observation after the terminal/lifecycle authority transition has committed. Processing that observation must be bounded and must not make terminal correctness depend on successful Block mutation.

No per-output-line Block event is allowed in M001.

### 8.3 Failure isolation

If Block observation/admission/storage fails because of capacity, allocation or internal metadata failure:

- the execution remains valid;
- PTY/VT/input/rendering continue;
- no retry spin/timer is created;
- the client may expose a bounded non-secret “Block metadata unavailable” state or simply use raw terminal presentation;
- recovery may occur only on an explicit later lifecycle/attachment/rebuild opportunity defined by implementation, not through a busy loop.

## 9. Local Block metadata projection

M001 needs a minimal native proof that the real Runtime-owned Block identity can cross the existing local attachment boundary without terminal-state duplication.

This section is a narrow additive extension of SPEC-004 framing. It does not create another transport.

### 9.1 Capability

Allocate server/client capability bit 4:

```text
CAP_BLOCK_METADATA = 1 << 4
```

A client advertises support in `ClientHello.client_capabilities`. Runtime advertises support in `ServerHello.server_capabilities` only when the Pass 8 producer is available.

Runtime sends Block metadata only when both sides support the capability.

Existing clients that do not advertise the capability continue unchanged.

### 9.2 Message type

Allocate local protocol message type 20:

```text
20  R→C  BlockState
```

`BlockState` uses the existing SPEC-004 24-byte frame header and is never valid C→R.

### 9.3 Exact payload

`BlockState` is exactly 56 bytes:

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

- payload length is exactly 56;
- `ExecutionId` and `BlockId` must be nonzero typed identities;
- `revision != 0`;
- `start_line_id != 0`;
- `kind == TerminalActivity` for M001;
- `state` is `Current` or `Completed`;
- all reserved fields are zero.

The payload deliberately contains no:

- terminal cells/rows/output;
- command text;
- prompt text;
- cwd/environment;
- committed input;
- exit code/status enrichment;
- timestamp;
- renderer coordinates.

### 9.4 Attach and update behavior

For a successful Block-capable attachment, Runtime queues the latest Block state for the attached `ExecutionId` when such metadata exists.

Subsequent committed Block state changes are sent as another complete `BlockState` record.

M001 has at most two records for a successfully admitted Block during its normal lifecycle:

```text
revision 1 / Current
revision 2 / Completed
```

`BlockState` is bounded mandatory metadata, not an unbounded presentation history queue. If a client cannot accept mandatory bounded metadata under the existing connection backpressure rules, the connection may be closed/rebuilt; terminal progress never waits for the client.

There is no Block-specific acknowledgement or synchronous Runtime↔client round trip.

### 9.5 Relation to terminal display projection

`BlockState` and Candidate-D terminal display state are separate derived projections of different authority:

```text
TerminalState → display snapshot/delta → disposable DisplayCache
BlockTimeline → BlockState            → disposable BlockCache
```

Neither projection owns the other.

M001 defines no exact generation correlation between `BlockState` and terminal display generations because the coarse Block does not claim per-command output ranges. A client must not block terminal rendering while waiting for Block metadata.

## 10. Client Block cache

A Block-capable client may keep one disposable latest Block record for its attached M001 execution.

Rules:

- cache state is not authority;
- detach/disconnect destroys or invalidates attachment-local presentation state;
- reattach rebuilds from Runtime/workspace state;
- a higher revision replaces a lower revision for the same `BlockId`;
- an identical duplicate revision/payload is idempotent;
- a lower revision is stale and cannot regress client state;
- the same `BlockId` + same revision with conflicting payload is invalid metadata and must not partially mutate committed client Block state;
- metadata decode failure must not invalidate the committed terminal display cache or stop input/rendering.

The client must not infer missing command text/output or create a new authoritative Block when metadata is absent.

## 11. Minimal native presentation seam

Pass 8 may expose a minimal non-interactive Block identity/state treatment around the existing terminal presentation to demonstrate that native UI is consuming real Runtime-owned Block metadata.

Requirements:

- the terminal itself remains the permanent Pass 6 Metal surface;
- no `NSTextView`, SwiftUI text renderer or copied transcript renders terminal output;
- no client-generated `BlockId`;
- no fake command header/status derived from terminal scraping;
- if Block metadata is missing, show the correct raw terminal rather than fabricated Block chrome;
- alternate-screen/TUI takeover suppresses normal Block chrome and uses the same terminal surface/input path;
- normal transcript scrolling follows the current frozen Pane-level single-scroll-owner rule when richer history presentation is later implemented.

A full multiline composer, command Block card, Block actions and transcript virtualization are not Pass 8 acceptance requirements.

## 12. Failure and recovery behavior

Required failure properties:

1. Block metadata allocation/admission failure cannot fail a successfully created terminal execution.
2. Block completion-processing failure cannot prevent terminal final drain or process cleanup.
3. Local Block metadata encode/queue failure cannot block PTY/VT progress.
4. A stalled Block-capable client cannot backpressure another attachment or execution.
5. Malformed `BlockState` cannot corrupt terminal display state or client input state.
6. Disconnect clears disposable client Block state; reconnect rebuilds it from Runtime/workspace state if still available.
7. There is no timer-driven retry loop for missing metadata.
8. Metadata unavailability is not repaired by scraping terminal output.
9. Anchor-content eviction/unavailability never causes anchor retargeting.
10. Runtime shutdown may discard M001 Block metadata; Pass 8 makes no Runtime-crash/reboot restoration claim.

## 13. Performance and resource constraints

Pass 8 is expected to be effectively invisible to steady terminal performance.

### 13.1 Hot-path prohibitions

No Block implementation may add to terminal input/output/render hot paths:

- JSON or general serialization;
- command/prompt semantic parsing;
- transcript copying;
- per-line/per-cell Block allocation;
- persistence;
- agent/cloud/licensing/telemetry work;
- synchronous client acknowledgement;
- per-Block thread/process/task loop;
- blocking lock acquisition;
- busy polling or periodic timers.

New production functions that actually join a registered terminal hot path must be added to `scripts/check-hot-path.py`; the preferred Pass 8 design is that no Block-specific function joins those paths at all.

### 13.2 Bounds

M001 bounds:

- at most one admitted `TerminalActivity` Block per Runtime-known terminal execution;
- Block records scale O(active Runtime execution population), not O(output lines/bytes);
- no unbounded completed-history list;
- local attached client keeps at most one M001 Block record for its one attached execution;
- each wire `BlockState` payload is exactly 56 bytes;
- Block delivery uses existing bounded connection output capacity.

### 13.3 Measurement targets

Final Pass 8 implementation evidence must use the final accepted Pass 7 exact-head measurements as the comparison baseline on the same controlled Apple-Silicon methodology where applicable.

Targets:

- no >5% controlled p99 regression attributable to Pass 8 in steady PTY-output→canonical-state and native-input→PTY boundaries; any >5% movement requires root-cause explanation, and >10% is a blocker unless independently re-approved;
- Pass 6/7 renderer preparation/presentation measurements show no >5% controlled p99 regression attributable to Block metadata;
- idle CPU remains within measurement noise and Block support introduces zero persistent wake/timer source;
- fixed-size Block metadata encode + client apply p99 <= 250 µs on the controlled host;
- 512 admitted M001 Block records add <= 1 MiB retained Runtime RSS attributable to Block metadata in the controlled release workload, excluding terminal history/render payload;
- one attached disposable client Block cache remains O(1) and does not allocate proportional to terminal output.

If measurement shows the proposed projection or ownership boundary causes material terminal-path regression, stop and refine the design rather than weakening the baseline.

## 14. Security and privacy

Block metadata is a derived application-level capability and must obey the same local attachment trust boundary as SPEC-004.

Rules:

- Runtime sends Block metadata only to the authenticated connection attached to that `ExecutionId` and only after capability negotiation;
- Observer and Controller attachments may both read Block metadata; BlockState grants no mutation authority;
- there is no C→R Block mutation message in M001;
- malformed metadata never triggers terminal mutation;
- Block metadata contains no terminal text, command, prompt, environment, cwd, committed input or secret-bearing content;
- logs/telemetry must not add terminal contents merely to diagnose Block behavior;
- opaque IDs/line IDs may be recorded only where existing diagnostics policy permits and are never treated as authorization by themselves;
- client Block metadata cannot be used to bypass attachment/controller authorization.

## 15. Required tests and evidence

Implementation is TDD/evidence-first.

### 15.1 Identity and ownership

- `BlockId` generation uniqueness and wire round-trip;
- exact `BlockId → ExecutionId` association;
- Block ownership exists outside `TerminalExecution` and `seyal-terminal`;
- dependency/layering guard rejects terminal/exec dependencies on workspace Block ownership;
- OSS dependency graph contains no commercial code.

### 15.2 Anchor correctness

- Block start references the expected canonical primary `LineId`;
- no viewport-row value is stored as durable anchor identity;
- primary scroll can move the anchored line physically without changing the stored `LineId`;
- resize away/back does not change Block anchor;
- display snapshot/resync does not change anchor;
- anchor-content unavailability does not silently retarget to another line;
- line-identity exhaustion/error paths do not create duplicate/invalid anchors.

### 15.3 Alternate-screen/TUI

- enter alternate screen: same BlockId/start/state;
- repeated alternate-screen redraws: no Block creation/update storm;
- leave alternate screen: original primary anchor unchanged;
- alternate-screen line identities never become M001 Block anchors;
- native TUI presentation does not overlay normal Block chrome.

### 15.4 Lifecycle

- creation produces exactly one Current/revision-1 Block;
- repeated output/Enter/resize does not create additional Blocks;
- kernel primary-exit indication before final drain does not prematurely complete the Block;
- PTY EOF while child is still alive does not complete the Block;
- termination request alone does not complete the Block;
- `TerminationFailed` does not complete the Block;
- accepted final drain completion performs exactly one Current→Completed/revision-2 transition;
- duplicate lifecycle observation cannot emit a revision regression or second semantic transition.

### 15.5 Attach/reconnect

- Controller attach receives latest BlockState when capability negotiated;
- Observer attach receives the same read-only metadata;
- client without capability receives no BlockState and otherwise behaves unchanged;
- GUI detach/reattach to the same live Runtime/execution preserves BlockId/start/revision;
- disconnect clears disposable client Block state;
- reconnect rebuilds from Runtime/workspace metadata rather than client persistence.

### 15.6 Protocol and fuzzing

- exact 56-byte BlockState layout and little-endian fixtures;
- capability negotiation and direction rules;
- malformed length/reserved/kind/state/zero-id/zero-revision/zero-line rejection;
- retained protocol fuzz target extended for BlockState decode;
- stale revision ignored without regression;
- duplicate identical revision is idempotent;
- conflicting same-revision payload does not partially mutate client state;
- arbitrary chunking/partial socket writes preserve frame correctness under existing SPEC-004 framing.

### 15.7 Failure isolation

Inject and prove:

- Block record allocation/admission failure;
- Block observation queue/capacity failure if such a queue exists;
- BlockState encode failure path where injectable;
- mandatory client output pressure/stall;
- client metadata decode failure;
- disconnect during Current and during completion publication.

For every case, terminal execution/input/output/rendering and cleanup remain correct, with no busy retry.

### 15.8 Performance/resource

- controlled same-host Pass 7 comparison for input/output/render boundaries;
- 1/10/50/100/512 Block metadata population RSS/resource measurement where supported by existing Runtime population harness;
- idle CPU/wake observation;
- BlockState encode/client-apply latency;
- sustained high-output workload proves no per-line Block work/allocation growth;
- alternate-screen high-damage workload proves no Block update amplification.

## 16. Acceptance criteria

Pass 8 is complete only when all of the following are evidenced on the final implementation head:

- [ ] real `BlockId` references the exact real `ExecutionId`;
- [ ] BlockTimeline authority is Runtime/workspace metadata, not `TerminalExecution`;
- [ ] one M001 `TerminalActivity` Block is created without command/prompt scraping;
- [ ] start anchor is immutable `BeforeLine(LineId)` over canonical primary logical identity;
- [ ] viewport/projection/renderer coordinates are not durable Block identity;
- [ ] Current→Completed is monotonic and driven only by accepted final execution-drain truth;
- [ ] resize, resync, detach/reattach and alternate-screen transitions preserve Block identity/anchor;
- [ ] alternate-screen frames create no durable Block snapshots;
- [ ] no second PTY/VT/grid/output transcript exists;
- [ ] PTY→VT→damage and Pass 7 input have no synchronous Block dependency;
- [ ] capability-gated BlockState is bounded, read-only, exact-layout and rebuildable;
- [ ] client Block metadata is disposable and stale updates cannot regress state;
- [ ] metadata failure falls back to correct raw terminal behavior;
- [ ] no unbounded Block history, per-output allocation or periodic retry loop exists;
- [ ] required deterministic/integration/fuzz/failure tests pass;
- [ ] controlled performance/CPU/RSS targets pass or any exception is explicitly re-reviewed;
- [ ] `make bootstrap`, `make build`, `make test`, `make check`, `make bench` pass;
- [ ] Foundation Quality and applicable native macOS smoke are green;
- [ ] independent architecture/security/performance review has no unresolved blocker;
- [ ] OSS remains independent of commercial code.

## 17. Explicit non-goals / deferred behavior

Pass 8 does **not** implement or claim:

- trusted shell integration;
- per-command Block start/end truth;
- prompt detection/output scraping heuristics;
- command text, cwd, exit code or semantic status metadata;
- multiline composer or command submission;
- shell history/fuzzy search;
- Block copy/rerun/pin/expand/actions;
- rich Block inspector/cards;
- production transcript virtualization;
- production scrollback/reflow/million-line history;
- disk Block persistence or Runtime-crash/reboot recovery;
- agent/artifact/DevOps enrichments;
- multiple panes/tabs/workspaces implementation;
- mouse/clipboard/M002 terminal breadth;
- remote Block transport;
- public Block/plugin API;
- commercial features.

The product roadmap places coherent raw/Block/composer presentation and trusted shell integration in M003. Pass 8 establishes the permanent low-level Block identity/ownership seam those features will consume.

## 18. Implementation gate

This specification may be reviewed and accepted while Pass 7 implementation is in progress.

Pass 8 production implementation must not start or become Ready until:

1. Pass 7 implementation is merged and all Pass 7 required exits/evidence pass;
2. this specification is independently reviewed, accepted and merged;
3. current master is revalidated for any Pass 7 changes affecting Runtime/client/native seams;
4. a separate Pass 8 production implementation Issue is created from this accepted contract with the exact-head benchmark baseline frozen there.

If those conditions are not true, refinement may continue but production code waits.
