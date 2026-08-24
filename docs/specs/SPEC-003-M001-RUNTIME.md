# SPEC-003 — M001 headless Runtime and multi-execution supervision

- **Status:** Active when ADR-006 is accepted/merged
- **Date:** 2026-08-24
- **Issue:** #70
- **Architecture:** Foundation Architecture + ADR-005 + ADR-006

## 1. Purpose

Define the observable M001 Pass 4 contract for Seyal's first headless Runtime: one per-user Runtime authority supervising multiple `TerminalExecution`s independently of GUI lifetime, with bounded readiness, lifecycle, input and termination behavior.

This specification does not define the Pass 5 local transport/projection protocol or Pass 9 full GUI-crash/reconnect validation.

## 2. Ownership invariants

1. The Runtime owns the live execution registry.
2. Each registry entry owns exactly one `TerminalExecution`.
3. Each `TerminalExecution` continues to own exactly one PTY endpoint/child lifecycle and one authoritative `TerminalState`.
4. Reactor registration never becomes PTY ownership.
5. GUI/client attachment objects never own the PTY, child or terminal state.
6. Zero attachments does not terminate a live execution.
7. Only the Runtime reactor owner mutates a live `TerminalExecution`; control producers enqueue bounded typed work and wake the reactor instead of touching PTY/VT state concurrently.
8. No renderer, Block, agent, persistence, cloud, licensing or commercial feature participates synchronously in terminal byte progress.

## 3. Runtime identity and execution identity

The Runtime has a stable `RuntimeId` for its current process/runtime lifetime.

Every created execution has an `ExecutionId` that:

- is unique within the Runtime lifetime;
- is never reused during that lifetime after an execution is removed;
- remains stable while attachments come and go;
- is distinct from OS PID, PTY FD, reactor registration token and viewport/pane identity.

Reactor registration tokens are allocated by `ExecutionReactor`, carry a reuse generation, and are never caller-selected durable identities.

This specification does not require identity secrecy and does not define the final persistence/restart identity format.

## 4. Headless lifecycle and per-user singleton

M001 Pass 4 introduces a real headless Runtime composition root that can launch without the macOS GUI and remain alive while no GUI is attached.

A binary/process name may be established by the implementation Issue, but the product contract is behavioral: the Runtime is a separate headless-capable process boundary, not code hosted inside `Seyal.app`.

The accepted architecture is one active local Runtime authority per logged-in user scope. Pass 4 must therefore provide a minimal interprocess singleton guard independent of the future Pass 5 command transport. Starting a second Runtime for the same user scope fails explicitly and must not affect the already-running Runtime or its executions. The singleton guard is not an authentication credential or the future Runtime discovery protocol.

Pass 4 must demonstrate:

- Runtime process startup without the GUI;
- only one active Runtime authority for the same user scope;
- stable Runtime identity while the process is alive;
- execution creation and supervision inside the Runtime owner;
- one execution continuing to produce/accept terminal I/O while it has zero logical attachments;
- Runtime shutdown/failure is distinct from GUI detach.

M001 does not claim that a Runtime crash preserves arbitrary live PTYs.

## 5. Execution registry

The Runtime provides an internal typed API sufficient to exercise these operations before Pass 5 transport exists:

- create execution;
- list executions;
- obtain immutable execution/lifecycle summary by `ExecutionId`;
- create a logical attachment reference;
- detach an attachment reference;
- submit bounded input;
- request resize only where current M001 authority permits;
- request explicit termination;
- observe primary-child exit and finalize/remove the execution according to this specification.

The registry has a configured/testable maximum live-execution bound. Creation beyond the bound fails explicitly without partially creating an untracked PTY or child.

Registry insertion is the publication point: before insertion, a created execution is not externally observable and failures must roll it back completely.

## 6. Logical attachment semantics

Pass 4 attachment is a Runtime-owned logical reference, not a network/socket/projection protocol.

- `attach` does not create a PTY, parser or grid.
- `detach` removes only the logical attachment reference.
- dropping/detaching the last attachment leaves the execution alive while its primary child remains live.
- attachment count/state cannot become terminal-state or process-lifecycle authority.
- GUI close is logical detach, not Runtime shutdown and not execution termination.
- transport authentication, controller/observer protocol roles and shared projection are Pass 5.

## 7. Reactor registration and immediate-exit race

On macOS, each live execution is registered with the ADR-006 `ExecutionReactor`.

Registration returns an opaque generation-bearing token allocated by the reactor. The Runtime associates the token with `ExecutionId`; the token is routing metadata only and is not exposed as durable execution identity.

A successful create transaction is:

```text
validate Runtime capacity + command + window size
→ create TerminalExecution
→ register PTY/primary-child readiness and obtain RegistrationToken
→ immediately reconcile primary-child state with nonblocking try_wait
→ if still live, insert/publish registry entry
```

The immediate reconciliation is mandatory so a command that exits between spawn and reactor registration cannot be missed or remain stuck forever.

If registration, reconciliation or registry insertion fails, the operation must roll back safely: remove any partial filters, reap/finalize the created execution as required, close owned descriptors, and leave no untracked live execution.

