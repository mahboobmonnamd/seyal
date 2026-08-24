# ADR-006 — Bounded multi-execution Runtime reactor on macOS

- **Status:** Accepted for M001 when this change is merged
- **Date:** 2026-08-24
- **Issue:** #70
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

### 2. Raw descriptors stay encapsulated

The reactor implementation may register the PTY master descriptor and child PID internally because it lives inside `seyal-exec`, but public Runtime callers do not receive a raw descriptor or ownership-transferring handle.

Registration uses an opaque generation-bearing token supplied/associated by the Runtime. Events return that token and readiness kind, not a raw FD as identity. Registration generations prevent a stale queued event from being applied to a later execution after FD/PID reuse.

The required lifecycle is:

```text
create TerminalExecution
→ register reactor filters
→ publish execution in Runtime registry
→ service events
→ mark/remove execution from registry
→ deregister reactor filters
→ only then drop the TerminalExecution owner
```

Rollback must remove any partially installed registration if create/register fails.

### 3. Read progress is bounded and fair

A readable event causes the Runtime to call the existing nonblocking `TerminalExecution::read_output` path. That path still feeds bytes directly into the one authoritative `TerminalState`.

One ready execution may consume only a bounded byte/work quantum per dispatch before the loop returns to other ready events. The exact initial quantum is an implementation constant justified by tests/measurements, not a product protocol. Continuous output from one PTY must not starve unrelated executions or Runtime control events.

Level readiness must be drained until `WouldBlock` or the per-dispatch quantum is reached. No renderer/client acknowledgement may gate the next PTY read.

### 4. Writable readiness is armed only for queued input

The Runtime owns a bounded pending-input queue per execution. It first attempts nonblocking PTY writes immediately. If progress becomes partial or `WouldBlock`, writable interest is armed. When the queue becomes empty, writable interest is removed/disabled so idle PTYs do not create permanent writable wakeups.

Queue capacity is bounded. Queue-full is explicit backpressure to the caller/control plane; it must never block PTY output processing or grow memory without bound.

### 5. Child exit is an explicit reactor event

The macOS reactor registers the execution's primary child with `EVFILT_PROC` / `NOTE_EXIT` in addition to PTY readiness. On notification the Runtime uses the existing child lifecycle/`try_wait` authority to classify and reap the exit.

PTY EOF/HUP remains meaningful terminal-endpoint state but is not the sole source of primary-child exit truth; descendants can keep a PTY slave open after the primary child has exited.

### 6. Runtime wakeup is kernel-event based

The reactor reserves one user/control wake event (Darwin `EVFILT_USER`) so Runtime-local control/shutdown work can interrupt a blocking wait without polling sleeps or a permanent per-execution thread. Pass 5 may register its real local control transport with the same Runtime scheduling layer, but this ADR does not define that transport.

### 7. Termination becomes a nonblocking Runtime state machine

`TerminalExecution::terminate(policy)` is a valid bounded convenience operation outside the shared reactor loop, but Pass 4 must not call a sleep/wait loop on the single Runtime reactor thread. Doing so could freeze every other terminal for the grace interval.

Runtime-owned termination therefore progresses as states/deadlines:

```text
Running
→ TerminatingGraceful(deadline)   # verified owned pgrp receives SIGTERM
→ Exited                          # NOTE_EXIT / try_wait
or, when deadline expires
→ TerminatingForced(deadline)     # same verified owned pgrp receives SIGKILL
→ Exited                          # reap
or
→ TerminationFailed               # bounded reap deadline exceeded
```

`seyal-exec` may expose additional safe nonblocking signal primitives required by this state machine, while preserving ADR-005 process-group verification and never signaling after reap.

The reactor wait timeout is bounded by the nearest Runtime deadline; Seyal does not create one timer thread per execution.

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
- one continuously busy output execution alongside idle/interactive executions, proving bounded fairness;
- PTY-readable event → committed `TerminalState` generation latency;
- bounded pending-input behavior and writable-interest disablement after drain;
- create/register/remove cycles without FD/kqueue registration leakage;
- primary-child exit observation when descendants keep the PTY open;
- stale registration events cannot act on a replacement execution;
- Runtime control wakeup does not require polling sleeps.

These are baseline measurements, not claims that M001 already meets the long-term performance targets.

## Security and failure behavior

- Reactor tokens are routing identities, not authorization credentials.
- No public raw PTY descriptor is introduced.
- Malformed/stale registration events are ignored safely.
- Queue and registration counts are bounded by Runtime policy.
- A reactor failure is a Runtime-level failure and must not silently create a second terminal owner or hand PTYs to the GUI.
- Same-user local transport authentication/permissions remain Pass 5 responsibilities.
- Runtime crash survival of arbitrary live PTYs remains explicitly outside M001.

## Consequences

- Pass 4 can introduce the real `seyal-runtime` physical boundary without changing PTY/VT ownership.
- `seyal-exec` gains a small macOS-only readiness-composition seam, not a daemon/socket abstraction.
- The Runtime can supervise many executions with a constant/small bounded thread model.
- Blocking termination convenience APIs are kept out of the shared event-loop path.
- Future Linux/Windows reactor implementations are designed only when those Runtime platforms become active.

## Revisit conditions

Revisit this ADR if measured macOS evidence shows direct `kqueue` cannot meet correctness/fairness/performance requirements, if a second Runtime platform becomes active and a shared low-level reactor materially reduces risk/cost, or if process-lifecycle requirements change enough that `EVFILT_PROC` is no longer the appropriate macOS mechanism.

## External evidence reviewed

- Darwin/macOS `kqueue(2)` semantics for `EVFILT_PROC`, `NOTE_EXIT`, and `EVFILT_USER`/`NOTE_TRIGGER`.
- Mio 1.2.x documentation describing macOS `kqueue`, token-based readiness, `Waker`, and its low-level event-loop scope.

External documentation informs mechanism choice only; Seyal architecture/specifications remain normative.