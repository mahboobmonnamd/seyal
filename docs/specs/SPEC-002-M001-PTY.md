# SPEC-002 — M001 PTY endpoint and child lifecycle

- **Status:** Active implementation contract
- **Date:** 2026-08-24
- **Owner:** `seyal-exec`
- **Issue:** #28
- **Architecture:** ADR-005, Seyal Foundation Architecture, MILESTONE-001

## 1. Purpose

Define observable M001 behavior for a local macOS terminal execution endpoint and child process. This specification constrains behavior before RILL PTY code is selectively salvaged.

It does not define VT semantics; output bytes are consumed by the already authoritative `seyal-terminal::TerminalState`.

## 2. Ownership invariants

1. One live `TerminalExecution` owns at most one PTY endpoint/master and one primary child lifecycle.
2. The same execution owns exactly one authoritative `TerminalState`; `seyal-exec` creates no second VT/grid.
3. Attachments/GUI objects never own the PTY lifetime.
4. Detach does not signal or intentionally close the live execution.
5. Explicit terminate is distinct from detach, natural exit and object/reference release.
6. The master descriptor is encapsulated by the endpoint implementation.

## 3. Spawn contract

For a successful local terminal spawn:

- a real PTY master/slave pair is created;
- the child has the slave as its controlling terminal with correct session/process-group semantics for interactive terminal behavior;
- the parent retains only the master-side ownership required by the execution;
- the master is configured for nonblocking operation;
- command/shell spawn failure is returned as an execution error and does not leave a live leaked child or descriptor set;
- inherited/configured environment is explicit at the spawn boundary.

M001 scaffolding does not invent a hard-coded `TERM`. The implementation must choose/inherit/configure `TERM` only after compatibility with the actual Seyal VT capability claim is justified and tested.

## 4. Byte I/O contract

### 4.1 Reads

- PTY reads expose the byte stream produced by the child without JSON, UTF-8 conversion, line parsing or terminal-semantic transformation in `seyal-exec`.
- Nonblocking no-data readiness is distinguishable from EOF/HUP and hard error.
- Partial reads are valid.
- Large/bursty output must make progress without deadlock or silent truncation attributable to the PTY layer.
- PTY output may be fed directly to `TerminalState::feed`; no intermediate duplicate terminal state is allowed.

### 4.2 Writes

- Input writes preserve byte ordering.
- Partial writes/backpressure are valid and must be represented/handled without busy waiting.
- Bounded helper behavior, if provided, must have an explicit policy rather than an arbitrary hidden timeout.
- No synchronous Swift/GUI/agent/persistence/cloud/licensing/Block callback participates in write progress.

### 4.3 Readiness

- Read/write readiness must not depend on `select` or `FD_SETSIZE`.
- The abstraction must remain valid when the PTY master has a descriptor number above traditional `select` limits.
- No permanent thread-per-PTY requirement may be introduced by the endpoint API.

## 5. Window size

A valid PTY size contains nonzero rows and columns. Pixel dimensions may remain zero unless a later rendering/input contract requires them.

On resize:

1. the PTY kernel state reflects the requested rows/columns;
2. the child can observe the new size through normal terminal APIs;
3. conventional child resize notification behavior is preserved;
4. repeated resize does not replace the PTY or child;
5. invalid zero row/column requests are rejected rather than partially applied.

## 6. Child lifecycle and exit status

The execution distinguishes at least:

- running;
- exited normally with an exit code;
- exited because of a signal;
- termination explicitly requested by Seyal;
- wait/reap failure.

Normal exit code and signal termination must not be collapsed into one integer status.

Wait/reap is idempotent at the ownership level: once terminal child ownership is resolved/reaped, later lifecycle queries must not accidentally wait on or signal an unrelated reused PID.

## 7. Explicit termination

Explicit termination targets only the process/session group owned by that `TerminalExecution` according to the implementation's validated lifecycle identity.

Termination must:

