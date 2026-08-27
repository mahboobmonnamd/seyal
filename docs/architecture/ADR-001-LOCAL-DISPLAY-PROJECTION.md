# ADR-001 — Local Display Projection for macOS M001

**Status:** Accepted — Candidate D split transport is the production M001 text/grid architecture. Controlled physical-Apple-Silicon performance validation passed at benchmark commit `c8c121380002c86a4e42b6737238289db10965af`; Issue #651 remains the closure authority for the complete Pass 5.1 acceptance set.

**Date:** 2026-08-23

**Amended:** 2026-08-25, 2026-08-26 (final controlled evidence and ancillary-security rationale added)

**Related foundation rationale:** `R-002`, `R-003`, `R-021`, `R-022`, `R-023`, `R-024`, `R-027`

## Context

The accepted Seyal authority model remains:

```text
PTY
→ TerminalExecution
→ Seyal-owned VT parser/state machine
→ one canonical TerminalState
→ derived presentation updates
→ attached client presentation cache
→ renderer
```

`TerminalExecution` remains the sole owner of the PTY, primary-child lifecycle and canonical terminal state. Runtime owns execution lookup, attachment/controller lifecycle and local transport. A client is never terminal authority.

The original Pass-5 decision provisionally selected a hybrid design: compact binary Unix-domain control traffic plus one Runtime-owned shared-memory grid projection per attached viewer. That implementation proved technically viable and produced useful correctness/security/failure evidence, but the widening transport comparison changed the architectural conclusion.

The important distinction is workload type:

- ordinary terminal presentation consists of relatively small, structured, damage-driven state changes where simplicity, low latency and bounded fanout matter;
- future images/rich graphics are large immutable payloads where avoiding repeated bulk copies may justify shared buffers.

Those two workloads must not be forced through the same transport merely because both eventually reach a renderer.

## Decision

Seyal adopts **Candidate D — split transport by data type**.

### 1. Control plane: compact versioned binary Unix-domain socket

The local Runtime connection carries:

- hello/version/capability establishment;
- execution enumeration;
- attach/detach;
- observer/controller authority;
- input;
- resize;
- lifecycle/error events;
- generation/resynchronization coordination.

Socket readiness stays on the existing Runtime/`ExecutionReactor` scheduling layer. Pass 5 adds no polling loop and no thread/process per attachment.

### 2. Text/grid presentation plane: binary terminal-model updates over the same UDS

For normal terminal presentation, Runtime sends generation-tagged **terminal-model updates**, not renderer-specific objects and not a per-view shared-memory grid.

The logical flow is:

```text
TerminalExecution
  PTY → VT → canonical TerminalState
                    │
                    │ consume canonical damage once
                    ▼
          TerminalModelUpdate
                    │
                    │ compact binary encode once per execution generation
                    ▼
             immutable frame(s)
          ┌─────────┼─────────┐
          ▼         ▼         ▼
       client A  client B  client C
          │         │         │
      RenderState RenderState RenderState
          │         │         │
        Metal     Metal     Metal
```

A terminal-model update may contain only presentation-neutral terminal data required to reconstruct the current visible model, including as applicable:

- generation;
- dimensions;
- changed row/cell range;
- cell scalar/style data supported by the accepted VT milestone;
- cursor state;
- primary/alternate-screen state;
- terminal mode bits required by presentation/input coordination.

It must not contain:

- Metal concepts;
- glyph-atlas indices;
- shaped-glyph caches;
- AppKit objects;
- GPU coordinates/resources;
- mutable Rust pointers/layout;
- parser internals;
- canonical grid ownership;
- unbounded history;
- JSON.

This keeps the persistent/headless Runtime independent of the macOS renderer and preserves a future remote transport boundary without prematurely creating a generic transport framework.

### 3. Client `RenderState` is derived, disposable and non-authoritative

Every attached client may maintain a derived cache convenient for rendering. That cache is never terminal authority.

The required failure/reconnect model is:

```text
client disappears
→ discard client RenderState
→ TerminalExecution continues unchanged

client reconnects
→ obtain current bounded snapshot at generation N
→ rebuild RenderState
→ resume incremental updates after N
```

No reconnect path replays historical PTY bytes and no client runs a second authoritative VT engine.

### 4. Snapshot for bootstrap/recovery; delta for steady state

Full current-state snapshots remain required for:

- first attach;
- reconnect;
- explicit resync;
- detected generation discontinuity;
- cases where bounded replacement is cheaper/safer than preserving intermediate display updates.

Normal steady-state presentation uses incremental damage-derived updates.

```text
snapshot = bootstrap/recovery mechanism
delta    = steady-state mechanism
```

Snapshots are current-state projections, not retained terminal histories.