Removal performs deregistration before the execution owner is destroyed. Stale events from an already-deregistered generation are ignored safely.

## 8. Output/read fairness

A readable PTY event drives the existing nonblocking `TerminalExecution::read_output` path on the Runtime reactor owner.

For one dispatch:

- bytes are read into a reusable bounded buffer;
- successful bytes feed the same authoritative `TerminalState` synchronously;
- reading stops on `WouldBlock`, EOF/HUP, or the Runtime's per-dispatch byte/work quantum;
- after the quantum is consumed the Runtime returns to the shared event loop before servicing more of that execution.

One continuously producing execution must not starve another ready execution or Runtime control work. If the quantum is reached while the level-triggered PTY remains readable, it remains eligible for a later dispatch.

No per-byte heap allocation, JSON, serialization, renderer acknowledgement or cross-language callback is permitted in this path. Reactor event/change buffers are bounded and reusable rather than allocated per wait/event.

## 9. Input, writable readiness and control ownership

The Runtime maintains a bounded FIFO input queue per execution and a bounded Runtime control queue.

Only the Runtime reactor owner performs PTY writes. A producer outside that owner may enqueue typed input/control work and trigger the Runtime wake event; it may not call `TerminalExecution` directly.

On the reactor owner, input handling:

1. attempts direct nonblocking PTY write;
2. preserves unwritten bytes in order;
3. arms writable readiness only while pending bytes remain;
4. on writable readiness continues bounded draining;
5. disarms writable readiness when the queue becomes empty.

If accepting new bytes would exceed the configured per-execution queue bound, the Runtime returns explicit backpressure/error. It does not block the shared reactor and does not discard/reorder already accepted input.

If the bounded Runtime control queue is full, new control work is rejected/backpressured explicitly rather than growing memory without bound. Wake notifications may coalesce; queue state, not wake-event count, is authoritative.

## 10. Primary child exit and execution finalization

The macOS reactor reports primary-child exit independently of PTY EOF/HUP.

On exit readiness the Runtime calls the existing safe child-lifecycle authority to obtain/reap `ChildExit`.

M001 defines primary-child exit as the execution's logical process-completion boundary. PTY EOF/HUP is not allowed to keep an execution alive indefinitely because descendants may retain a PTY slave after the primary child exits.

Finalization requirements:

1. classify/reap the primary child; normal exit and signal exit remain distinct;
2. perform a bounded final nonblocking drain of bytes already available on the PTY master so output queued before exit is committed to the authoritative `TerminalState`;
3. do not wait indefinitely for PTY EOF if descendants still hold the slave;
4. deregister PTY/process filters;
5. remove the registry entry;
6. drop `TerminalExecution`, closing the PTY master.

Output produced by arbitrary descendants after primary completion is not guaranteed by M001. A future product requirement to keep such a terminal execution alive would require a separate lifecycle decision.

A stale process event cannot be applied to a later execution after PID reuse, and removal/finalization never signals a primary process that has already been reaped.

## 11. Nonblocking termination state machine

Runtime termination is asynchronous with respect to the shared reactor loop.

The observable lifecycle is:

```text
Running
→ TerminatingGraceful
→ Exited
```

or, after the configured graceful deadline:

```text
TerminatingGraceful
→ TerminatingForced
→ Exited
```

or, if bounded forced reap does not complete:

```text
TerminatingForced
→ TerminationFailed
```

Requirements:

- the owned primary process group is verified before each signal, preserving ADR-005;
- SIGTERM is sent first;
- SIGKILL is sent only after the graceful deadline expires;
- the shared Runtime reactor thread does not sleep/wait for either deadline;
- nearest pending deadline bounds the next reactor wait;
- no signal is sent after primary reap;
- after primary reap, finalization follows §10, including bounded final PTY drain and master closure;
- a termination timeout/failure is observable and does not silently remove ownership bookkeeping;
- a shell job placed in a distinct job-control process group must not keep Runtime terminal registrations/resources alive after the primary execution is finalized.

Seyal does not enumerate arbitrary descendant PIDs/process groups as an unsafe substitute for PTY/session lifecycle. Processes intentionally detached from terminal semantics are outside the execution's direct process-group ownership.

## 12. Wake/control behavior

The Runtime can wake a blocking reactor wait for local control/shutdown work without polling sleeps and without one wake thread per execution.

On macOS the M001 mechanism is the ADR-006 kernel user event. Multiple triggers may coalesce; after waking, the reactor owner services bounded queued control work rather than assuming one wake event maps to one command.

Pass 5 may add the actual local command/attachment transport. This Pass 4 wake path is not itself a public protocol.

## 13. Resize interaction

Resize continues to obey Foundation §5.4 and ADR-005:

```text
validate authority/size
→ apply fallible PTY winsize
→ commit canonical TerminalState resize
→ expose resulting damage later
```

The shared reactor does not become resize authority. A resize operation cannot race with another owner mutating the same `TerminalExecution`; Pass 4 preserves single-owner execution mutation.

## 14. Concurrency model

