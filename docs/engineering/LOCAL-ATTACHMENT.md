# M001 local attachment and display projection

This document is the contributor map for the M001 Pass 5 implementation. The normative behavior remains `docs/specs/SPEC-004-M001-LOCAL-ATTACHMENT-PROJECTION.md` and its accepted ADRs; this page explains where the implementation lives, how ownership is split, how to validate it, and how to debug failures without creating a second terminal authority.

## Ownership

The production path is:

```text
TerminalExecution
  owns PTY + child lifecycle + canonical TerminalState
        |
        | one destructive canonical damage consumer per execution
        v
Runtime
  owns attachment registry + controller lease + projection lifetime
        |
        +-- compact binary Unix-domain control/input socket
        |
        +-- Runtime-writable shared-memory display projection
                |
                +-- independently opened O_RDONLY descriptor
                    transferred to one local client with SCM_RIGHTS
```

There is no client-side VT parser in this pass, no second terminal grid, no replay PTY, and no PTY per Block/attachment. Attach, reconnect and resync project the current canonical `TerminalExecution` state.

Important implementation boundaries:

- `crates/seyal-exec/src/execution.rs` owns the real PTY/child/canonical terminal state and exposes projection snapshots.
- `crates/seyal-exec/src/projection.rs` is the terminal-neutral projection boundary used by Runtime; this prevents `seyal-runtime` from depending directly on `seyal-terminal`.
- `crates/seyal-runtime/src/runtime.rs` owns local-attachment composition, authority, projection fanout, replacement and lifecycle cleanup.
- `crates/seyal-runtime/src/local_ipc/framing.rs` owns the fixed binary wire format.
- `crates/seyal-runtime/src/local_ipc/connection.rs` owns nonblocking bounded socket framing/queues. Readiness comes from the existing `ExecutionReactor`; it does not own another event loop.
- `crates/seyal-runtime/src/local_ipc/attachment.rs` owns connection-bound attachment identity and the one-controller-per-execution lease.
- `crates/seyal-runtime/src/local_ipc/discovery.rs` owns local endpoint discovery/validation.
- `crates/seyal-runtime/src/local_ipc/auth.rs` owns Darwin peer-credential validation.
- `crates/seyal-runtime/src/local_ipc/fd_transfer.rs` owns SCM_RIGHTS send/receive helpers.
- `crates/seyal-runtime/src/projection/layout.rs` owns the fixed shared-memory ABI.
- `crates/seyal-runtime/src/projection/lifecycle.rs` owns shm object/fd/mmap lifetime.
- `crates/seyal-runtime/src/projection/writer.rs` owns atomic publication and reader validation.
- `crates/seyal-runtime/src/projection/producer.rs` converts execution snapshots into ABI records.

## Scheduling and hot-path rules

Pass 5 registers the listener and accepted local sockets into the same Darwin kqueue used by `ExecutionReactor`. There is no local-IPC polling timer and no thread/process per attachment.

The PTY/VT path never waits synchronously for an attachment, projection reader, renderer, persistence system or agent. A stalled client is bounded by the local connection queue policy and may be disconnected without stopping terminal progress.

Canonical damage is consumed exactly once per execution and then fanned out to all live projections. The projection-neutral execution snapshot carries the canonical `full/first_row/last_row` redraw guidance; ordinary row-local damage is not promoted to full-screen damage. Attach and resync use non-destructive current-state snapshots and therefore deliberately carry full redraw guidance; they must never call the destructive damage consumer.

## Discovery and trust boundary

The default Darwin endpoint is created under a verified per-user runtime directory. The directory is owner-only (`0700`) and `control.sock` is explicitly `0600`. Existing symlink/non-socket or insecure paths are rejected. A connectable active socket is never unlinked as stale.

Accepted peers are checked with Darwin peer credentials and must have the same effective UID as the Runtime. This is a same-OS-user trust domain, **not** a sandbox or process-isolation boundary. An open socket grants no attachment authority.

Every attachment is bound to the authenticated connection that created it. `AttachmentId` is an opaque identity, not a bearer capability: presenting another connection's ID cannot authorize input, resize, resync or detach. Observers cannot input or resize. At most one controller exists for an execution.

## Binary protocol and bounds

The control/input protocol is explicitly encoded and versioned; Rust struct layout is never used as wire layout.

Current M001 limits from SPEC-004 include:

- 24-byte frame header with `SEYALIPC` magic and protocol 1.0;
- frame payload at most 262,144 bytes;
- input payload at most 65,536 bytes;
- at most 16 live attachments;
- mandatory outbound queue at most 262,144 bytes per connection;
- at most one in-flight/pending advisory `GenerationWake` history point: a newer not-yet-started wake replaces the older pending wake.

Opening the socket does not select an execution. The client must complete `ClientHello` and then attach explicitly.

## Shared-memory ABI and lifetime

Runtime is the sole writer of projection memory. A projection is created from a new POSIX shared-memory object, the writer maps it read/write, an independent descriptor is opened `O_RDONLY`, and the shm name is unlinked before publication. The client receives exactly one read-only descriptor with the `Attached` or `ProjectionReplaced` control frame.