### 5. Display updates are replaceable state, not an unbounded reliable event log

Control/input/lifecycle commands and presentation updates have different delivery semantics.

Control/input authority remains ordered and bounded according to the protocol specification.

Presentation state is coalescible. A slow client must never force Runtime to retain an unbounded sequence such as every generation from `N+1` through `N+100000`. If continuity cannot be preserved within bounded queues, Runtime marks that client for resynchronization; the client rebuilds from a current snapshot.

A stalled, suspended or killed client must never backpressure:

```text
PTY → VT → canonical TerminalState
```

and must not affect another client.

### 6. Fanout work is execution-scoped, not viewer-scoped where avoidable

For one canonical execution generation, Seyal should perform the expensive terminal work once:

```text
1 × canonical damage consumption
1 × terminal-model update construction
1 × binary encoding
N × bounded socket delivery/reference
```

The implementation must not intentionally become:

```text
N × terminal traversal
N × delta calculation
N × serialization
```

merely because N viewers are attached. Encoded presentation data should be immutable/shareable across connection queues where practical and measured.

The attachment identity remains connection-bound; presentation frames may rely on the one-attachment-per-connection state machine so viewer-specific identifiers do not force per-view re-encoding of otherwise identical terminal data.

### 7. Future bulk-object plane is separate and deliberately not implemented in M001

Large immutable image/graphics/media payloads are not forced through the textual grid update path.

The architecture reserves only an **execution-associated bulk-object seam**:

```text
canonical terminal/runtime ownership
        │
        ├─ textual terminal state
        │      → binary UDS terminal-model updates
        │
        └─ future large immutable object
               → object/reference identity
               → future measured shared-buffer transport
```

A future local macOS implementation may use POSIX shared memory, IOSurface or another platform-native mechanism if measurements and graphics semantics justify it. Remote transport may use a different bulk mechanism.

M001 does **not** implement this bulk transport and does not create a speculative generic transport abstraction for it.

The goal is **shared-buffer/minimal-copy bulk transfer**, not a promise of end-to-end “zero-copy”. Decoding, transformation or GPU upload may inherently require copies.

## Superseded provisional choice

The earlier provisional production choice was:

```text
binary UDS control
+
per-attachment shared-memory visible-grid projection
+
GenerationWake
```

That is no longer the intended production architecture for ordinary text/grid presentation.

The earlier shared-projection implementation may remain only as isolated comparator/reference code behind non-default benchmark support. It must not be reachable from normal production Runtime merely “for future images”. Future bulk graphics have a separate ownership/transport seam.

## Why this decision is stronger

### Simpler normal-terminal lifetime

The text/grid path no longer requires one shm object/mapping/reader fd/slot publication lifecycle per visible attachment, descriptor replacement on resize, shared-memory ABI synchronization, or a separate projection-memory budget merely to show ordinary terminal cells.

### Correct persistence boundary

The persistent Runtime remains authoritative while clients are thin and rebuildable. This matches Seyal's detach/reconnect requirement without duplicating terminal emulation in each GUI.

### Better failure isolation

Client cache corruption, renderer failure, queue saturation or client termination can be recovered with resync and cannot corrupt canonical terminal state.

### Better data-type fit

Small incremental terminal state and large immutable graphics have different copy/synchronization economics. Split transport lets each be optimized independently when evidence exists.

### Better future local/remote symmetry

The logical semantics — snapshot, delta, generation, resync, input, resize and lifecycle — are not intrinsically tied to shared memory. Local M001 uses UDS; a future authenticated remote transport can carry equivalent model semantics without introducing another terminal authority.

## Rejected alternatives

### Raw PTY bytes to every client

Rejected. Every client would need another VT interpretation/state machine, creating competing terminal state and difficult reconnect/conformance behavior.

### Full-grid socket snapshot on every update

Rejected as the steady-state design. Full snapshots remain recovery/bootstrap tools, not the default for every damage generation.

### Per-attachment shared-memory grid

No longer selected for ordinary terminal presentation. It remains a useful comparator because it gives the strongest case for a shared local projection, but its complexity must earn its place through measurements; it is not justified by hypothetical future images.

### Share the mutable canonical `TerminalState` heap

Rejected. It would expose implementation layout as an ABI, blur authority, complicate synchronization/crash safety and create a de facto second ownership boundary.

### Build the future bulk transport now

Rejected as speculative architecture. Preserve the seam only; design and implement the bulk mechanism when image/graphics requirements and measurements exist.

## First-attach transaction commit point

The attachment transaction remains fail-closed and Runtime-owned.

For the selected UDS presentation architecture the target order is:

