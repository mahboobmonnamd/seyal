# `seyal-exec`

`seyal-exec` is the M001 ownership boundary for terminal execution: PTY/endpoint ownership, child lifecycle and the future `TerminalExecution` composition that consumes the authoritative `seyal-terminal::TerminalState`.

This crate is created by Issue #28 because PTY/process ownership is now a real physical boundary. The initial scaffold intentionally contains **no fake PTY implementation, no placeholder public API and no no-op tests**. Public types and executable behavior tests are added only with the behavior they implement.

## Dependency direction

```text
seyal-terminal
      ↑
  seyal-exec
```

`seyal-terminal` must never depend on `seyal-exec`. `seyal-exec` must not depend on renderer, workspace, runtime, GUI, commercial or agent code.

## Module responsibilities

- `execution` — future composition boundary for endpoint + child lifecycle + authoritative terminal state; no duplicate VT/grid.
- `endpoint` — PTY master ownership and descriptor encapsulation.
- `child` — child/session/process-group lifecycle, wait/reap and exit classification.
- `readiness` — nonblocking readiness and bounded read/write coordination; no busy wait.
- `winsize` — PTY size contract and resize behavior.
- `platform` — smallest internal OS-specific syscall seam; macOS first.

The exact implementation mechanism is intentionally not scaffolded. RILL remains behavioral evidence only; the implementation must be revalidated against ADR-005, SPEC-002 and current macOS/POSIX behavior before code is salvaged.