`ReadOnlyMapping` re-validates descriptor access mode, ABI-bounded logical length and `FD_CLOEXEC` on the receiving side before mapping. Darwin may report a page-rounded backing extent; only the logical ABI length from the validated control frame is mapped/exposed.

The ABI uses two slots. Publication sequence/generation words and concurrently accessed payload words are accessed atomically. Writers publish with release ordering and readers validate with acquire ordering. Do not create normal Rust references/slices over concurrently mutated shared bytes; helper reads copy atomically into owned memory.

Reader safety is tied to the actual mapped length, not just the encoded header. `read_region_header` rejects an encoded region larger than its `RegionMemory`; `read_latest` revalidates full two-slot bounds/alignment before using decoded offsets; and atomic word accessors enforce alignment and bounds in release builds before pointer arithmetic. Malformed projection metadata must become a validation error rather than an out-of-bounds access or panic.

Limits are 8 MiB per projection region and 128 MiB aggregate projection memory per Runtime.

## Attach, replacement, resync and finalization

Initial attach is transactional: build the projection, enforce aggregate budget, prepare the read-only fd and enqueue `Attached`+fd before publishing the attachment/controller/projection to Runtime registries. A failed delivery must not leave a controller lease or projection allocation behind.

A resize always updates the canonical terminal first. If the existing projection is too small, Runtime builds a replacement and transfers `ProjectionReplaced`+fd before committing the registry switch. If replacement cannot be completed, the canonical resize remains valid and the projection may become temporarily unavailable. A later `Resync` rebuilds from canonical state.

When an execution reaches finalization, Runtime first publishes any canonical damage still pending from the PTY drain while `TerminalExecution` remains alive. Only after that final projection publication does Runtime remove the execution, notify every still-live attachment connection with `Lifecycle::Finalized`, and release attachment/projection/controller state. This ordering prevents bytes read immediately before EOF from disappearing from an already attached client's final projection. Attachments whose projection was already unavailable still receive the lifecycle notification.

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

Focused Pass 5 checks on macOS:

```bash
cargo test -p seyal-runtime local_ipc -- --nocapture
cargo test -p seyal-runtime projection -- --nocapture
cargo test -p seyal-runtime --test local_ipc_protocol -- --nocapture
cargo test -p seyal-runtime --test local_ipc_adversarial -- --nocapture
cargo test -p seyal-runtime --test final_projection -- --nocapture
cargo bench -p seyal-runtime --bench runtime_scalability -- --nocapture
```

Retained fuzz targets cover binary protocol decoding, the production shared-projection reader (`read_region_header` → `read_latest`) and real Runtime reconnect/resync state transitions over the Unix-domain protocol. Fuzz seeds are operation/resource bounded. A retained seed run is a deterministic smoke gate; it must not be described as a long mutational fuzz campaign.

## Transport comparator

`crates/seyal-runtime/benches/runtime_scalability.rs` is the ADR-001 display-transport comparator. It compares semantically equivalent visible snapshots from the same canonical execution:

1. a benchmark-only reference transport that serializes the fixed-width display state and actually copies it through a nonblocking Unix stream; and
2. the production hybrid path using the real control UDS, `SCM_RIGHTS`, read-only shared memory and `GenerationWake`.

The comparator covers populations 1/10/50/100, representative terminal geometries, primary/alternate screen, update/readable latency, signal/readable latency, bytes/writes, RSS/CPU/fd/thread counts, resync, reconnect and cleanup. Host PTY ceilings are reported as `PLATFORM_LIMITED`, never silently discarded or converted into performance claims.

The current ADR evidence gate remains open until the comparator also records the missing allocation/fanout/repetition evidence defined by SPEC-004. Do not infer that shared memory is justified merely because the hybrid path is implemented, and do not switch to socket-only from an asymmetric or single-sample run.

See `docs/engineering/PERFORMANCE.md` for evidence rules. Benchmark output is evidence only for the exact commit/environment that produced it.

## Debugging guide

If Runtime startup reports local IPC discovery/stale-endpoint failure:

1. verify only one intended production Runtime owns the per-user endpoint;
2. inspect the runtime-directory owner and mode;
3. verify `control.sock` is a socket, not a symlink/regular file;
4. do not manually unlink a socket that still accepts connections;
5. for tests that are not testing Pass-5 IPC, use `LocalIpcMode::Disabled` or a unique short test runtime directory rather than making production discovery less strict.

If attach succeeds but no display update arrives, check in this order:

1. the attachment still belongs to the same connection;
2. the execution's canonical damage generation advances;
3. Runtime's single damage fanout publishes a newer projection generation;
4. the `GenerationWake` matches the attachment/projection identity;
5. `read_latest` validates a committed slot/generation.

Do not fix display-update bugs by adding PTY replay, a client VT parser, another damage consumer, another event loop, sleep-based polling or unbounded queues. Those violate the accepted architecture rather than repair it.
