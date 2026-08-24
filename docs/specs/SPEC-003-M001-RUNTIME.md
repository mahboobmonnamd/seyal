# SPEC-003 — M001 headless Runtime and multi-execution supervision

- **Status:** Active when ADR-006 is accepted/merged
- **Date:** 2026-08-24
- **Issue:** #70
- **Architecture:** Foundation Architecture + ADR-005 + ADR-006

## 1. Purpose

Define the observable M001 Pass 4 contract for Seyal's first headless Runtime: one Runtime authority supervising multiple `TerminalExecution`s independently of GUI lifetime, with bounded readiness, lifecycle, input and termination behavior.

This specification does not define the Pass 5 local transport/projection protocol or Pass 9 full GUI-crash/reconnect validation.

## 2. Ownership invariants

1. The Runtime owns the live execution registry.
2. Each registry entry owns exactly one `TerminalExecution`.
3. Each `TerminalExecution` continues to own exactly one PTY endpoint/child lifecycle and one authoritative `TerminalState`.
4. Reactor registration never becomes PTY ownership.
5. GUI/client attachment objects never own the PTY, child or terminal state.
6. Zero attachments does not terminate a live execution.
7. No renderer, Block, agent, persistence, cloud, licensing or commercial feature participates synchronously in terminal byte progress.

## 3. Runtime identity and execution identity

The Runtime has a stable `RuntimeId` for its current process/runtime lifetime.

Every created execution has an `ExecutionId` that:

- is unique within the Runtime lifetime;
- is never reused during that lifetime after an execution is removed;
- remains stable while attachments come and go;
- is distinct from OS PID, PTY FD, reactor token and viewport/pane identity.

This specification does not require identity secrecy and does not define the final persistence/restart identity format.

## 4. Headless lifecycle

M001 Pass 4 introduces a real headless Runtime composition root that can launch without the macOS GUI and remain alive while no GUI is attached.

A binary/process name may be established by the implementation Issue, but the product contract is behavioral: the Runtime is a separate headless-capable process boundary, not code hosted inside `Seyal.app`.

Pass 4 must demonstrate:

- Runtime process startup without the GUI;
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
- observe terminal child exit and remove/reap execution according to lifecycle policy.

The registry has a configured/testable maximum live-execution bound. Creation beyond the bound fails explicitly without partially creating an untracked PTY or child.

## 6. Logical attachment semantics

Pass 4 attachment is a Runtime-owned logical reference, not a network/socket/projection protocol.

- `attach` does not create a PTY, parser or grid.
- `detach` removes only the logical attachment reference.
- dropping/detaching the last attachment leaves the execution alive.
- attachment count/state cannot become the terminal-state authority.
- transport authentication, controller/observer protocol roles and shared projection are Pass 5.

## 7. Reactor registration

On macOS, each live execution is registered with the ADR-006 `ExecutionReactor`.

Registration provides an opaque generation-bearing token. The token is Runtime/reactor routing metadata only and is not exposed as the execution's durable identity.

A successful create transaction is:

```text
validate Runtime capacity + command + window size
→ create TerminalExecution
→ register PTY/child readiness
→ insert/publish registry entry
```

If registration or registry insertion fails, the operation must roll back safely and must not leave an untracked live execution.

Removal performs deregistration before the execution owner is destroyed.

## 8. Output/read fairness

A readable PTY event drives the existing nonblocking `TerminalExecution::read_output` path.

For one dispatch:

- bytes are read into a reusable bounded buffer;
- successful bytes feed the same authoritative `TerminalState` synchronously;
- reading stops on `WouldBlock`, EOF/HUP, or the Runtime's per-dispatch byte/work quantum;
- after the quantum is consumed the Runtime returns to the shared event loop before servicing more of that execution.

One continuously producing execution must not starve another ready execution or Runtime control work.

No per-byte heap allocation, JSON, serialization, renderer acknowledgement or cross-language callback is permitted in this path.

## 9. Input and writable readiness

The Runtime maintains a bounded FIFO input queue per execution.

Submission behavior:

1. attempt direct nonblocking PTY write where practical;
2. preserve unwritten bytes in order;
3. arm writable readiness only while pending bytes remain;
4. on writable readiness continue bounded draining;
5. disarm writable readiness when the queue becomes empty.