- be intentional and separate from detach;
- avoid unbounded busy waiting;
- use a documented bounded escalation/reap policy once the implementation chooses that policy;
- preserve signal-vs-exit result classification where observable;
- never signal a PID/process group after ownership has been conclusively released/reaped.

The exact grace/escalation durations are **not** chosen by this scaffold. They require implementation/test evidence and must be documented when introduced.

## 8. Detach behavior

Detach is a Runtime/attachment transition, not a PTY-close operation.

For M001 execution code this means the endpoint/child lifetime APIs must permit the later Runtime to retain a live `TerminalExecution` while zero GUI clients are attached. Nothing in the endpoint API may require GUI ownership to keep the child alive.

A later attach/detach Runtime test proves GUI-close survival end to end; Issue #28 proves the PTY/child layer does not bake terminate-on-client-release semantics into its ownership model.

## 9. EOF, HUP and failure

The endpoint must distinguish normal temporary no-data from terminal closure conditions.

Tests must cover child exit followed by master EOF/HUP behavior on macOS without assuming a single syscall/error ordering if the OS permits equivalent orderings. The contract is eventual, deterministic lifecycle resolution without spin, leak or duplicate close/reap.

## 10. Resource and performance constraints

- no forced thread per PTY;
- no busy-wait readiness loop;
- no terminal-byte JSON/serialization;
- no cross-language hot-path callback;
- no duplicate VT/grid;
- no avoidable per-byte heap allocation in steady-state I/O;
- descriptor and child resources must be released deterministically after execution termination;
- benchmark/readiness evidence must record environment/workload metadata and must not invent product performance claims.

## 11. Security/safety constraints

- raw descriptor ownership is encapsulated;
- command/path errors do not leak descriptors or zombie children;
- signaling is limited to the owned child/process group;
- environment handling must not log secrets;
- no RILL-specific environment names such as `RILL_*` remain;
- no commercial/licensing code enters the OSS execution path.

## 12. Required implementation tests

The Issue #28 implementation must add executable tests for:

1. real shell/command spawn and byte round-trip;
2. large/bursty output without PTY-layer loss/deadlock;
3. child-visible rows/columns and resize notification behavior;
4. normal exit code;
5. signal-caused exit distinct from normal exit;
6. master EOF/HUP lifecycle;
7. explicit terminate + reap of the owned execution only;
8. repeated spawn/terminate without descriptor/zombie leakage;
9. high descriptor-number operation proving no `select`/`FD_SETSIZE` dependency;
10. invalid command/shell path cleanup;
11. absence of RILL identifiers in production surface;
12. direct delivery of PTY bytes into the existing terminal-state path without another grid/state model.

Tests requiring real PTYs/process/session behavior run on macOS. Portable value/state tests may run on Linux when they do not fake macOS PTY semantics.

## 13. Benchmark evidence required before merge

At minimum record a reproducible local PTY workload covering sustained/bursty reads and readiness/write progress where appropriate. Record OS, hardware, build mode, workload, run count and measurement method using the repository benchmark metadata contract.

A first measurement establishes a baseline only. It is not a latency/CPU/RSS product claim unless the measurement Issue explicitly defines such a target.

## 14. Deferred / out of scope

- Runtime daemon and durable attachment registry;
- reconnect protocol;
- persistence/reboot recovery;
- Linux/ConPTY implementation;
- renderer/Metal behavior;
- Blocks/semantic extraction;
- full job-control policy beyond the interactive PTY lifecycle required by M001;
- remote/cloud execution;
- commercial features.

## 15. Implementation gate

The scaffold is complete when the crate/module/dependency/doc/CI structure exists and remains behavior-neutral. **Do not count the scaffold as PTY functionality.**

The implementation may begin only by adding real failing tests/fixtures for the first selected PTY behavior, then the smallest production code that passes them. Any RILL behavior that conflicts with this specification is rejected or separately escalated through architecture/spec change discipline.
