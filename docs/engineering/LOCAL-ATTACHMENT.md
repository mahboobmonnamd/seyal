# M001 local attachment and display projection

This document is the contributor map for M001 Pass 5. Normative architecture authority is `docs/architecture/ADR-001-LOCAL-DISPLAY-PROJECTION.md`; normative protocol behavior is `docs/specs/SPEC-004-M001-LOCAL-ATTACHMENT-PROJECTION.md` once its Candidate-D amendment is complete.

## Current transition state

ADR-001 now selects **Candidate D — split transport by data type**:

```text
TerminalExecution
  owns PTY + child lifecycle + canonical TerminalState
        |
        | canonical damage consumed once per execution
        v
Runtime
  owns execution/attachment/controller lifecycle
        |
        +-- compact binary UDS control/input/lifecycle
        |
        +-- compact binary UDS terminal-model snapshot/delta delivery
                |
                v
          disposable client RenderState
                |
                v
              Metal

future large immutable graphics/media
        -> separate measured shared-buffer path
           (for example shm/IOSurface if justified)
```

The migration is complete: PR #106 merged Candidate-D as the production attachment/display path, and production Runtime attachment/display delivery uses binary snapshot/delta + generation/resync exclusively. The earlier per-attachment shared-memory grid implementation remains in the tree only as isolated comparator/reference code, gated behind the non-default `benchmark-shared-projection` Cargo feature (`default = []`); it is not compiled into, and is not reachable from, a normal production build or `cargo test`/`cargo build`.

M001 Pass 5's remaining acceptance work was tracked by Issue #651 (performance-matrix rigor on controlled hardware, failure-injection audit completeness, and final independent review) rather than by this migration, which is done. Issue #651 is **closed** (Pass 5.1 acceptance complete). Do not delete useful shared-projection benchmark evidence, but do not reintroduce the shared-grid path into the production Runtime attachment path merely because future graphics may need shared memory — future bulk graphics has its own transport seam (see ADR-001 §7).

## Ownership

The following invariants do not change:

- `TerminalExecution` owns the real PTY, primary-child lifecycle and sole canonical `TerminalState`.
- Runtime owns execution lookup, connection-bound attachment identity and controller leasing.
- the client owns no PTY, VT parser, canonical grid, scrollback authority or mutable terminal memory;
- attach/reconnect/resync rebuild from current canonical state; historical PTY bytes are never replayed;
- client cache state is derived, disposable and non-authoritative;
- renderer failure, client failure or a slow client must never stall PTY -> VT progress;
- no second local IPC event loop and no thread/process per attachment.

Important implementation boundaries after migration:

- `crates/seyal-exec/src/execution.rs` — PTY/child/canonical terminal state and the projection-neutral snapshot/damage seam.
- `crates/seyal-exec/src/projection.rs` — terminal-model projection values independent of renderer and Runtime internals.
- `crates/seyal-runtime/src/runtime.rs` — attachment/controller authority, canonical damage consumption, model-update fanout, resync and lifecycle cleanup.
- `crates/seyal-runtime/src/local_ipc/framing.rs` — fixed binary wire protocol.
- `crates/seyal-runtime/src/local_ipc/connection.rs` — nonblocking bounded socket I/O and presentation/control queue semantics on the existing reactor.
- `crates/seyal-runtime/src/local_ipc/attachment.rs` — connection-bound attachment identity and one-controller-per-execution lease.
- `crates/seyal-runtime/src/local_ipc/discovery.rs` / `auth.rs` — endpoint discovery and same-UID peer validation.

The existing `projection/layout.rs`, `projection/lifecycle.rs`, `projection/writer.rs`, shared-memory FD transfer and shared-projection fuzz/bench code remain only while they provide comparator/failure evidence. Final production dependency on them must be removed unless ADR-001 is explicitly reopened with measured evidence.

## Presentation semantics

### Bootstrap and recovery

Full current-state snapshots are used for:

- first attach;
- reconnect;
- explicit resync;
- detected generation discontinuity;
- bounded replacement when incremental continuity cannot be preserved safely.

### Steady state

Normal display progress uses generation-tagged terminal-model updates derived from canonical damage. The update contains presentation-neutral terminal data only: changed cells/rows supported by the current VT milestone, cursor, dimensions, screen/mode metadata and generation information.

Do not send Metal objects, shaped glyphs, atlas coordinates, AppKit types, Rust layouts, parser state or canonical-grid pointers across the Runtime boundary.

### Fanout

One execution generation should incur expensive terminal work once:

```text
1 x damage consume
1 x model-update construction
1 x binary encode
N x bounded delivery/reference
```

Avoid N terminal traversals or N serializations for N viewers when the payload is otherwise identical. Encoded presentation bytes should be immutable/shareable where practical and measured.