```text
validate peer/state/role/ExecutionId/capacity
→ allocate AttachmentId privately
→ read current canonical visible TerminalState without consuming canonical damage
→ encode bounded current-state snapshot
→ enqueue/begin nonblocking Attached + initial snapshot delivery successfully
→ publish attachment/controller authority in Runtime registries
→ transition connection to Attached
```

“Delivery successfully” at this commit boundary means accepted by the Runtime's bounded nonblocking send/queue path; it is **not** a renderer/client acknowledgement and may never block PTY/VT progress.

A failure before Runtime authority publication leaves no controller lease or attachment record. After publication, disconnect cleanup owns later client loss and is idempotent.

## Generation/resync invariants

- Generation ordering comes from the canonical terminal/damage source, not from a client.
- Initial attach/reconnect/resync reconstruct from current canonical state.
- A client applies an incremental update only when its required predecessor generation is satisfied.
- A generation gap never causes PTY replay or a second VT parse.
- Queue saturation may replace obsolete presentation work with a bounded resync requirement/current snapshot.
- Client resync cannot block terminal progress.
- Final PTY bytes must be projected before execution teardown so attached clients can observe the final canonical state.

## Security invariants

- Same-UID local peer authentication and connection-bound attachment authority remain mandatory.
- Observer/controller authorization remains independent of presentation transport.
- Client-to-Runtime frames remain bounded and malformed input fails closed.
- No client-supplied length may cause unbounded allocation.
- A slow/malicious client may be disconnected without affecting terminal execution.
- Presentation payloads never expose canonical mutable memory or Rust layout.
- No secrets/input bytes are added to error/log payloads.
- Removing the text-grid shm transport does not weaken future bulk-object security requirements; any later descriptor/shared-buffer protocol requires its own explicit threat review.
- On Darwin, production ancillary receive storage is sized to the process descriptor-table capacity to prevent the SCM_RIGHTS truncation hazard for every descriptor set the process can legally receive. Client→Runtime descriptor transfer remains forbidden. A reported `MSG_CTRUNC` or malformed/oversized ancillary header is still protocol-fatal and bounded to owned storage during parsing.

## Required Pass-5 validation gate

The architecture decision is accepted, and the controlled physical-M5-Pro production benchmark at `c8c121380002c86a4e42b6737238289db10965af` grants the M001 Pass 5.1 performance sign-off for Candidate D. Issue #651 remains the authority for closing the complete correctness/security/documentation/review gate.

The measured path is:

```text
real shell/process
→ real PTY
→ Seyal VT mutation
→ canonical TerminalState
→ canonical damage extraction
→ terminal-model delta construction
→ binary UDS delivery
→ client RenderState apply/readable state
```

At minimum the acceptance evidence covers:

### Fanout

- 1 viewer;
- 2 viewers;
- 4 viewers;
- 8 viewers;
- 16 viewers of the same execution.

### Workloads

- sparse interactive output;
- normal shell command output;
- sustained high-volume streaming/logs;
- burst output;
- scrolling;
- full-screen redraw/TUI-style churn;
- primary and alternate screen.

### Geometry

- 80×24;
- 120×40;
- 200×60;
- accepted maximum/boundary cases where practical.

### Lifecycle/failure

- first attach;
- detach/reattach;
- reconnect;
- resync after a generation gap;
- resize;
- slow client;
- killed client;
- execution finalization with final output.

### Metrics

Record at least:

- PTY-read/terminal-mutation → client-state-ready p50/p95/p99;
- throughput;
- Runtime CPU and meaningful client CPU where measurable;
- Runtime/client RSS or justified process-model limits;
- allocations/reallocations/bytes allocated;
- bytes copied/written;
- socket write/send syscall count where instrumentable;
- queue depth/coalescing/resync frequency;
- descriptor/thread/resource counts;
- cleanup state.

The decisive stress combination is:

```text
sustained high-output streaming
× same-execution fanout
× real PTY → VT → model update → UDS → client cache
```

including the 16-viewer case and a large representative geometry.

Benchmark output must identify commit, build mode, hardware, OS, run count and percentile method. Single-sample or asymmetric comparator output is diagnostic only.

## Measured evidence (M001 Pass 5.1)

`crates/seyal-runtime/benches/pass5_production_transport.rs` traverses the real selected path end to end: real child process → real PTY → Seyal VT → canonical `TerminalState`/damage → Candidate-D binary encode → production Unix-domain socket → real client `DisplayCache` decode/apply. It is not a synthetic encoder-only or comparator path.

### Historical diagnostic evidence

