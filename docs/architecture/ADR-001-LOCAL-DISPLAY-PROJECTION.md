# ADR-001 — Local Display Projection for macOS M001

**Status:** Accepted for M001 implementation

**Date:** 2026-08-23

**Related foundation rationale:** `R-002`, `R-003`, `R-021`, `R-022`, `R-023`, `R-024`, `R-027`

## Context

The accepted foundation fixes the authority model:

```text
Seyal Runtime
→ authoritative TerminalState
→ derived local display projection
→ Seyal.app renderer
```

The remaining M001 decision is how the Runtime supplies that derived projection to the local macOS app without moving VT ownership into the GUI, introducing synchronous request/response IPC, serializing cells through JSON/text, or making slow/hidden clients backpressure PTY→VT progress.

## Decision

Use a **hybrid local projection**:

1. A versioned compact binary Unix-domain connection carries attach/detach, execution identity, input/control, resize requests, generation metadata, resync requests, and lifecycle events.
2. For each attached visible terminal surface, the Runtime creates a bounded, versioned shared-memory projection region and remains its **single writer**.
3. The region contains only renderer-facing immutable-by-generation data derived from canonical `TerminalState`; it never exposes the Runtime's mutable VT/grid heap.
4. The projection uses generation-stamped slots/buffers. The writer publishes a complete generation atomically only after its payload is ready. The client reads only committed generations.
5. Damage records identify changed rows/runs/metadata for incremental rendering. A bounded full visible-state snapshot is always available for first attach, reconnect, missed-generation recovery, and corruption/version mismatch handling.
6. A one-way lightweight notification wakes the app when a newer generation is available. Terminal progress never waits for renderer acknowledgement.
7. If the client falls behind the bounded projection window, it skips obsolete intermediate generations and consumes/resyncs to the newest complete generation.
8. Hidden/detached executions do not keep a dedicated renderer projection region unless an attached client needs one.

Conceptually:

```text
PTY
→ canonical VT mutation
→ damage generation N
→ Runtime writes derived projection slot N
→ publish generation N
→ one-way wake
→ app reads generation N
→ Rust/native render preparation
→ Metal
```

Input remains independent:

```text
NSEvent
→ native normalization
→ compact binary input/control message
→ Runtime canonical mode/key encoding
→ PTY
```

## Projection contents

M001 projection data is intentionally renderer-facing and bounded. It may include:

- protocol/version header;
- `ExecutionId` and attachment generation;
- terminal rows/columns;
- committed projection generation;
- cursor position/visibility/style subset;
- terminal mode bits needed by presentation/input coordination;
- damage descriptors;
- visible cell/run data required for shaping and drawing;
- compact style/color identifiers;
- full-visible-snapshot metadata.

It must not contain:

- mutable Rust pointers/struct layouts;
- canonical parser state;
- authoritative primary/alternate grid ownership;
- unbounded scrollback;
- Block output copies;
- JSON;
- per-cell Swift callbacks.

## Why not socket-only cell deltas as the default

Compact binary deltas over a Unix-domain socket remain a credible fallback and benchmark comparator, but making every display mutation a socket payload imposes avoidable copying/serialization pressure as visible surface count and output rate rise. It is simpler, but M001 should not lock the permanent local render path to repeated cell payload copies without measurement.

## Why not shared canonical TerminalState

Sharing the canonical mutable terminal heap across processes would blur ownership, expose implementation layout as an ABI, complicate crash safety and synchronization, and create a de facto second authority boundary. The shared region is therefore strictly a rebuildable renderer projection.

## Why not theoretical zero-copy

M001 does not require zero-copy between all stages. It requires minimal practical copies and bounded synchronization. A copy from canonical terminal state into a compact renderer projection is acceptable if measurements show it is cheaper and simpler than a more fragile scheme.

## Required M001 benchmark check

Before Pass 5 is accepted, benchmark at least:

```text
A. compact binary Unix-domain snapshot/deltas
B. hybrid Unix-domain control + shared-memory projection
```

with equivalent terminal workloads and 1/10/50/100 execution scenarios where applicable.

Capture:

- TerminalState/damage → readable projection latency;
- app wake → projection read latency;
- bytes copied/written per visible update;
- allocations/update;
- Runtime and app CPU;
- RSS;
- reconnect/full-snapshot cost;
- behavior when the app is stalled or killed.

The hybrid design remains selected unless the socket-only implementation is measurably equivalent/better enough to justify its simpler implementation. If measurements overturn this choice, amend this ADR with the evidence; do not move VT ownership to the GUI.

## Safety and reconnect invariants

- Runtime is the only canonical terminal-state writer.
- Client writes never mutate projection state.
- An incomplete generation is never rendered.
- A killed GUI cannot corrupt Runtime terminal state.
- Reattach begins from a full current visible snapshot plus generation, then resumes damage updates.
- Missed generations cause coalescing/resync, never PTY backpressure.
- Projection protocol/layout is versioned independently of Rust internal structs.

## Consequences

This adds a small amount of projection management complexity in exchange for preserving persistent Runtime ownership without forcing the local display path through repeated high-level serialization. The mechanism is local-only; future remote/mobile transport can use bounded binary snapshot/delta streams derived from the same canonical state without sharing this memory layout.