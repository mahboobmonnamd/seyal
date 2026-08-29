# SPEC-009 — M001 detach, reconnect and GUI-crash survival

- **Status:** Proposed for M001 Pass 9 / Issue #717; refinement dependency gates through Pass 8 are satisfied. Merge/acceptance still requires explicit owner confirmation.
- **Date:** 2026-08-28
- **Reconciled:** 2026-08-29 against merged Pass 8 and current master
- **Issue:** #717
- **Architecture authority:** accepted Seyal foundation architecture, ADR-001/004/005/006/007
- **Depends on:** SPEC-002, SPEC-003, SPEC-004, SPEC-005, SPEC-006 and accepted SPEC-007
- **Pass 8 authority:** reviewed head `54b3a1748effc7c47c409d1f7cfdcbd547e8d1cc`, merged by PR #721 as `d9d21187e8429bbd3dbeb3e1c7cc4d05c1d147e6`
- **Numbering note:** SPEC-008 is already the active M003 command-Blocks/composer specification and is intentionally not a Pass 9 dependency.

## 1. Purpose

M001 Pass 9 proves that Seyal presentation/client lifetime is independent from a live terminal-execution lifetime on macOS.

Required proof:

```text
live Seyal.app surface
→ attached Controller for ExecutionId E
→ close/detach GUI or abruptly kill/crash GUI
→ Runtime stays alive
→ same PTY + child + canonical TerminalState for E stay alive
→ reopen Seyal.app
→ discover same RuntimeId
→ attach to E with a fresh AttachmentId
→ rebuild disposable client/display/renderer state from Runtime authority
→ resume rendering + input + resize
```

The proof uses the permanent Runtime, Candidate-D local transport, Pass 7 input/resize path, Pass 8 Block metadata seam and permanent Metal renderer. No temporary reconnect terminal engine or second VT/grid is permitted.

Runtime-crash live-PTY survival is explicitly outside M001 Pass 9.

## 2. Non-negotiable invariants

1. `TerminalExecution` remains sole owner of PTY, child lifecycle and canonical `TerminalState`.
2. Runtime remains the live authority while every GUI is absent.
3. GUI window/process, local connection, `AttachmentId`, client display cache, renderer resources, IME state, input queue and resize-reconciliation state are disposable.
4. Losing every GUI/client attachment must not terminate a live `TerminalExecution` or stop the per-user Runtime.
5. Reconnect reconstructs from current Runtime/canonical state; historical PTY bytes are never replayed into a client VT/parser.
6. One surviving Runtime incarnation retains one stable `RuntimeId`.
7. A surviving live execution retains the same `ExecutionId`; a replacement process after execution death receives a new `ExecutionId`.
8. Every reconnect receives a fresh `AttachmentId`; stale attachment/controller/request identities never regain authority.
9. Reconnect never preempts a genuinely live Controller.
10. Detach/reconnect work never synchronously gates PTY → VT → canonical state → damage progress.
11. Persistence/journaling is not responsible for keeping the M001 live PTY alive.
12. Pass 8 Workspace association, Block identity, logical anchor and state remain continuous for the surviving execution; presentation loss/reappearance is not a Block lifecycle transition.
13. Seyal OSS remains independent of commercial code.

## 3. Scope

Pass 9 covers:

- normal terminal-window close/detach;
- GUI application termination;
- abrupt GUI process death/crash;
- Runtime connection-loss detection and idempotent cleanup;
- Controller-lease release and safe reacquisition;
- same-Runtime/same-execution identity proof;
- current-state reconstruction through SPEC-004 Candidate D;
- Pass 7 input/resize usability after reattach;
- Pass 8 Block identity/state continuity;
- detached live output and detached execution-exit behavior;
- stale-authority and failure-injection coverage;
- repeated lifecycle/resource tests;
- controlled reconnect performance/resource evidence;
- native real-shell and minimal alternate-screen end-to-end proof.

## 4. Explicit non-goals

Pass 9 does not implement or claim:

- Runtime-crash PTY survival, PTY keeper/worker supervision or Runtime-restart continuity of live OS resources;
- reboot/login-session recovery;
- journal-based restoration of a dead PTY;
- durable terminal history, transcript persistence or million-line scrollback;
- durable layout/tab/split/workspace presentation restore;
- remote/network reconnect or cross-device continuation;
- M002 terminal-compatibility expansion;
- tabs/splits/rich workspace navigation;
- agent/cloud/mobile/commercial behavior.

