# Seyal Runtime-Crash Live PTY Survival R&D

**Document:** SEYAL-RUNTIME-CRASH-LIVE-PTY-RD-001  
**Date:** 2026-09-15  
**Issue:** #924  
**Status:** Active R&D spike; no production implementation authorized  
**Parents:** #641 persistence/recovery, #666 M004 market-ready v0.1

## 1. Question

Can Seyal survive a **Runtime coordinator crash/restart** while the same live terminal process continues, then safely re-adopt that execution without creating another PTY, shell, VT authority or terminal-state copy?

Target:

```text
Runtime R1
  -> execution worker E owns PTY + child + canonical TerminalState

R1 crashes
  -> E continues independently

Runtime R2 starts
  -> authenticates/fences/adopts E
  -> same ExecutionId + PTY + child continue
  -> clients reconnect through normal attachment/controller semantics
```

Machine reboot is explicitly different: the live process/PTY is gone. Reboot recovery may restore durable workspace/history metadata and optionally restart/reconcile work, but cannot resurrect the same live PTY.

## 2. Current-code audit

The current architecture cannot provide Runtime-crash live continuity without changing the process boundary.

### 2.1 Runtime owns `TerminalExecution` in-process

`crates/seyal-runtime/src/runtime/entry.rs` stores:

```text
Entry
  -> TerminalExecution
```

The Runtime registry therefore owns each live execution object by value in the Runtime process.

### 2.2 `TerminalExecution` owns both PTY endpoint and canonical terminal state

`crates/seyal-exec/src/execution.rs` stores:

```text
TerminalExecution
  -> TerminalEndpoint
  -> TerminalState
```

This is correct single-authority ownership, but it also means Runtime process death currently destroys the only owner of both the PTY master and canonical VT/state.

### 2.3 `TerminalEndpoint` owns the PTY master and child lifecycle

`crates/seyal-exec/src/endpoint.rs` stores:

```text
TerminalEndpoint
  -> master PTY handle
  -> ChildLifecycle
```

The macOS PTY setup marks descriptors `FD_CLOEXEC`, creates a new session with `setsid()`, and makes the slave the controlling terminal with `TIOCSCTTY`.

This means merely restarting the Runtime cannot reconstruct the same terminal. The PTY master fd and canonical `TerminalState` must remain owned by some surviving process for the terminal to stay live and semantically correct.

### 2.4 Existing continuity authority intentionally says Runtime death loses live PTY

`SEYAL-RUNTIME-WORKSPACE-CONTINUITY-RD-001` currently defines `RuntimeId` as one Runtime-process incarnation and says an execution whose Runtime died is not reconstructed as live. That remains correct **until** this spike proves a surviving execution owner and an accepted re-adoption protocol.

Therefore this work is not a reconnect patch. It is a process-ownership change requiring ADR/spec promotion if viable.

## 3. Key architectural conclusion from the audit

To survive coordinator death, **the PTY + child + canonical `TerminalState` cannot remain owned by the coordinator process**.

The surviving owner must hold the complete `TerminalExecution` authority:

```text
PTY master
+ child lifecycle
+ VT parser/modes
+ primary/alternate grids
+ scrollback/history authority
+ damage generation
+ terminal-generated replies
```

Keeping only the PTY fd in a helper is insufficient because the canonical terminal state would still disappear with Runtime R1. Restoring a new parser from later bytes would create discontinuity and could not reconstruct alternate-screen/mode/history truth.

## 4. Candidate topologies

### A. Per-execution worker — current leading hypothesis

```text
launchd / Seyal supervisor
  |
  +-- Runtime coordinator
  |
  +-- execution-worker E1
  |      -> TerminalExecution
  |      -> PTY + child + TerminalState
  |
  +-- execution-worker E2
         -> TerminalExecution
         -> PTY + child + TerminalState
```

Advantages:

- matches Seyal's existing per-`TerminalExecution` isolation principle;
- one worker crash loses one execution rather than every execution;
- coordinator can crash/restart without owning live PTY resources;
- worker protocol can expose bounded typed projection/control operations rather than terminal bytes as authority;
- clean future fit for external-client attach and remote execution seams.

Costs to measure:

- one process per execution;
- IPC for input, resize, projection updates, lifecycle and metadata events;
- fd/socket/process overhead at 10/50/100 executions;
- more supervision/reaping complexity.

### B. Shared execution-host process

```text
supervisor
  +-- Runtime coordinator
  +-- execution-host
         +-- E1
         +-- E2
         +-- ...
```

Advantages: fewer processes and potentially lower idle memory.

Concern: the execution-host becomes a large failure domain; one crash can kill every local terminal. That conflicts with the project's stated preference for per-`TerminalExecution` worker isolation unless measurements show per-execution workers are impractical.

### C. PTY-only keeper

Rejected as the default direction unless evidence changes the conclusion.

