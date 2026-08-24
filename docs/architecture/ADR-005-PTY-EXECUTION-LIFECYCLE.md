# ADR-005 — PTY endpoint and child lifecycle ownership

- **Status:** Accepted
- **Date:** 2026-08-24
- **Issue:** #28
- **Scope:** local terminal execution ownership on macOS

## Context

M001 has Seyal's permanent `seyal-terminal` VT/state implementation. The next
slice needs a real PTY and child lifecycle without recreating RILL's kernel
coupling, GUI-owned lifetime, duplicate terminal state, synchronous language
round trips, or oversized PTY modules.

RILL is reviewed behavior evidence only. Seyal architecture and SPEC-002 are
authority.

## Decision

Create `seyal-exec` as the physical execution boundary:

```text
TerminalExecution
  ├─ exactly one TerminalEndpoint / PTY master
  ├─ exactly one primary child/session/process-group lifecycle
  └─ exactly one authoritative seyal-terminal::TerminalState
```

`seyal-exec → seyal-terminal` is allowed. The reverse dependency is forbidden.

### Endpoint ownership

The execution owner retains the PTY master for the whole live execution.
Attachments, GUI, renderer and Blocks never own it. The descriptor is private;
public callers receive operations and value state, not a raw-fd escape hatch.

### Child/session ownership

The child calls `setsid` and acquires the PTY slave as controlling terminal
before `exec`. The resulting session/process group is owned by the
`TerminalExecution`. Natural exit and signal exit remain distinct and reap is
idempotent.

Before explicit group signaling, Seyal verifies that the live child's process
group still matches the group created for the execution. Once the child is
reaped, signaling is never attempted again.

### Detach is not terminate

Detach is a later Runtime attachment transition. It does not close the
execution-owned PTY and does not signal the child. The Runtime must keep the
`TerminalExecution` alive while zero GUI clients are attached.

Dropping the actual execution owner is therefore **not** the detach API.

Issue #28 deliberately does not invent a hidden `Drop` termination timeout. Explicit
termination requires a caller-supplied policy and detach must preserve the live
execution. The later Runtime owner is therefore required to hold each live
`TerminalExecution` until it has either observed/reaped natural exit or performed
explicit policy-driven termination. Runtime implementation must provide a
supervised reap/cleanup path so a programmer error cannot turn dropped ownership
into an intentional detach contract.

Until that Runtime owner exists, tests and examples must explicitly reap or
terminate every spawned execution before the owning value leaves scope. A future
Runtime PR must add failure tests for owner shutdown/crash paths; this PTY slice
must not pretend that object destruction is persistent-session management.

### Nonblocking I/O

The master is nonblocking. Reads and writes represent partial progress and
would-block. Readiness uses `poll`, never `select`, so descriptor numbers above
`FD_SETSIZE` remain valid.

No permanent thread per PTY is required. No JSON, serialization, Swift callback,
agent, persistence, cloud, licensing or Block operation participates in byte
progress.

A bounded write helper is allowed only when the caller supplies the timeout.

### Terminal-state relationship

PTY output bytes feed the existing `seyal-terminal::TerminalState` directly.
`seyal-exec` does not create a parser, grid, transcript engine or renderer
mirror.

### Resize

The endpoint owns `TIOCSWINSZ`/`TIOCGWINSZ`. Valid sizes have nonzero rows and
columns. The normal kernel SIGWINCH behavior is preserved; no renderer-owned
dimension authority is introduced.

Resize follows the canonical Foundation §5.4 transaction. It is one **logical
Runtime operation** across kernel PTY geometry and canonical `TerminalState`:

```text
validate resize authority + WindowSize
→ prepare all locally rejectable/infallible resize inputs
→ apply fallible PTY TIOCSWINSZ
→ commit canonical TerminalState resize/reflow
→ expose damage/projection
```

For macOS M001 the implementation validates `WindowSize`, applies the fallible
PTY `TIOCSWINSZ`, and only after that succeeds resizes the canonical
`TerminalState`. This avoids publishing/reflowing canonical state when the
kernel refuses the resize. Physical endpoint-first commit does not make the PTY
semantic authority; `TerminalState` remains the one canonical terminal state.

