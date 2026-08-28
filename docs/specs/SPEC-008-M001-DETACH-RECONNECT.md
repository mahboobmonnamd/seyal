# SPEC-008 — M001 detach, reconnect and GUI-crash survival

- **Status:** Proposed for M001 Pass 9 / Issue #717. Production implementation is blocked until Pass 8 exits.
- **Date:** 2026-08-28
- **Issue:** #717
- **Architecture authority:** accepted Seyal foundation architecture, ADR-001/004/005/006/007
- **Depends on:** SPEC-002, SPEC-003, SPEC-004, SPEC-005, SPEC-006 and the finally accepted SPEC-007

## 1. Purpose

M001 Pass 9 proves that Seyal presentation lifetime is independent from live terminal-execution lifetime on macOS.

Required proof:

```text
live Seyal.app surface
→ attached Controller for ExecutionId E
→ close/detach GUI or abruptly kill/crash GUI
→ Runtime stays alive
→ same PTY + child + canonical TerminalState for E stay alive
→ reopen Seyal.app
→ discover same RuntimeId
→ attach to E with a new AttachmentId
→ reconstruct disposable client/display/renderer state from Runtime authority
→ resume rendering + input + resize
```

The proof uses the permanent Runtime, Candidate-D transport, client state, native input/resize and Metal renderer. No temporary reconnect path is permitted.

Runtime-crash live-PTY survival is explicitly not M001 Pass 9.

## 2. Non-negotiable invariants

1. `TerminalExecution` remains sole owner of PTY, child lifecycle and canonical `TerminalState`.
2. GUI window/process, local connection, `AttachmentId`, display cache, renderer resources, IME state and resize-reconciliation state are disposable presentation/client state.
3. Losing every GUI/client attachment does not terminate a live `TerminalExecution` and does not stop the per-user Runtime.
4. Reconnect reconstructs from current Runtime/canonical state. Historical PTY bytes are never replayed into a client VT/parser.
5. One live Runtime incarnation has one stable `RuntimeId`; reconnect to the surviving Runtime observes that same ID.
6. A surviving live execution retains the same `ExecutionId`; a replacement process after terminal death must receive a new `ExecutionId`.
7. Every new attachment receives a fresh `AttachmentId`. Stale attachment, controller and connection-local request identities never regain authority.
8. Reconnect never preempts a genuinely live Controller.
9. No detach/reconnect operation synchronously gates PTY → VT → canonical state → damage progress.
10. No persistence journal/database is required to keep the M001 live PTY alive.
11. If Pass 8 Block metadata is present, reconnect does not create a new Block merely because presentation disappeared or reappeared.
12. Seyal OSS remains independent of commercial code.

## 3. Scope

Pass 9 covers:

- normal terminal-window close/detach;
- GUI application termination;
- abrupt GUI process death/crash;
- Runtime connection-loss detection and idempotent cleanup;
- controller-lease release and safe reacquisition;
- same-Runtime/same-execution identity proof;
- current-state display reconstruction through SPEC-004 Candidate D;
- Pass 7 input/resize usability after reattach;
- Pass 8 Block identity/state continuity where applicable;
- detached live output and detached execution-exit behavior;
- repeated lifecycle/resource/security/failure tests;
- controlled reconnect performance/resource evidence.

## 4. Explicit non-goals

Pass 9 does not implement or claim:

- Runtime-crash PTY survival, PTY keeper/worker supervision or Runtime restart continuity of live OS resources;
- reboot/login-session recovery;
- durable terminal history, transcript persistence or million-line scrollback;
- durable layout/tab/split/workspace presentation restore;
- journal-based restoration of a dead PTY;
- remote/network reconnect;
- cross-device continuation;
- M002 terminal compatibility expansion;
- tabs/splits/rich workspace navigation;
- agent/cloud/mobile/commercial behavior.

A later durable metadata store may remember records after Runtime death, but it must never claim that such records resurrect the old live PTY.

## 5. Identity and authority contract

### 5.1 Surviving Runtime

For the successful M001 Pass 9 continuity path:

```text
RuntimeId(before GUI loss) == RuntimeId(after GUI reopen)
ExecutionId(before GUI loss) == ExecutionId(after GUI reopen)
AttachmentId(before GUI loss) != AttachmentId(after GUI reopen)
```

The Runtime process/incarnation remains live while the GUI is absent.

If the observed `RuntimeId` changes, the client must discard all old connection/attachment/controller/request reconciliation state. It must not claim that the old live PTY survived merely because durable or caller-supplied IDs exist.

### 5.2 Attachment/controller identity

