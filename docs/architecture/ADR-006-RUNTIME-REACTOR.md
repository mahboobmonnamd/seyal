# ADR-006 — Bounded multi-execution Runtime reactor on macOS

- **Status:** Accepted for M001
- **Date:** 2026-08-24
- **Issue:** #70, hardened by #80
- **Scope:** M001 Pass 4 headless Runtime readiness, process-exit observation, bounded input/output progress, and termination scheduling

## Context

M001 Pass 3 established `TerminalExecution` as the owner of one PTY endpoint, one child lifecycle and one authoritative `TerminalState`. Its current `wait_readable` / `wait_writable` operations are correct for single-execution tests, but they are not themselves the Runtime concurrency architecture.

The accepted foundation requires one per-user headless Runtime supervising many executions without one thread/daemon per terminal, without busy scanning, and without exposing PTY ownership to the GUI or another crate. M001 also requires CPU/RSS/thread measurements at 1/10/50/100 executions.

RILL previously exposed a PTY `poll_with_extras` helper that coupled terminal readiness to daemon/socket polling. That design is evidence only and is not copied into Seyal.

## Decision

### 1. Use a Seyal-owned Darwin `kqueue` reactor for M001

On macOS, `seyal-exec` owns the smallest safe reactor capability required to compose many `TerminalExecution`s over one kernel event queue.

```text
Seyal Runtime
  ├─ execution registry
  │    ├─ ExecutionId A -> TerminalExecution
  │    ├─ ExecutionId B -> TerminalExecution
  │    └─ ...
  └─ ExecutionReactor
       └─ one Darwin kqueue
            ├─ PTY readable/hangup readiness
            ├─ PTY writable readiness only while pending input exists
            ├─ primary-child exit observation
            └─ Runtime wake/control event
```

`ExecutionReactor` is a readiness capability, not an execution owner. `TerminalExecution` remains the only owner that closes the PTY master or reaps/signals its child.

The kqueue descriptor is a Runtime-internal descriptor and must be close-on-exec. It must never leak into spawned shells/commands. Event/change buffers are bounded and reusable; the wait/dispatch path must not require heap allocation per event.

### 2. Raw descriptors and event identity stay encapsulated

The reactor implementation may register the PTY master descriptor and child PID internally because it lives inside `seyal-exec`, but public Runtime callers do not receive a raw descriptor or ownership-transferring handle.

The reactor allocates an opaque generation-bearing `RegistrationToken` and returns it to the Runtime for association with an `ExecutionId`. Runtime code does not choose or recycle the registration generation. Events return that token and readiness kind, not a raw FD or PID as identity. Registration generations prevent a stale queued event from being applied to a later execution after FD/PID reuse.

On Darwin, the `kevent.udata` field is only an event-routing carrier. Seyal encodes a by-value opaque integer/generation token (or an equivalent non-owning integer representation) in it. `udata` must never contain a pointer or reference to movable, freed, or Runtime-owned Rust storage. Kernel event delivery therefore cannot outlive a Rust object's address and turn a stale event into a dangling-pointer dereference.

The required lifecycle is:

```text
create TerminalExecution
→ register reactor filters and obtain RegistrationToken
→ reconcile immediate child exit with try_wait
→ publish execution in Runtime registry only if still live
→ service events
→ mark/remove execution from registry
→ deregister reactor filters idempotently
→ only then drop the TerminalExecution owner
```

The spawn-to-registration window is a correctness boundary: a command that exits immediately must not become a permanently registered or permanently unobserved execution. Registration followed by an immediate nonblocking child-state reconciliation closes that race. Rollback must remove any partially installed registration and explicitly finalize/reap the created execution if create/register/publish fails.

Deregistration is idempotent. Teardown can race with kernel-observed process/descriptor/filter disappearance; expected already-gone conditions are treated as successful cleanup (with diagnostics where useful), not promoted into a Runtime-wide failure. A queued event carrying an obsolete generation is ignored even if the numeric FD or PID has already been reused.

### 3. Read progress is bounded and fair

A readable event causes the Runtime reactor owner to call the existing nonblocking `TerminalExecution::read_output` path. That path still feeds bytes directly into the one authoritative `TerminalState`.

One ready execution may consume only a bounded read byte/work quantum per dispatch before the loop returns to other ready events. The exact initial quantum is an implementation constant justified by tests/measurements, not a product protocol. Continuous output from one PTY must not starve unrelated executions or Runtime control events.

Level readiness must be drained until `WouldBlock` or the per-dispatch read quantum is reached. If the quantum is reached while data remains readable, the level-triggered registration remains eligible for a subsequent dispatch. No renderer/client acknowledgement may gate the next PTY read.

### 4. Writable readiness, aggregate memory and write fairness

The Runtime owns a bounded pending-input queue per execution, but only the single Runtime reactor owner mutates a `TerminalExecution` or writes its PTY. Cross-thread/control producers, when they exist, enqueue bounded typed control/input work and trigger the Runtime wake event; they never call `TerminalExecution` concurrently.

Input memory is bounded at two levels:

```text
per-execution pending-input byte limit
+
Runtime-wide total pending-input byte limit across all executions
```