### Backpressure and resync

Control/lifecycle authority and presentation state have different semantics.

- control/input/lifecycle remains ordered and bounded;
- display state is coalescible/replaceable;
- no unbounded generation history is retained for a slow client;
- if continuity is lost, mark the client for resync and rebuild from a current snapshot;
- one slow client must not delay another client or canonical terminal progress.

A valid client may therefore jump from generation `N` to a later current snapshot rather than receive every obsolete intermediate display generation.

## Discovery and trust boundary

The default Darwin endpoint remains under the verified per-user runtime directory. The directory is owner-only (`0700`) and `control.sock` is `0600`. Symlink/non-socket/insecure paths fail closed and an active connectable endpoint is never removed as stale.

Accepted peers are checked with Darwin peer credentials and must have the same effective UID as Runtime. This is a same-user trust domain, not a sandbox claim.

Opening the socket grants no execution authority. Every attachment is bound to the authenticated connection that created it. `AttachmentId` is an opaque identity, not a bearer capability. Observers cannot input or resize and there is at most one controller per execution.

Client-to-Runtime frames must not gain descriptor authority as a side effect of removing the text-grid shm path. Unexpected ancillary descriptors remain malformed/protocol-fatal unless a future explicitly versioned bulk-object protocol defines them.

## Attach/resync/finalization

Initial attach remains transactional:

```text
validate peer/state/role/ExecutionId/capacity
-> allocate AttachmentId privately
-> snapshot current canonical visible state without consuming canonical damage
-> encode bounded initial snapshot
-> enqueue nonblocking Attached + initial snapshot successfully
-> publish attachment/controller authority
-> transition connection to Attached
```

The enqueue boundary is Runtime-local admission, not a renderer acknowledgement.

Resync obtains a new current-state snapshot from canonical terminal state. It must never rebuild by replaying PTY bytes or introducing a client VT parser.

Before execution teardown, final PTY bytes must be reflected in the last deliverable canonical model update/snapshot. Only then may Runtime remove the execution and notify live attachments of finalization.

## Future bulk objects

M001 does not implement the graphics/media bulk plane. Preserve only the ownership seam for large immutable objects associated with an execution. A future implementation may use shared memory, IOSurface or another platform-native mechanism after separate security/performance analysis.

Do not call the future design guaranteed “zero-copy”; the requirement is minimal-copy/shared-buffer transfer where evidence supports it.

## Validation

Canonical repository gates:

```bash
make bootstrap
make build
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
make test
make check
make bench
```

Pass-5 focused checks must cover:

- real Runtime/PTY attach to an existing `ExecutionId` without spawning another PTY/shell;
- initial current-state snapshot;
- incremental terminal-model updates;
- primary/alternate screen and resize within accepted VT scope;
- detach/reattach/reconnect;
- generation-gap resync;
- final output before teardown;
- controller/observer authorization;
- invalid/stolen attachment identities;
- malformed/oversized protocol frames;
- slow/dead clients;
- multiple viewers of one execution;
- cleanup/resource return to baseline.

The final production path must also fuzz the binary terminal-model snapshot/delta decoder and generation/resync state machine with bounded resources. Shared-memory reader fuzzing may remain as comparator coverage only while comparator code remains in the branch.

## Performance gate

The decisive path is:

```text
real PTY
-> real Seyal VT mutation
-> canonical TerminalState
-> canonical damage
-> terminal-model update
-> binary UDS
-> client RenderState apply/readable
```

Measure fanout 1/2/4/8/16 on the same execution across sparse interactive output, normal command output, sustained and burst logs, scrolling, full-screen/TUI-like churn and alternate screen. Include 80x24, 120x40 and 200x60.

Record p50/p95/p99 output-to-client-state latency, throughput, CPU, RSS, allocations/reallocations, bytes copied/written, socket syscalls where instrumentable, queue/coalescing/resync behavior and cleanup resources.

The decisive stress case is sustained high-output streaming x 16 viewers x a large representative geometry through the real production pipeline.

Existing socket-vs-shared-projection benchmarks remain diagnostic comparison evidence. They do not substitute for measuring the final selected UDS delta path.

See `docs/engineering/PERFORMANCE.md` and ADR-001 for evidence/reopen rules.

## Debugging rules

If display state is stale, check in this order:

1. attachment still belongs to the same live connection;
2. canonical damage generation advances;
3. Runtime consumes that damage once;
4. the expected terminal-model generation is encoded;
5. the bounded presentation queue either delivers it or marks resync;
6. client RenderState applies the expected predecessor generation or performs a current-state resync.

Never “fix” display bugs by adding PTY replay, a client-side canonical VT engine, another damage consumer, another event loop, sleep polling, unbounded queues, synchronous renderer acknowledgements or renderer-specific state in Runtime.