With a validated nonzero `WindowSize`, the current M001 `TerminalState::resize`
has no recoverable failure other than invalid size. If that contract changes,
resize must be redesigned as an explicit prepare/commit/rollback transaction so
endpoint and canonical state cannot diverge after a partial commit.

No renderer projection/damage consumer may observe a successful new geometry
until the canonical `TerminalState` resize is complete.

### macOS platform seam and unsafe policy

M001 implements macOS only. Direct Darwin/POSIX FFI is confined to
`src/platform/macos.rs`. That module is the sole production unsafe-code
exception in `seyal-exec`; the crate remains `unsafe_code = "deny"` everywhere
else.

This narrow exception is justified because controlling-terminal setup requires
operations such as `openpty`, `setsid`, `ioctl`, `poll`, `fcntl`, `getpgid` and
`kill`. Hiding them behind a larger PTY/runtime framework would add a stronger
dependency and obscure ownership without removing the underlying OS contract.

The implementation uses the Rust `libc` binding crate only for ABI declarations
and constants. It is not a terminal engine or architecture dependency.

### PTY creation choice

Use Darwin `openpty` for the macOS M001 implementation, followed by explicit
close-on-exec and nonblocking configuration of the master. The slave retains
normal interactive line discipline; production code does not expose RILL's
test-oriented `Raw` discipline switch.

This is intentionally macOS-local. Do not create a generic POSIX abstraction
until Linux is an active target.

### Environment choice

The PTY layer does not hard-code `TERM` and does not inject `SEYAL_INSIDE` or
legacy `RILL_*` markers. `CommandSpec` explicitly chooses inherited vs cleared
environment and optional overrides.

A TERM value becomes a product compatibility claim and belongs to the VT/shell
compatibility milestone, not to PTY creation convenience.

### Termination policy

Explicit termination is separate from detach and requires a caller-supplied
`TerminationPolicy` containing:

- SIGTERM grace duration;
- post-SIGKILL reap duration.

No hidden two-second or other arbitrary product timeout is embedded in the PTY
layer. During termination only, a short bounded sleep is used between
nonblocking `try_wait` checks; this is not a terminal I/O hot-path loop.

The verified owned process group receives SIGTERM first. If it does not exit
inside the supplied grace period, the same verified group receives SIGKILL.
The child is then reaped within the supplied bound or termination reports a
timeout.

## RILL salvage decisions

### Preserved after validation

- execution side owns the PTY;
- `setsid` + controlling-terminal setup;
- nonblocking master I/O;
- `poll` instead of `select`;
- signal-aware child exit classification;
- explicit termination distinct from detach;
- bounded termination/reap;
- kernel PTY window-size operations;
- no public master descriptor.

### Corrected / redesigned

- split endpoint, child, readiness, winsize, command, execution and platform
  responsibilities instead of one PTY/kernel module;
- no `poll_with_extras` API that couples the PTY to future Runtime socket
  polling;
- no hard-coded two-second write timeout;
- no immediate unconditional SIGKILL;
- verify the owned process group before signaling;
- no hard-coded `TERM`;
- no production raw-line-discipline toggle used to support tests;
- unsafe code is isolated to one Darwin module;
- PTY bytes feed Seyal's existing authoritative `TerminalState`;
- resize follows the canonical prepare → endpoint commit → TerminalState commit
  transaction and exposes no damage until both physical states agree.

### Rejected / deferred

- RILL names and `RILL_INSIDE`;
- RILL `Session`/daemon/socket ownership;
- cwd vnode taps;
- mutation features/test hooks in production code;
- Linux/Windows implementation;
- Runtime attachment/reconnect/persistence behavior;
- hidden `Drop` termination policy in the PTY layer;
- renderer/Blocks/agent behavior.

## Consequences

`seyal-exec` is the second physical production Rust crate in M001. The platform
module becomes a small audited unsafe boundary, while all execution-facing APIs
remain safe Rust. Runtime and native UI can later compose this crate without
becoming PTY owners.

The Runtime milestone inherits an explicit lifecycle obligation: it must own
live executions across client detach and provide supervised reap/termination on
actual Runtime shutdown/ownership release.

## Reopen conditions

Revisit only if correctness/performance/platform evidence shows that this
ownership or syscall boundary cannot satisfy a real target. Library convenience
alone is not sufficient.