A submission is accepted only if both limits remain satisfied. This prevents 100 individually bounded terminals from multiplying into an unintended aggregate memory commitment. Queue-full/budget-full is explicit backpressure to the caller/control plane; already accepted bytes retain order and are not discarded.

On the reactor owner, input handling first attempts a nonblocking PTY write. If progress becomes partial or `WouldBlock`, unwritten bytes remain queued and writable interest is armed. When the queue becomes empty, writable interest is removed/disabled so idle PTYs do not create permanent writable wakeups.

Writable draining is also fair: one execution may consume only a bounded write byte/work quantum per dispatch before the loop returns to other ready PTYs and Runtime control work. A large queued paste/upload must not monopolize the event loop merely because its PTY remains writable. The exact initial write quantum is implementation evidence, not product protocol.

### 5. Child exit is an explicit reactor event and primary completion boundary

The macOS reactor registers the execution's primary child with `EVFILT_PROC` / `NOTE_EXIT` in addition to PTY readiness. On notification the Runtime uses the existing child lifecycle/`try_wait` authority to classify and reap the exit.

PTY EOF/HUP remains meaningful terminal-endpoint state but is not the sole source of primary-child exit truth; descendants can keep a PTY slave open after the primary child has exited. M001 therefore defines primary-child exit as the execution's logical process-completion boundary. The Runtime must not wait indefinitely for PTY EOF merely because a descendant retained the slave.

After reaping the primary child, the Runtime enters a short `DrainingAfterPrimaryExit` finalization state. It continues normal nonblocking/fair PTY reads across reactor dispatches until the first `WouldBlock`, EOF or HUP, so bytes already queued by the primary command are not truncated merely because they exceed one dispatch quantum. A configured/testable finalization deadline is also armed; continuous descendant output cannot extend the execution lifetime indefinitely. When drain completion or the deadline is reached, the Runtime deregisters the PTY/process filters and drops the `TerminalExecution`, closing the master.

Output produced by arbitrary descendants after primary completion is not a persistent-session contract in M001. This rule avoids both lost final output and ghost executions that remain forever because an unrelated descendant inherited the slave. A future requirement to keep an execution alive after its primary command has exited would require a separate lifecycle decision rather than treating PTY EOF as implicit authority.

### 6. Runtime wakeup is kernel-event based

The reactor reserves one user/control wake event (Darwin `EVFILT_USER`) so Runtime-local control/shutdown work can interrupt a blocking wait without polling sleeps or a permanent per-execution thread. The corresponding Runtime control queue is bounded. Multiple wake triggers may coalesce; the reactor owner therefore drains queued control work to its bounded fairness limit rather than assuming one wake event equals one command.

Pass 5 may register its real local control transport with the same Runtime scheduling layer, but this ADR does not define that transport.

### 7. Termination becomes a nonblocking Runtime state machine

`TerminalExecution::terminate(policy)` is a valid bounded convenience operation outside the shared reactor loop, but Pass 4 must not call a sleep/wait loop on the single Runtime reactor thread. Doing so could freeze every other terminal for the grace interval.

Runtime-owned termination therefore progresses as states/deadlines:

```text
Running
→ TerminatingGraceful(deadline)   # verified owned primary pgrp receives SIGTERM
→ DrainingAfterPrimaryExit(deadline)
or, when graceful deadline expires
→ TerminatingForced(deadline)     # same verified owned primary pgrp receives SIGKILL
→ DrainingAfterPrimaryExit(deadline)
or
→ TerminationFailed               # bounded reap deadline exceeded
```

`seyal-exec` may expose additional safe nonblocking signal primitives required by this state machine, while preserving ADR-005 process-group verification and never signaling after reap.

After primary reap, finalization follows §5. Seyal does not enumerate arbitrary descendant PIDs/process groups as a substitute for PTY/session semantics; implementation tests must prove that a shell job in a distinct job-control process group cannot keep Runtime terminal resources registered after primary completion/termination.

The reactor wait timeout is bounded by the nearest Runtime deadline, including post-exit finalization deadlines; Seyal does not create one timer thread per execution.

### 8. One Runtime loop, bounded supporting workers

M001 starts with one reactor/event-loop owner for terminal byte/state mutation. Background pools remain bounded and may be added only for work that cannot safely execute on the reactor owner. PTY/VT mutation itself is not moved to a thread-per-terminal model.

If measurements later demonstrate one event-loop shard cannot meet workload requirements, the architecture may add a small measured number of I/O shards. Sharding must preserve single ownership of each `TerminalExecution` and cannot create cross-shard duplicate terminal state.

## Alternatives reviewed

### A. Rebuild/scan a `pollfd[]` set

Rejected for the M001 permanent macOS Runtime. A retained poll array is simple, but each wake requires scanning readiness across the registered set and it does not naturally provide the same process-exit/user-wakeup capabilities. It is acceptable for Pass 3 single-execution helpers, not the many-execution Runtime architecture.

### B. Copy RILL `poll_with_extras`

Rejected. It makes the PTY abstraction aware of future daemon/socket composition and recreates RILL ownership coupling. Seyal keeps the reactor as an explicit execution-composition capability instead.