Disconnect or successful `Detach` revokes the old attachment and Controller lease before those identities can be reused. A later connection starts a fresh connection-local request-ID space as already required by SPEC-004/006.

Old `AttachmentId`, `ResizeRequest.request_id`, applied-generation fence or queued client input state is never transferred into the new connection.

### 5.3 Workspace/Block continuity

Once SPEC-007 is accepted and implemented, a surviving live execution retains the same owning Workspace association and the same M001 `BlockId`/logical start anchor/state across detach/reconnect.

Presentation loss is not a Block lifecycle transition. Reattach is not a new execution or new Block.

## 6. Normal detach semantics

Closing the M001 terminal window or terminating Seyal.app must make that presentation stop owning Controller authority.

Required sequence is logically:

```text
stop accepting new native terminal input for the closing surface
→ discard/cancel ephemeral IME composition without emitting marked text
→ best-effort bounded Detach/Goodbye when the connection is healthy
→ close/release the client connection/attachment
→ release disposable display/resize/client queues
→ release renderer/surface-dedicated GPU resources per SPEC-005
```

Correctness does not depend on receiving `Detached`, `Goodbye` or any other acknowledgement. App/window shutdown must not block indefinitely waiting for Runtime acknowledgement.

Normal presentation close must not call terminal-execution termination and must not ask Runtime to exit merely because no GUI remains.

If the macOS process remains alive after its last terminal window closes, it must not retain a hidden Controller attachment solely because the process is still alive.

## 7. Abrupt GUI death semantics

`SIGKILL`, crash, forced termination, socket reset/EOF or equivalent client disappearance cannot execute a graceful detach handshake. Runtime therefore owns correctness.

On detected connection loss Runtime must, idempotently and in bounded work:

1. revoke every attachment bound to that connection;
2. release its Controller lease before a future attachment can mutate;
3. discard unsent per-client presentation/control state and per-client resource accounting;
4. retain the live `TerminalExecution`, PTY, child, canonical `TerminalState`, Runtime/workspace metadata and other clients;
5. continue terminal output, child-exit observation and explicit execution lifecycle independently.

Repeated cleanup signals or partially completed cleanup must not double-free, double-release or terminate the execution.

## 8. Reconnect state machine

A reopened/recreated M001 surface follows:

```text
Disconnected
→ DiscoverRuntime
→ Connect
→ Hello/RuntimeId validation
→ ResolveExecution
→ AttachController
→ AwaitCurrentState
→ CommitDisposableDisplayState
→ RendererReady
→ Usable
```

Failure at any stage returns to a bounded disconnected/recovery state. It must not create a second VT/grid or replay PTY bytes.

### 8.1 Runtime discovery

If the existing per-user Runtime is alive, the client connects to that singleton endpoint and validates the same-user/security rules from SPEC-004.

A new GUI must not launch a competing Runtime merely because its first connection attempt races an already-live Runtime startup/discovery path. Existing singleton/stale-socket authority remains with Runtime.

### 8.2 Resolving the target execution

M001 has only one product terminal surface and does not introduce durable layout/navigation persistence.

For the Pass 9 user-visible proof, the production path must have exactly one eligible surviving interactive execution and reattach to that `ExecutionId`.

If multiple eligible executions exist, the client must not guess, terminate extras or silently select by unstable list order. A caller/test may provide an explicit `ExecutionId`; richer multi-execution selection belongs to later workspace/tab UI.

If zero eligible executions survive, reconnect continuity is not claimed. Normal creation of a new execution is a separate lifecycle path and receives a new `ExecutionId`.

### 8.3 Controller reacquisition race

A new client may arrive before Runtime has processed EOF/reset for the prior crashed Controller. In that narrow race `ControllerBusy` is allowed.

The client must:

- surface/retain a non-secret non-usable state while Controller authority is absent;
- never buffer unbounded user input while waiting;
- never silently route terminal input as Observer;
- retry only through a bounded reconnect policy with no busy polling.

M001 policy: at most 6 automatic Controller-attach attempts for this cleanup race, using monotonic backoff delays of at least `10, 20, 40, 80, 160, 250 ms` between attempts. Any success cancels remaining retries. Exhaustion stops automatic retry and exposes a recoverable non-secret state; a later explicit reconnect/reopen may try again.

A genuinely live existing Controller is never preempted.

## 9. Current-state reconstruction

Successful attach queues authoritative current display state as defined by SPEC-004. The client creates a fresh disposable display cache and applies a complete valid current snapshot before the surface becomes `Usable`.

Rules:

- no PTY byte history replay;
- no GUI VT/parser/grid authority;
- no reuse of stale committed display generation from the prior process/connection;
- no reuse of prior `appliedAwaitingProjection` resize fence;
- no reuse of old IME composition or accepted-but-unwritten input queue;
- renderer resources may be rebuilt/repopulated from committed disposable display state;
- current-state generation and subsequent deltas follow ordinary SPEC-004 continuity/resync rules.

A reconnecting client may show bounded reconnect/empty/loading chrome before the first authoritative current snapshot commits, but it must not display stale terminal content as if current.

## 10. Detached execution behavior

### 10.1 Output while detached

A live execution continues reading PTY output and mutating its canonical `TerminalState` while no GUI is attached.

On reattach the client receives the current authoritative M001 state. Pass 9 does not promise durable capture of every line emitted while detached beyond the current bounded terminal/history contract.

The acceptance fixture must prove forward progress while detached with output generated after GUI disappearance and observable current state after reconnect.

### 10.2 Input while detached

No GUI attachment means no GUI Controller input. Runtime does not invent input or replay client input that had not been successfully admitted before connection loss.

Input actions still sitting only in a dead client's local accepted-but-unwritten queue are lost with that client and must never be silently retried after reconnect.

### 10.3 Execution exit while detached

If the child exits while GUI is detached, Runtime performs the ordinary final-drain/lifecycle path. Pass 8 final Block ordering, if present, remains authoritative.

Reopening must not resurrect the dead PTY or create a replacement under the old `ExecutionId`.

If the finalized execution has already been retired from the M001 registry, it is absent from live execution resolution. Durable historical records are later scope.

## 11. Runtime failure boundary

M001 continuity requires the Runtime incarnation itself to survive GUI loss.

If Runtime crashes or is terminated:

```text
old RuntimeId ends
→ old live PTY continuity is not claimed by M001
→ old client attachment/controller identities are invalid
→ any later replacement execution uses a new live execution lifetime
```

No journal, Block record, display snapshot or renderer cache may be presented as restoration of a live PTY.

Runtime-crash live-PTY preservation requires a separately accepted future supervisor/keeper architecture and failure model.

## 12. Security and privacy

Reconnect preserves all SPEC-004/006 local peer and authority checks.

Required regressions:

- stale old `AttachmentId` cannot input/resize/detach the new attachment;
- a second same-UID client cannot preempt a live Controller;
- malformed reconnect/attach frames are bounded and cannot retain leaked Controller state;
- old connection-local resize request IDs cannot correlate against the new connection;
- reconnect/failure logs contain no terminal contents, input bytes, marked text, cwd/environment or secrets;
- crash cleanup does not weaken same-UID endpoint validation or attachment authorization;
- RuntimeId mismatch invalidates all cached authority/reconciliation state before mutation.

## 13. Resource and hot-path constraints

Detach/reconnect is control/lifecycle work, not terminal hot-path work.

Forbidden:

- per-execution or per-client polling threads/timers that remain active while detached;
- synchronous persistence/agent/cloud/licensing work on reconnect;
- unbounded retry/input/presentation queues;
- copied terminal transcript/grid retained solely to support reconnect;
- hidden renderer/GPU allocations retained after final surface detach contrary to SPEC-005;
- terminal progress waiting for a GUI reconnect or acknowledgement.

Runtime retains only the live execution state already required without a GUI plus bounded Runtime/workspace metadata.

## 14. Required tests and failure injection

Implementation is TDD/evidence-first. At minimum:

### Identity/lifecycle

- normal window close keeps Runtime/PTY/child/`ExecutionId` alive;
- app quit keeps the same live execution alive;
- GUI `SIGKILL`/forced crash keeps the same live execution alive;
- `RuntimeId` is unchanged across surviving-Runtime reconnect;
- new `AttachmentId` is different and old identity is rejected;
- same `ExecutionId` resumes input and resize after reattach;
- if Pass 8 is active, same Block identity/anchor/state survives detach/reconnect.

### Current-state/recovery

- output produced while detached is reflected in current state after reconnect;
- initial reconnect snapshot is authoritative before `Usable`;
- generation gap during reconnect takes ordinary bounded resync path;
- stale old display cache/resize fence/IME/input queue is never reused;
- client crash during attach and during multi-chunk snapshot leaves Runtime/execution healthy;
- reconnect after snapshot decode failure recovers through fresh bounded attach/resync, not PTY replay.

### Controller races

- old connection EOF cleanup releases Controller exactly once;
- new attach arriving before cleanup sees `ControllerBusy` rather than preemption;
- bounded retry schedule succeeds when cleanup completes;
- persistent genuine Controller occupancy exhausts bounded retries without spin or hidden input buffering.