Earlier physical Apple M2 Pro runs (`Mac14,9`, macOS 26.6.2, rustc 1.98.0, commit range `be9f9b8`–`af8adfe`) completed the matrix but also showed roughly 30–50x run-to-run latency variance and a reproduced timeout under extreme host contention (`vm.loadavg` around 69). Those measurements were valuable diagnostic evidence and helped distinguish benchmark-workload pacing from Runtime/Candidate-D processing, but they were explicitly not sufficient for absolute product sign-off.

Two unrelated defects discovered while making the benchmark trustworthy were fixed independently of the transport decision:

1. `EVFILT_PROC`/`NOTE_EXIT` exit notifications could be missed relative to `waitpid(WNOHANG)`, stranding an execution and causing a dead-PTY EOF busy-spin. Runtime now treats EOF as an independent exit signal and carries an explicit bounded retry state.
2. Three TUI benchmark workloads accidentally used malformed escape literals; the fixtures now emit real terminal control sequences.

### Final controlled physical-Apple-Silicon evidence

The final acceptance evidence was captured from the production benchmark at commit `c8c121380002c86a4e42b6737238289db10965af` on:

```text
macOS 26.5.2 (25F84)
model Mac17,9
Apple M5 Pro
rustc 1.98.0 (88d9e12ae 2026-08-18)
release build
percentile method: nearest-rank
```

The decisive `sustained_high_output_2s` workload at `200x60` completed for fanout `1/2/4/8/16` with no timeout, panic, platform error, shutdown failure or pending accepted-but-unwritten input. Across two captured M5 Pro runs, the 16-viewer case measured approximately:

- p95 update-to-client-cache latency: `3.168–6.336 ms`;
- p99 update-to-client-cache latency: `3.682–6.780 ms`;
- sampled CPU: `24–35.9%`;
- populated RSS: approximately `15.9–18.4 MiB`;
- source PTY throughput: approximately `220–274 KiB/s`;
- aggregate UDS throughput: approximately `166–207 MB/s`;
- `shutdown_ok=true` and `aggregate_pending_input_final=0`.

The ordinary 16-viewer interactive case remained much lower latency (about `122 µs` p95 in the captured run). The matrix also exercised token streaming, normal command output, burst/scroll, partial/full TUI redraw, alternate screen, reconnect and a 512×256 representative maximum geometry.

The host created 27 live PTYs in the requested 50/100 execution-population cases and then returned `Device not configured (os error 6)`. Those rows remain correctly classified `PLATFORM_LIMITED`; they are not reinterpreted as a Seyal scalability ceiling.

Full-redraw and alternate-screen stress show high cumulative allocation volume despite bounded retained RSS. That remains an explicit optimization/regression target, but the captured evidence does not establish a retained-memory leak or a material M001 architecture failure.

**Decision:** the controlled evidence does not trigger Candidate D's reopen criterion. Candidate D remains the accepted production text/grid transport.

## Reopen criterion

This ADR is evidence-driven rather than ideological.

Reopen the transport choice if production-equivalent measurements show that binary model-delta fanout materially fails Seyal's latency/CPU/RSS objectives — for example, if serialization/socket copying dominates, p99 degrades materially with fanout, or full-screen churn scales unacceptably — and an execution-scoped shared publication mechanism demonstrates a substantial measured advantage.

If that happens, evaluate the simplest mechanism that fixes the measured bottleneck. Do **not** automatically return to one shared-memory grid per attachment.

## Implementation state

The Candidate-D migration is complete for M001 production text/grid presentation:

1. ADR-001 is the architectural authority for Candidate D.
2. SPEC-004 and contributor/security/performance docs describe binary snapshot/delta + generation/resync semantics.
3. Production Runtime attachment/display delivery no longer uses per-view shared grids.
4. Earlier shared-projection code is isolated as non-default comparator/reference support and is not reachable from normal production attachment delivery.
5. The decisive streaming/fanout matrix has been exercised on the real selected path.
6. Issue #651 owns final closure after exact-head correctness/security/performance/documentation review and CI are green.

This amendment does not authorize Pass 6+, remote transport, graphics protocol implementation or renderer work.

## Consequences

Positive:

- fewer production mechanisms for normal terminal display;
- no per-view shm grid lifecycle in the intended steady-state path;
- disposable client state and straightforward reconnect/resync;
- one canonical VT/grid authority retained;
- natural bounded fanout semantics;
- future graphics can use a purpose-built bulk path without contaminating text-grid transport;
- local protocol semantics can later inform a remote transport without sharing local memory.

Costs:

- clients must maintain a derived RenderState cache;
- binary snapshot/delta framing requires careful bounded encoding/decoding;
- generation discontinuity and backpressure/resync semantics must be explicitly tested;
- full snapshots may still copy visible state during attach/recovery;
- fanout/socket-copy behavior must be measured rather than assumed.

These costs are preferable to retaining a more complex per-view shared-memory grid without measured need.