A future durable metadata store may remember records after Runtime death, but it must never claim those records resurrect the old live PTY.

## 5. Identity and authority contract

For a successful continuity path:

```text
RuntimeId(before GUI loss) == RuntimeId(after GUI reopen)
ExecutionId(before GUI loss) == ExecutionId(after GUI reopen)
AttachmentId(before GUI loss) != AttachmentId(after GUI reopen)
```

If observed `RuntimeId` changes, the client must discard all old connection, attachment, Controller, request, display and resize-reconciliation state. It must not claim continuity of the old live PTY.

Disconnect or successful `Detach` revokes the old attachment and Controller lease. A later connection starts a fresh connection-local request-ID space as required by SPEC-004/006. Old `AttachmentId`, resize request IDs, applied-generation fences, IME state and accepted-but-unwritten local input are never transferred.

The surviving execution retains its Pass 8 Workspace association and M001 `BlockId`/logical start anchor/state until ordinary Block/execution lifecycle changes it. Detach or reattach alone does not create, complete or replace a Block.

## 6. Normal detach semantics

Closing the M001 terminal window or terminating Seyal.app must stop that presentation from owning Controller authority.

Logical shutdown sequence:

```text
stop accepting new native terminal input
→ cancel/discard ephemeral IME composition without emitting marked text
→ best-effort bounded Detach/Goodbye when healthy
→ close/release client connection and attachment
→ release disposable display/resize/input queues
→ release surface-dedicated renderer/GPU resources per SPEC-005
```

Correctness must not depend on a `Detached`, `Goodbye` or other acknowledgement. App/window shutdown must not block indefinitely waiting for Runtime response.

Normal presentation close must not terminate the `TerminalExecution` and must not stop Runtime merely because no GUI remains. If the macOS process remains alive after its final terminal window closes, it must not retain a hidden Controller lease.

## 7. Abrupt GUI death semantics

`SIGKILL`, crash, forced termination, socket EOF/reset or equivalent disappearance cannot perform a graceful handshake. Runtime therefore owns correctness.

On detected connection loss Runtime must, idempotently and in bounded work:

1. revoke every attachment bound to the dead connection;
2. release its Controller lease before a future attachment can mutate;
3. discard unsent per-client presentation/control state and per-client resource accounting;
4. retain the live `TerminalExecution`, PTY, child, canonical `TerminalState`, Runtime/workspace metadata and unrelated clients;
5. continue PTY output, child-exit observation and execution lifecycle independently.

Repeated or partially overlapping cleanup must not double-free, double-release or terminate the execution.

## 8. Reconnect state machine

A recreated M001 surface follows:

```text
Disconnected
→ DiscoverRuntime
→ Connect
→ Hello / RuntimeId validation
→ ResolveExecution
→ AttachController
→ AwaitCurrentState
→ CommitDisposableDisplayState
→ RendererReady
→ Usable
```

Failure at any stage returns to a bounded disconnected/recovery state. It must not create another VT/grid or replay PTY bytes.

### 8.1 Runtime discovery

If the existing per-user Runtime is alive, the client connects to that singleton endpoint and applies SPEC-004 same-user/security rules. A new GUI must not launch a competing Runtime merely because the first connection races startup/discovery or stale-socket cleanup.

### 8.2 Target execution resolution

M001 still has one product terminal surface and does not add durable workspace/layout navigation.

For the user-visible Pass 9 proof, exactly one eligible surviving interactive execution must be resolved automatically. If multiple eligible executions exist, the client must not guess, terminate extras or silently select by unstable list order. Tests/callers may specify an exact `ExecutionId`; richer selection belongs to later workspace UI.

If no eligible execution survives, continuity is not claimed. Creating a new execution is a separate path with a new `ExecutionId`.

### 8.3 Controller reacquisition race

A replacement GUI may arrive before Runtime processes EOF/reset for the dead Controller. In this narrow race `ControllerBusy` is valid.

The client must:

- remain non-usable for terminal mutation until Controller authority is obtained;
- never silently fall back to Observer input;
- never buffer unbounded user input;
- retry only through bounded non-spinning recovery.

M001 policy permits at most 6 automatic Controller-attach attempts with monotonic backoff delays of at least `10, 20, 40, 80, 160, 250 ms`. Success cancels remaining retries. Exhaustion stops automatic retry and exposes a recoverable non-secret state. A genuinely live existing Controller is never preempted.

## 9. Current-state reconstruction

