# seyal-exec

`seyal-exec` is Seyal OSS's terminal-execution ownership boundary.

For M001 it owns:

- one macOS POSIX PTY endpoint/master;
- the primary child/session/process-group lifecycle;
- nonblocking byte I/O and poll-based readiness;
- PTY window-size operations;
- explicit bounded termination/reap behavior;
- `TerminalExecution`, which couples that endpoint to exactly one authoritative
  `seyal_terminal::TerminalState`.

It deliberately does **not** own a second parser/grid, renderer state, Blocks,
GUI lifetime, Runtime attachment orchestration, persistence, cloud/licensing or
commercial behavior.

## Platform and unsafe boundary

M001 production PTY support is macOS-only. Direct Darwin/POSIX FFI is confined
to `src/platform/macos.rs`, which is the only production module allowed to use
unsafe code. The public and ownership layers above it are safe Rust.

The only external dependency added for this boundary is the Rust `libc` binding
crate. It is used for Darwin/POSIX ABI definitions; it is not a terminal engine,
runtime framework or event loop.

## Environment

`CommandSpec` inherits the caller environment by default or can clear it
explicitly and add selected values. Seyal does not silently inject `TERM`,
`SEYAL_INSIDE`, legacy `RILL_*` variables or shell configuration.

A future TERM capability claim must be justified by the VT compatibility
milestone rather than hidden in the PTY layer.

## I/O

The master is nonblocking. `read`/`write` expose partial progress and
would-block explicitly. `write_all_bounded` requires the caller to supply its
timeout. Readiness uses `poll`, never `select`, and the master descriptor is
never exposed as public API.

## Lifecycle

Natural exit and signal exit are represented separately. Explicit termination
requires a caller-supplied `TerminationPolicy`; it first targets the verified
owned process group with SIGTERM, then escalates to SIGKILL only after the
supplied grace period, and uses a separate supplied reap bound.

Detach is not represented by dropping this execution. The later Runtime keeps
`TerminalExecution` alive while GUI/client attachments come and go.
