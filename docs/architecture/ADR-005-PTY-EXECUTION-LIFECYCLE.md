# ADR-005 — PTY endpoint and child lifecycle ownership

- **Status:** Accepted for M001 implementation
- **Date:** 2026-08-24
- **Issue:** #28
- **Scope:** local terminal execution ownership on macOS

## Context

M001 now has a permanent `seyal-terminal` VT/state implementation. The next vertical slice must add the real POSIX terminal endpoint and child lifecycle without reintroducing the architectural failures that Seyal is explicitly avoiding: GUI-owned PTYs, duplicate terminal engines, synchronous language/process round trips, accidental terminate-on-detach behavior, or a large RILL-derived kernel module copied wholesale.

RILL is useful evidence for individual PTY behaviors, but it is not architectural authority. Seyal owns the production execution architecture.

## Decision

Create `seyal-exec` as the physical ownership boundary for terminal execution.

The authoritative ownership chain for a local terminal execution is:

```text
TerminalExecution
  ├─ exactly one TerminalEndpoint / PTY master
  ├─ exactly one owned child lifecycle
  └─ exactly one authoritative seyal-terminal::TerminalState
```

`seyal-exec` consumes `seyal-terminal`. `seyal-terminal` never depends on process/PTY ownership.

### 1. Endpoint ownership

The execution side owns the PTY master for the full live-execution lifetime. The GUI, renderer, Block system and attachments do not own or duplicate it.

The raw master descriptor remains encapsulated inside the endpoint implementation. Public callers receive operations/value state, not an unrestricted descriptor escape hatch.

### 2. Child lifecycle

The execution side owns child identity, session/process-group state, wait/reap state and exit classification.

Natural exit, signaled exit and explicit termination remain distinguishable. Signaling must be based on currently owned lifecycle identity; code must not blindly signal a stale/reused PID or process group after ownership has ended.

### 3. Detach is not terminate

A client/GUI detach removes an attachment to a live `TerminalExecution`; it does not drop execution ownership and it does not signal the child.

Closing the execution-owned PTY is therefore **not** the detach mechanism. The later Runtime keeps the execution object alive while attachments come and go.

Explicit termination is a separate operation and must be intentional, bounded and followed by correct reap/exit handling.

### 4. Nonblocking I/O and readiness

The PTY master is nonblocking. Read/write progress uses OS readiness rather than busy waiting and must not rely on `select`/`FD_SETSIZE` assumptions.

The execution abstraction must not require one permanent thread per PTY. It must remain compatible with the later Runtime event loop/worker strategy without embedding that Runtime now.

No JSON, serialization, agent call, persistence call, cloud/licensing check, Block semantic extraction or Swift callback may be inserted into PTY byte progress.

### 5. Terminal-state relationship

PTY output bytes are delivered to the already authoritative `seyal-terminal::TerminalState`. `seyal-exec` must not own another parser, grid, transcript engine or renderer state.

The execution layer may coordinate byte delivery and lifecycle; terminal semantics stay in `seyal-terminal`.

### 6. Resize

PTY resize belongs to the execution endpoint. The contract is that the kernel/child observes the requested terminal size and conventional resize signaling behavior required by SPEC-002. The renderer does not own PTY dimensions.

### 7. Platform seam

macOS is implemented first. The platform seam remains internal and narrow: endpoint creation, controlling-terminal/session setup, nonblocking flags/readiness, window size, signaling and wait/reap operations.

Do not create a cross-platform PTY framework before another platform is active. Future Linux may share POSIX code where evidence supports it; Windows/ConPTY is a separate platform implementation.

### 8. Implementation mechanism is not fixed by scaffolding

This ADR does **not** preselect `openpty` vs `posix_openpt`, a specific syscall wrapper crate, or a spawn/fork helper before the RILL salvage review and macOS API review. Those are implementation choices unless they alter the ownership, detach, lifecycle, safety or performance decisions above.

The workspace currently forbids unsafe code in Seyal crates. If direct unsafe FFI is proposed, the Issue must explicitly justify a change to that policy. Prefer a reviewed safe syscall abstraction when it satisfies the required semantics without hidden architecture cost.

## Rejected alternatives

### PTY owned by the Swift/macOS host

Rejected. GUI close/detach would become entangled with execution lifetime, headless operation becomes harder, and terminal progress would cross the language boundary unnecessarily.

### PTY owned by `seyal-terminal`

Rejected. VT/grid semantics and process/descriptor ownership are different responsibilities and portability boundaries.

### PTY owned by a Block/session presentation object

Rejected. Blocks are presentations/history metadata over the same execution and must never create or own another PTY.

### Copy RILL PTY/kernel module wholesale

Rejected. We salvage validated behavior only. Seyal uses its own names, ownership, error model, module decomposition and tests; known `select`, timeout, teardown or daemon-coupling assumptions are not inherited automatically.

### Drop means terminate

Rejected. It conflicts with the persistence/detach wedge and makes attachment/UI lifetime unsafe as an execution lifetime signal.

## Consequences

- `crates/seyal-exec` becomes the second physical Rust production crate in M001.
- The crate depends downward on `seyal-terminal` only among current Seyal production crates.
- PTY behavior tests must be real macOS/process tests, not mocked green scaffolding.
- Runtime/daemon/attachment orchestration remains out of scope; this ADR leaves a clean ownership seam for it.
- The implementation PR must list RILL behavior preserved, corrected, deferred and rejected.

## Reopen conditions

Revisit this ADR only if measured/correctness evidence shows the ownership boundary itself is wrong, or a target platform fundamentally cannot preserve the same execution semantics. Library/API convenience alone is not sufficient.