Successful attach queues authoritative current display state as defined by SPEC-004. The new client builds a fresh disposable display cache and commits a complete valid current snapshot before the terminal surface becomes `Usable`.

Reconnect must not reuse:

- PTY byte history;
- a prior GUI VT/parser/grid;
- prior committed display generation as current authority;
- old `appliedAwaitingProjection` resize fences;
- old IME composition;
- old accepted-but-unwritten client input queue;
- stale renderer/GPU state as terminal truth.

Renderer resources may be rebuilt from newly committed disposable display state. Generation continuity and resync thereafter follow SPEC-004.

A reconnecting client may show bounded reconnect/loading chrome, but it must not display stale terminal content as if current.

## 10. Detached execution behavior

### 10.1 Output while detached

A live execution continues consuming PTY output and mutating canonical `TerminalState` while no GUI is attached. Reattach observes current authoritative state within M001's bounded screen/history contract; Pass 9 does not promise durable capture of every detached line.

### 10.2 Input while detached

With no GUI Controller, Runtime invents no input. Input that existed only in a dead client's local accepted-but-unwritten queue is lost with that client and must never be silently replayed after reconnect.

### 10.3 Execution exit while detached

If the child exits while detached, Runtime performs ordinary final drain and lifecycle completion. Pass 8 final display → Block Completed → Lifecycle Finalized ordering remains authoritative.

Reopening must not resurrect the dead PTY or create a replacement under the old `ExecutionId`. A retired finalized execution is absent from live resolution.

## 11. Runtime failure boundary

Pass 9 continuity requires the Runtime incarnation itself to survive GUI loss.

```text
Runtime dies
→ old RuntimeId ends
→ M001 does not claim old live-PTY continuity
→ old attachment/controller/request identities are invalid
→ later replacement execution has a new live execution lifetime
```

No journal, Block record, display snapshot or renderer cache may be presented as restoration of a live PTY. Runtime-crash survival requires a separately reviewed supervisor/keeper architecture.

## 12. Security and privacy

Required regressions include:

- stale old `AttachmentId` cannot input, resize or detach the new attachment;
- a second same-UID client cannot preempt a live Controller;
- malformed reconnect/attach frames are bounded and cannot retain leaked authority;
- old connection-local resize/request IDs cannot correlate against a new connection;
- RuntimeId mismatch invalidates cached authority/reconciliation state before mutation;
- reconnect/failure logs contain no terminal contents, input bytes, marked text, cwd, environment or secrets;
- crash cleanup does not weaken same-UID endpoint validation or attachment authorization.

## 13. Resource and hot-path constraints

Detach/reconnect is lifecycle/control work, never terminal hot-path work.

Forbidden:

- per-execution/per-client polling threads or timers that remain active while detached;
- synchronous persistence, agent, cloud, licensing or telemetry work on reconnect;
- unbounded retry/input/presentation queues;
- copied transcript/grid retained solely for reconnect;
- hidden renderer/GPU allocations retained after final surface detach contrary to SPEC-005;
- terminal progress waiting for GUI reconnect or acknowledgement.

Runtime retains only live execution state already required without a GUI plus bounded Runtime/workspace metadata.

## 14. Required tests and failure injection

### Identity/lifecycle

- normal window close keeps Runtime/PTY/child/`ExecutionId` alive;
- app quit keeps the same live execution alive;
- GUI `SIGKILL`/forced crash keeps the same live execution alive;
- `RuntimeId` remains unchanged across surviving-Runtime reconnect;
- new `AttachmentId` differs and old identity is rejected;
- same `ExecutionId` resumes input and resize after reattach;
- same Pass 8 Workspace/Block identity/anchor/state survives detach/reconnect.

### Current-state/recovery

- output generated while detached is reflected after reconnect;
- initial reconnect snapshot is authoritative before `Usable`;
- generation gaps use ordinary bounded resync;
- stale display cache/resize fence/IME/input state is never reused;
- client crash during attach or multi-chunk snapshot leaves Runtime/execution healthy;
- snapshot decode failure recovers through a fresh bounded attach/resync, never PTY replay.

### Controller races

- EOF cleanup releases Controller exactly once;
- new attach before cleanup observes `ControllerBusy`, not preemption;
- bounded retry succeeds when cleanup completes;
- genuine persistent Controller occupancy exhausts bounded retries without spin or hidden buffering.

### Detached outcomes