M001 Pass 4 must not create:

- one thread per PTY;
- one daemon/process per execution;
- one async runtime/executor per execution;
- concurrent mutable owners for one `TerminalExecution`;
- an O(N) busy-spin over all PTYs as the idle scheduling mechanism.

The initial implementation uses one Runtime reactor owner for PTY/VT mutation plus only bounded supporting workers where separately justified.

If later measurements justify a small fixed number of I/O shards, each `TerminalExecution` still has exactly one owning shard and no duplicated terminal state.

## 15. Runtime-only descriptor hygiene

All Runtime-internal descriptors that must not be inherited by spawned commands are close-on-exec, including the kqueue descriptor and any singleton/wakeup-support descriptor introduced by Pass 4.

The implementation must test descriptor inheritance from a spawned shell/command. A child may inherit only descriptors intentionally part of its terminal execution contract.

## 16. Controlled Runtime shutdown

A controlled Runtime shutdown is not GUI detach.

When shutdown is explicitly requested, the Runtime stops accepting new executions/control work and drives every live execution through bounded termination/finalization using the same nonblocking reactor/deadline machinery. The event loop must continue servicing output/exit events while shutdown progresses.

If a bounded controlled shutdown cannot complete, the failure is reported; the implementation must not falsely report clean shutdown while live owned executions/registrations remain.

Abrupt Runtime crash/kill remains outside M001 survival guarantees.

## 17. Failure behavior

- Reactor registration failure leaves no published partial execution.
- Immediate child exit during create/register is reconciled and cannot become a stuck registry entry.
- A stale/unknown reactor token is ignored and diagnosed safely.
- PTY read error is isolated to the affected execution unless it indicates a Runtime-wide reactor failure.
- One execution's full input queue cannot stall other execution output.
- One continuously ready PTY cannot monopolize the event loop indefinitely.
- A full control queue produces explicit backpressure instead of unbounded allocation.
- Runtime-wide reactor failure must surface as a Runtime failure; it must not move terminal ownership into the GUI.
- Runtime process crash survival of arbitrary live PTYs remains out of M001.

## 18. Resource and performance evidence

Pass 4 is not complete without reproducible measurements, including environment metadata, for:

- Runtime startup;
- idle CPU with 1/10/50/100 live idle executions;
- Runtime RSS with 1/10/50/100 live idle executions;
- Runtime thread count with 1/10/50/100 live idle executions;
- one hot-output execution plus other ready/idle executions, proving fairness;
- PTY-ready event → committed `TerminalState` generation latency;
- bounded input/control queue behavior;
- repeated create/register/remove cycles and descriptor/registration leak checks.

Long-term architecture targets remain targets until measured; Pass 4 must not relabel them as achieved.

## 19. Required tests

At minimum:

1. Runtime launches headlessly with no GUI dependency.
2. A second Runtime for the same user scope is rejected without disturbing the first; a new Runtime can start after the first cleanly exits.
3. Create/list returns stable unique `ExecutionId`s.
4. Capacity limit rejects excess creation without leaks.
5. An immediately exiting command cannot be missed between spawn and reactor registration.
6. Last logical attachment detaches while a live primary execution remains alive.
7. Multiple PTYs make progress on one Runtime reactor without thread-per-PTY.
8. A bursty/hot PTY cannot starve another ready PTY.
9. Pending input drains after writable readiness and writable interest becomes inactive when empty.
10. Input/control queue bounds produce explicit backpressure without corrupting accepted order.
11. Primary child exit is observed/reaped even if a descendant retains the slave; final already-buffered output is committed and the Runtime does not wait forever for descendant EOF.
12. SIGTERM → deadline → SIGKILL termination progresses without blocking unrelated execution output.
13. A shell job in a distinct job-control process group cannot keep Runtime terminal resources/registrations alive after primary execution finalization.
14. Repeated create/remove does not accumulate descriptors, registrations or zombies.
15. Stale registration generation cannot target a new execution after FD/PID reuse.
16. Runtime-only descriptors, including kqueue/singleton descriptors, are not inherited by spawned commands.
17. Controlled Runtime shutdown progresses all live executions without blocking the reactor and does not report success with owned live resources remaining.
18. No RILL identifiers/daemon/socket coupling are introduced.
19. No GUI/Swift dependency enters Runtime or PTY/VT byte progress.

## 20. Explicitly deferred

- Pass 5 versioned Unix-domain control/attachment transport;
- local shared-memory/display projection selection and security review;
- controller/observer protocol authorization beyond the logical Pass 4 seam;
- Metal rendering/input wiring;
- Block timeline implementation;
- complete GUI crash/reconnect validation (Pass 9);
- Runtime-crash/reboot PTY survival;
- Linux/Windows Runtime reactor implementation;
- remote/cloud execution;
- preservation of a terminal execution after its primary command has exited;
- commercial code.

## 21. Definition of Done for Pass 4

Pass 4 is complete only when the implementation is:

```text
working
+ tested
+ headlessly demonstrable
+ measured/benchmarked where required
```

and all repository/architecture/security gates required by the owning implementation Issue are green.