If accepting new bytes would exceed the configured per-execution queue bound, the Runtime returns explicit backpressure/error. It does not block the shared reactor and does not discard/reorder already accepted input.

## 10. Primary child exit

The macOS reactor reports primary-child exit independently of PTY EOF/HUP.

On exit readiness the Runtime calls the existing safe child-lifecycle authority to obtain/reap `ChildExit`.

Requirements:

- normal exit and signal exit remain distinct;
- reap remains idempotent;
- an exited primary child is detected even when descendants still hold the PTY slave open;
- a stale process event cannot be applied to a later execution after PID reuse;
- removal does not signal a process that has already been reaped.

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

- the owned process group is verified before each signal, preserving ADR-005;
- SIGTERM is sent first;
- SIGKILL is sent only after the graceful deadline expires;
- the shared Runtime reactor thread does not sleep/wait for either deadline;
- nearest pending deadline bounds the next reactor wait;
- no signal is sent after reap;
- a termination timeout/failure is observable and does not silently remove ownership bookkeeping.

## 12. Wake/control behavior

The Runtime can wake a blocking reactor wait for local control/shutdown work without polling sleeps and without one wake thread per execution.

On macOS the M001 mechanism is the ADR-006 kernel user event.

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

## 15. Failure behavior

- Reactor registration failure leaves no published partial execution.
- A stale/unknown reactor token is ignored and diagnosed safely.
- PTY read error is isolated to the affected execution unless it indicates a Runtime-wide reactor failure.
- One execution's full input queue cannot stall other execution output.
- One continuously ready PTY cannot monopolize the event loop indefinitely.
- Runtime-wide reactor failure must surface as a Runtime failure; it must not move terminal ownership into the GUI.
- Runtime process crash survival of arbitrary live PTYs remains out of M001.

## 16. Resource and performance evidence

Pass 4 is not complete without reproducible measurements, including environment metadata, for:

- Runtime startup;
- idle CPU with 1/10/50/100 live idle executions;
- Runtime RSS with 1/10/50/100 live idle executions;
- Runtime thread count with 1/10/50/100 live idle executions;
- one hot-output execution plus other ready/idle executions, proving fairness;
- PTY-ready event → committed `TerminalState` generation latency;
- bounded input queue behavior;
- repeated create/register/remove cycles and descriptor/registration leak checks.

Long-term architecture targets remain targets until measured; Pass 4 must not relabel them as achieved.

## 17. Required tests

At minimum:

1. Runtime launches headlessly with no GUI dependency.
2. Create/list returns stable unique `ExecutionId`s.
3. Capacity limit rejects excess creation without leaks.
4. Last logical attachment detaches while execution remains alive.
5. Multiple PTYs make progress on one Runtime reactor without thread-per-PTY.
6. A bursty/hot PTY cannot starve another ready PTY.
7. Pending input drains after writable readiness and writable interest becomes inactive when empty.
8. Input queue bound produces explicit backpressure without corrupting accepted order.
9. Primary child exit is observed/reaped even if a descendant retains the slave.
10. SIGTERM → deadline → SIGKILL termination progresses without blocking unrelated execution output.
11. Repeated create/remove does not accumulate descriptors, registrations or zombies.
12. Stale registration generation cannot target a new execution after FD/PID reuse.
13. No RILL identifiers/daemon/socket coupling are introduced.
14. No GUI/Swift dependency enters Runtime or PTY/VT byte progress.

## 18. Explicitly deferred

- Pass 5 versioned Unix-domain control/attachment transport;
- local shared-memory/display projection selection and security review;
- controller/observer protocol authorization beyond the logical Pass 4 seam;
- Metal rendering/input wiring;
- Block timeline implementation;
- complete GUI crash/reconnect validation (Pass 9);
- Runtime-crash/reboot PTY survival;
- Linux/Windows Runtime reactor implementation;
- remote/cloud execution;
- commercial code.

## 19. Definition of Done for Pass 4

Pass 4 is complete only when the implementation is:

```text
working
+ tested
+ headlessly demonstrable
+ measured/benchmarked where required
```

and all repository/architecture/security gates required by the owning implementation Issue are green.