- child continues running/outputting with zero GUI attachments;
- child exit while detached completes final drain/lifecycle correctly;
- retired execution is never resurrected/reused;
- explicit terminate remains distinct from detach.

### Resource/failure

- at least 100 graceful detach/reattach cycles;
- at least 100 abrupt client-loss/reconnect cycles in deterministic harnesses excluding process-launch cost;
- zero leaked Runtime attachment/controller records, sockets/fds and renderer resources;
- malformed/stale reconnect attempts remain bounded;
- disconnect during input backpressure, outstanding resize, display chunking and Block finalization remains correct.

### Native end-to-end

On the production macOS path demonstrate:

1. launch a real shell and long-lived fixture that changes visible state;
2. close the terminal window and verify Runtime, shell PID and PTY remain live;
3. reopen and interact with the same execution;
4. repeat with abrupt GUI process kill;
5. verify normal input, Control-C and resize after reconnect;
6. verify one accepted alternate-screen fixture can detach/reconnect without another PTY/VT authority.

## 15. Performance and measurement contract

Pass 9 implementation must preserve the final Pass 8 controlled baseline retained by PR #721 and `docs/engineering/M001-PASS8-BLOCK-METADATA.md`. The production implementation Issue must record exact dependency SHAs and baseline values before implementation becomes Ready.

Controlled Apple-Silicon measurements must report:

- Runtime disconnect-event dispatch → attachment/controller cleanup;
- local connect/hello/resolve/attach → complete authoritative 120x40 current-state client commit;
- committed client state → first renderer-ready update;
- repeated-cycle CPU/RSS/fd/attachment/resource counts;
- idle detached Runtime CPU with a live execution;
- paired Pass 8 input/output/render/resize regression attribution after reconnect work lands.

Targets on the controlled M5 Pro class:

- disconnect event dispatch → Controller/attachment cleanup p99 `<= 10 ms`;
- warm local reconnect from socket-connect start → complete 120x40 current-state client commit p99 `<= 25 ms`;
- no persistent timer/poll wake while detached or connected-idle;
- zero leaked attachment/controller/fd/resource counters after deterministic lifecycle cycles;
- after 100 reconnect/crash cycles, final steady-state same-process RSS increase above `4 MiB` is blocking unless allocator behavior is independently measured and explicitly accepted;
- paired Pass 8 latency movement `>5%` requires root-cause explanation and `>10%` is blocking absent explicit re-review.

Full macOS app process-launch time is measured separately because launch-services/cache state is noisy; it cannot substitute for deterministic Runtime/client reconnect measurement.

## 16. Acceptance criteria

Pass 9 production implementation is complete only when all are true:

- graceful close leaves the same Runtime and live execution running;
- abrupt Seyal.app death leaves the same Runtime and live execution running;
- reopen observes same `RuntimeId`, same `ExecutionId`, fresh `AttachmentId`;
- stale controller/attachment/request identities are rejected;
- current authoritative display is reconstructed without PTY replay or another VT/grid;
- input, Control-C and authoritative resize work after reconnect;
- detached output advances canonical state and is reflected within M001 bounded-state limits;
- detached child exit is handled without resurrection;
- Pass 8 Workspace/Block continuity is preserved;
- repeated graceful/crash cycles are leak-free and bounded;
- security/privacy regressions pass;
- controlled performance gates pass;
- `make bootstrap`, `make build`, `make test`, `make check`, `make bench` are green on final head;
- native clean-checkout proof succeeds on the permanent production path;
- independent architecture/security/performance review has no unresolved blocker.

## 17. Refinement acceptance and Pass 9 readiness

The original refinement was authored before Pass 8 completion. Current authority has now been reconciled:

- Pass 7 implementation is merged and retained in current master;
- SPEC-007 is accepted;
- Pass 8 implementation PR #721 is independently review-green and merged;
- Pass 8 reviewed head is `54b3a1748effc7c47c409d1f7cfdcbd547e8d1cc`;
- Pass 8 merge commit is `d9d21187e8429bbd3dbeb3e1c7cc4d05c1d147e6`;
- this specification is renumbered to SPEC-009 because SPEC-008 is already active M003 command-Blocks authority.

Production Pass 9 remains `NOT_READY` until:

1. this SPEC-009 exact head is reviewed and explicitly accepted/merged;
2. current master post-Pass-8 validation has no unresolved blocker;
3. a separate Pass 9 production implementation Issue records exact dependency SHAs, the final Pass 8 baseline, implementation scope and required evidence.

No Pass 9 production code belongs in this refinement PR.