### Detached terminal outcomes

- child continues running/outputting with zero GUI attachments;
- child exits while detached and final drain/lifecycle complete correctly;
- dead/retired execution is never resurrected/reused on reopen;
- explicit execution termination remains distinct from detach.

### Resource/failure

- at least 100 graceful detach/reattach cycles;
- at least 100 abrupt GUI/client-loss/reconnect cycles in deterministic harnesses where process launch cost is excluded;
- no leaked Runtime attachment/controller records, sockets/fds, renderer resources or unbounded client allocations;
- repeated malformed/stale reconnect attempts remain bounded;
- shutdown/cleanup remains correct when disconnect occurs during input backpressure, resize outstanding state, display chunking and Block finalization.

### Native end-to-end

On macOS, demonstrate using the production app/runtime path:

1. launch real shell;
2. run a long-lived fixture that changes visible state;
3. close the terminal window and verify shell PID/PTy/runtime remain;
4. reopen and interact with the same execution;
5. repeat with abrupt GUI process kill;
6. verify input, Control-C and resize after reconnect;
7. verify one accepted alternate-screen fixture can detach/reconnect without creating another PTY/VT authority.

## 15. Performance and measurement contract

Pass 9 must preserve Pass 8's final exact-head controlled baseline once Pass 8 exits. The implementation Issue records those exact SHAs/numbers before coding becomes Ready.

Controlled Apple-Silicon measurements must report at least:

- Runtime disconnect-detection → attachment/controller cleanup latency;
- local socket connect/hello/list/attach → complete authoritative current snapshot committed in client state;
- committed current client state → first renderer-ready update;
- repeated-cycle CPU/RSS/fd/attachment/resource counts;
- idle detached Runtime CPU with live execution;
- Pass 8 input/output/render p50/p95/p99 comparison after reconnect work lands.

M001 Pass 9 targets on the controlled M5 Pro class are:

- disconnect event dispatch → Controller/attachment cleanup p99 `<= 10 ms`;
- warm local reconnect from established client process start of socket connect → complete current-state client commit p99 `<= 25 ms` for 120x40 single-execution state;
- no persistent timer/poll wake while detached or connected-idle;
- zero leaked attachment/controller/fd/resource counters after deterministic lifecycle cycles;
- after 100 reconnect/crash cycles, retained process RSS must not show unbounded/monotonic growth; any final steady-state increase above `4 MiB` versus the same-process pre-cycle baseline is blocking unless allocator behavior is independently measured and explicitly accepted;
- Pass 8 final controlled input/output/render p99 movement `>5%` requires root-cause explanation; `>10%` is blocking absent explicit re-review.

Full macOS app process-launch time is recorded separately because OS launch services/cache state adds noise. It is user-visible evidence but is not substituted for the deterministic Runtime/client reconnect boundary above.

## 16. Acceptance criteria

Pass 9 production implementation is complete only when all are true:

- closing the M001 terminal presentation leaves the same Runtime and live execution running;
- abruptly killing/crashing Seyal.app leaves the same Runtime and live execution running;
- reopening observes the same `RuntimeId` and reattaches the same `ExecutionId` with a fresh `AttachmentId`;
- old controller/attachment/request identities are rejected;
- current authoritative display state is reconstructed without PTY replay or another VT/grid;
- terminal input/Control-C and authoritative resize work after reconnect;
- detached output advances canonical state and is reflected within M001 current-state/history limits after reconnect;
- detached child exit is handled honestly and never resurrected;
- Pass 8 Block identity/lifecycle continuity is preserved when that capability is active;
- repeated graceful/crash cycles have bounded resource behavior and no orphaned Controller;
- security/privacy regressions pass;
- controlled performance gates pass;
- `make bootstrap`, `make build`, `make test`, `make check`, `make bench` are green on final head;
- native clean-checkout demo succeeds on the permanent production path;
- independent architecture/security/performance review has no unresolved blocker.

## 17. Dependency and readiness rule

This specification may be reviewed while Pass 7/8 implementation work is still incomplete because it preserves already accepted lifetime/authority architecture.

Production Pass 9 must remain `NOT_READY` until:

1. Pass 7 implementation is merged with all exit evidence;
2. SPEC-007 is finally accepted and Pass 8 implementation is merged with all exit evidence;
3. current master is revalidated for actual Runtime/client/native/Block seams;
4. this SPEC-008 is independently accepted and merged;
5. a separate Pass 9 implementation Issue records exact dependency SHAs and the final controlled Pass 8 benchmark/resource baseline.

If final Pass 8 implementation changes an assumption used here, amend and re-review this specification before Pass 9 coding starts.
