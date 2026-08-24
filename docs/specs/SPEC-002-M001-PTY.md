# SPEC-002 — M001 PTY endpoint and child lifecycle

- **Status:** Active implementation contract
- **Date:** 2026-08-24
- **Owner:** `seyal-exec`
- **Issue:** #28
- **Architecture:** ADR-005, Seyal Foundation Architecture, MILESTONE-001

## 1. Purpose

Define observable M001 behavior for a local macOS PTY execution and its primary
child lifecycle. VT semantics remain owned by `seyal-terminal`.

## 2. Ownership invariants

1. One live `TerminalExecution` owns one PTY endpoint and primary child
   lifecycle.
2. The same execution owns one authoritative `TerminalState`.
3. No attachment/GUI/renderer/Block object owns the PTY.
4. Detach is not terminate.
5. Raw master descriptors are not public API.
6. `seyal-terminal` never depends on `seyal-exec`.

## 3. Command and environment

`CommandSpec` requires an explicit program and supports arguments, current
directory, environment inheritance/clearing and overrides.

The PTY layer:

- does not invoke a shell unless the caller explicitly chooses one;
- does not inject `TERM`;
- does not inject `SEYAL_INSIDE`;
- contains no `RILL_*` environment names;
- does not log environment values.

## 4. Spawn

Successful macOS spawn must:

- create a real PTY master/slave;
- apply the requested valid window size;
- make the master nonblocking and close-on-exec;
- connect slave stdin/stdout/stderr;
- create a new child session/process group;
- acquire the slave as controlling terminal before exec;
- close parent-side slave ownership after spawn.

A spawn error must release all created descriptors and leave no live child owned
by the failed call.

Non-macOS M001 calls return `UnsupportedPlatform`; they do not fake PTY
semantics.

## 5. Byte I/O

### Read

`read` returns exactly one of:

- `Bytes(n)` for raw child bytes;
- `WouldBlock` for temporary no-data;
- `Eof` for PTY closure.

Partial reads are valid. On macOS the implementation normalizes the PTY-master
closure form represented by zero-length read or `EIO` to `Eof`.

No UTF-8 decoding, line parsing, JSON, serialization or terminal-semantic
mutation occurs inside `TerminalEndpoint::read`.

### Write

`write` returns partial byte progress or `WouldBlock`. Byte order is preserved.

`write_all_bounded` may retry using writable readiness, but its timeout is
supplied by the caller. No hidden write timeout exists.

### Readiness

Read and write readiness use `poll`. `select`/`fd_set` are forbidden. A master
descriptor above 1024 must remain functional.

No permanent thread-per-PTY requirement is introduced.

## 6. TerminalExecution

`TerminalExecution::read_output` feeds each successful PTY byte slice directly
to its owned `TerminalState::feed`.

There is no second parser/grid/state mirror in `seyal-exec`.

Resize updates the PTY first and then the same authoritative terminal state. A
valid `WindowSize` guarantees the terminal resize cannot fail because of zero
rows/columns.

## 7. Window size

Rows and columns must be nonzero. Pixel dimensions may be zero.

`set_window_size` must make `window_size` report the requested values and must
preserve the same child/PTY identity. Normal kernel resize notification behavior
must remain observable by the child.

## 8. Child exit

`ChildExit` distinguishes:

- `Exited(code)`;
- `Signaled(signal)`.

Once reaped, `try_wait` returns the same stored result and never waits on a
reused PID.

## 9. Explicit termination

`TerminationPolicy` is caller supplied and contains:

- SIGTERM grace duration;
- SIGKILL reap duration.

Termination:

1. checks for natural exit first;
2. verifies that the live child's process group matches the execution-owned
   group;
3. sends SIGTERM to that group;
4. returns if the child exits inside the supplied grace period;
5. otherwise verifies/signals the same group with SIGKILL;
6. reaps within the supplied post-kill bound;
7. returns `TerminationTimedOut` if the child still cannot be reaped.

No signal is sent after a terminal child result has already been reaped.

## 10. Detach

Issue #28 does not implement Runtime attachments. Its contract is structural:
nothing in the PTY API requires a GUI/client reference to keep the execution
alive, and there is no detach operation implemented as PTY close or child
signal.

End-to-end GUI-close survival is a later Runtime milestone.

## 11. EOF/HUP

After child/slave closure, the master must eventually expose EOF or HUP without
busy spinning. Equivalent macOS ordering between final buffered bytes, HUP and
EOF is accepted.

## 12. Safety and resources

- direct unsafe FFI exists only in `src/platform/macos.rs`;
- all public APIs above that module are safe Rust;
- master descriptor ownership stays encapsulated;
- no external terminal engine is used;
- no cross-language callback exists in byte progress;
- no commercial code/dependency enters this crate;
- repeated spawn/terminate must not accumulate descriptors or zombies.

## 13. Tests required in PR #40

macOS executable tests cover:

1. real command/PTY input round trip;
2. large burst output;
3. child-visible resize and stable child identity;
4. normal exit code;
5. signal-caused exit;
6. EOF/HUP after exit;
7. explicit terminate and idempotent reap;
8. repeated spawn/terminate descriptor behavior;
9. operation with PTY descriptor above traditional `FD_SETSIZE`;
10. invalid executable cleanup;
11. direct PTY-byte delivery into authoritative `TerminalState`;
12. environment behavior and absence of RILL marker injection.

Portable source/value checks run on Linux and macOS where they do not fake PTY
behavior.

## 14. Benchmark

The M001 PTY benchmark uses a real macOS PTY with a raw/no-echo `cat` child,
fixed payload and fixed iteration count. It records elapsed time and bytes/sec
as a **baseline measurement only**, not a product performance claim.

Foundation Quality runs this benchmark on the macOS job. The user may repeat it
locally with `make bench` before PR #40 is merged.

## 15. Deferred

- Runtime daemon/attachment/reconnect;
- persistence/crash/reboot recovery;
- Linux and ConPTY implementations;
- renderer/Metal;
- Blocks/semantic extraction;
- remote/cloud execution;
- cwd semantic taps;
- commercial features.

## 16. Merge gate

PR #40 remains open until:

```text
working
+ tested
+ locally demonstrable
+ benchmarked
```

All Foundation Quality jobs must be green. The user-requested local macOS
validation is an additional pre-merge gate for this migration PR.