A process that only keeps the PTY master alive does not preserve canonical `TerminalState`; Runtime restart would need to reconstruct VT truth from incomplete information. That violates single authoritative terminal-state ownership.

## 5. Ownership model to prototype

Provisional worker ownership:

| State/resource | Provisional owner |
|---|---|
| PTY master | execution worker |
| primary child/process group lifecycle | execution worker |
| VT parser/modes | execution worker |
| primary + alternate grids | execution worker |
| canonical scrollback/history | execution worker |
| damage generation/projection producer | execution worker |
| terminal protocol replies | execution worker |
| `ExecutionId` immutable live identity | execution worker + coordinator registry reference |
| Workspace association / Block metadata | Runtime/workspace authority |
| client `AttachmentId` / Controller leases | Runtime coordinator |
| GUI/Metal presentation | disposable client |

The worker must not become Workspace, Block, agent, licensing or cloud authority.

## 6. Re-adoption protocol requirements

A replacement Runtime must never infer a live execution from pid, cwd, socket name or persisted metadata alone.

Minimum handshake to investigate:

```text
Runtime R2 starts
-> discover bounded worker endpoints from supervisor-owned registry
-> verify local peer identity and worker executable identity
-> exchange protocol version/capabilities
-> worker presents immutable ExecutionId + worker instance identity
-> R2 acquires a new coordinator generation/lease
-> worker atomically fences older coordinator generations
-> R2 reconciles Workspace/Block metadata references
-> old AttachmentIds/Controller leases remain invalid
-> fresh client attachment may reconnect
```

Split-brain is a mandatory failure case:

```text
R1 suspended
R2 becomes current coordinator
R1 resumes
=> worker rejects all stale R1 control mutations
```

`RuntimeId` should remain process-incarnation identity. Re-adoption should not pretend R2 is R1.

## 7. Phase-1 prototype boundary

The first prototype should avoid production GUI changes and prove only the ownership/failure property.

Recommended experiment:

```text
seyal-execution-worker --prototype
  -> spawns one TerminalExecution
  -> owns PTY + TerminalState
  -> exposes a private versioned Unix-domain control socket

prototype coordinator
  -> asks worker for identity/snapshot
  -> submits bounded input/resize
  -> consumes projection updates
```

Evidence sequence:

1. spawn a shell in worker;
2. record `ExecutionId`, worker pid and child pid;
3. run a long-lived process producing output;
4. kill coordinator only;
5. prove worker + same child remain alive and VT state continues to advance;
6. launch replacement coordinator;
7. authenticate and re-adopt worker;
8. prove same `ExecutionId`, same worker, same child and zero new PTYs;
9. obtain current projection and resume input;
10. attempt stale coordinator mutation and prove fencing rejects it.

Prototype code is evidence only and must not be merged as the production runtime path from this spike.

## 8. Mandatory measurements

For 1/10/50/100 executions measure:

- worker RSS and total attributable RSS;
- idle CPU/wakeups;
- process/thread/fd/socket counts;
- PTY output throughput and latency;
- input latency;
- projection IPC bytes/copies/allocations;
- coordinator restart + adoption latency;
- high-volume output while coordinator is absent;
- resource return after repeated coordinator crashes/restarts.

A per-execution worker is acceptable only if these numbers stay consistent with Seyal's low-memory/low-CPU objective.

## 9. Security/failure gates

Before architectural acceptance, prove behavior for:

- forged/stale worker endpoint;
- socket replacement;
- pid reuse;
- stale Runtime generation after resume;
- stale client Controller token after coordinator restart;
- coordinator crash during adoption;
- worker crash during/after adoption;
- partial metadata reconciliation;
- orphan workers;
- malicious/slow coordinator connection;
- log/privacy handling with terminal secrets;
- worker resource exhaustion.

No public identifier alone is authorization.

## 10. Decision gate

This spike will produce one of two outcomes:

### Promote to M004

Only if the worker architecture proves correctness, isolation, security and acceptable 1/10/50/100 execution resource cost. Then:

1. amend the Runtime/workspace continuity authority;
2. create an ADR for execution-worker ownership/supervision;
3. create a protocol/spec for worker discovery, fencing and re-adoption;
4. split production implementation into independently reviewable issues/PRs.

### Defer beyond M004

If worker overhead or supervision complexity threatens v0.1 terminal quality, retain honest M004 recovery:

```text
Runtime dies
-> live PTY is lost
-> durable workspace/history can restore
-> replacement execution gets a new ExecutionId
```

Do not fake live continuity through journaling or PTY-byte replay.

## 11. Current recommendation

Proceed with **per-execution worker as the first prototype**, not as an accepted production decision.

Reason: it is the only candidate that naturally preserves all three current Seyal requirements simultaneously:

1. one authoritative `TerminalExecution` state;
2. Runtime-coordinator crash independence;
3. per-execution failure isolation.

The shared-host design remains the comparison candidate if measured per-worker resource cost is too high.