### C. Adopt Mio as the M001 reactor

Not selected now. Mio 1.2.x is a credible low-level readiness library: its documented macOS backend is `kqueue`, it uses token-based events and provides a cross-thread waker, and it states zero runtime allocations for its core event queue path. However, M001 needs direct child `EVFILT_PROC/NOTE_EXIT` semantics in addition to PTY readiness, and the current Seyal surface is macOS-only. Adding Mio would introduce another abstraction/dependency without removing the Darwin-specific process-lifecycle code that Seyal still must own.

Revisit Mio or another low-level reactor only when a second Runtime platform is active and evidence shows the shared dependency reduces code/risk without adding hot-path cost or weakening process-exit semantics.

### D. Tokio/async runtime or thread-per-PTY

Rejected for M001. Neither is justified by the current terminal/runtime requirements. A general async scheduler adds runtime/executor semantics and dependency weight; thread-per-PTY violates the explicit scaling model.

## Performance and validation requirements

Pass 4 implementation must record reproducible evidence for at least:

- idle Runtime CPU, RSS and thread count with 1, 10, 50 and 100 live idle executions;
- one continuously busy output execution alongside idle/interactive executions, proving bounded read fairness;
- one execution with a large pending writable backlog alongside readable/control work, proving bounded write fairness;
- PTY-readable event → committed `TerminalState` generation latency;
- per-execution and Runtime-wide aggregate pending-input backpressure;
- writable-interest disablement after drain;
- create/register/remove cycles without FD/kqueue registration leakage;
- idempotent removal when filters/processes/descriptors disappear during teardown;
- immediate-exit commands cannot be missed in the spawn/register race;
- primary-child exit is observed and final buffered output is preserved even when descendants keep the PTY slave open;
- continuous descendant output cannot extend post-primary-exit lifetime beyond the configured finalization bound;
- a distinct job-control process group cannot keep Runtime terminal resources registered after execution finalization;
- stale registration events cannot act on a replacement execution;
- event routing uses by-value tokens rather than Rust object pointers;
- Runtime control wakeup does not require polling sleeps;
- the kqueue descriptor and other Runtime-only descriptors are not inherited by spawned commands.

These are baseline measurements, not claims that M001 already meets the long-term performance targets.

## Security and failure behavior

- Reactor tokens are routing identities, not authorization credentials.
- Registration generations are allocated by the reactor and are not caller-selected/reused identity.
- Darwin `kevent.udata` carries only a by-value opaque token representation; it never points at movable/freed Rust storage.
- No public raw PTY descriptor is introduced.
- Runtime-only descriptors are close-on-exec.
- Malformed/stale registration events are ignored safely.
- Expected already-gone teardown conditions are idempotent cleanup, not Runtime-wide failure.
- Per-execution input, aggregate Runtime input, control queue and registration counts are bounded by Runtime policy.
- A reactor failure is a Runtime-level failure and must not silently create a second terminal owner or hand PTYs to the GUI.
- Same-user local transport authentication/permissions remain Pass 5 responsibilities.
- Runtime crash survival of arbitrary live PTYs remains explicitly outside M001.

## Consequences

- Pass 4 can introduce the real `seyal-runtime` physical boundary without changing PTY/VT ownership.
- `seyal-exec` gains a small macOS-only readiness-composition seam, not a daemon/socket abstraction.
- The Runtime can supervise many executions with a constant/small bounded thread model.
- Event delivery cannot depend on the memory address/lifetime of movable Rust registry entries.
- Per-terminal backpressure cannot multiply into unbounded aggregate pending-input memory.
- Both read and write progress have explicit dispatch fairness boundaries.
- Teardown races are handled idempotently instead of escalating normal kernel disappearance into Runtime failure.
- Blocking termination convenience APIs are kept out of the shared event-loop path.
- Primary-child exit cannot be confused with PTY EOF, and descendants cannot keep dead executions registered indefinitely.
- Final primary-command output is drained fairly without a one-shot quantum truncation rule.
- Future Linux/Windows reactor implementations are designed only when those Runtime platforms become active.

## Revisit conditions

Revisit this ADR if measured macOS evidence shows direct `kqueue` cannot meet correctness/fairness/performance requirements, if a second Runtime platform becomes active and a shared low-level reactor materially reduces risk/cost, if preserving terminal execution after primary-command exit becomes a product requirement, or if process-lifecycle requirements change enough that `EVFILT_PROC` is no longer the appropriate macOS mechanism.

## External evidence reviewed

- Darwin/macOS `kqueue(2)` semantics for `EVFILT_PROC`, `NOTE_EXIT`, `EVFILT_USER`/`NOTE_TRIGGER`, event identity and `udata` round-tripping.
- POSIX/macOS session and job-control semantics: `setsid()` creates one session/process group initially, while interactive shells may create distinct process groups for foreground/background jobs.
- Mio 1.2.x documentation describing macOS `kqueue`, token-based readiness, `Waker`, and its low-level event-loop scope.

External documentation informs mechanism choice only; Seyal architecture/specifications remain normative.
