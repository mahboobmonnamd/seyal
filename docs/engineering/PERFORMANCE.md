# Performance engineering

Performance claims require measurements. Terminal latency, CPU and RSS are architectural constraints, not polish.

## Hot-path rule

No avoidable synchronous IPC, JSON, high-level serialization, copies, allocations, locks, agent calls, persistence, cloud/licensing/telemetry, Lua or Block semantics may enter canonical terminal progress.

Canonical PTY/VT progress must never synchronously depend on a renderer/client.

## Evidence

Performance-sensitive Issues must define before acceptance:

- workload and terminal dimensions;
- hardware/OS/build mode;
- exact commit SHA;
- metrics and percentile method;
- run/repetition count;
- baseline/result being compared;
- expected budget/target or explicit decision process;
- acceptable regression threshold.

Measure applicable:

- key/input latency stages;
- PTY read -> TerminalState mutation;
- TerminalState/damage -> presentation-model update;
- presentation update -> client cache ready;
- end-to-end output latency;
- throughput;
- idle/active CPU;
- Runtime and client RSS;
- thread/descriptor counts;
- allocations/reallocations/bytes allocated;
- bytes copied/written;
- syscall/write counts where instrumentable;
- queue depth/coalescing/resync frequency;
- reconnect/snapshot cost;
- GPU resources for visible/hidden surfaces once renderer work exists.

Never state “faster”, “zero-copy”, “low CPU” or equivalent as achieved without evidence.

## M001 Pass 5 transport authority

ADR-001 now selects Candidate D:

```text
control/input/lifecycle
    -> compact binary UDS

normal terminal presentation
    -> generation-tagged binary terminal-model snapshots/deltas over UDS
    -> disposable client RenderState cache

future large immutable graphics/media
    -> separate measured bulk-object path
       (shared memory/IOSurface-style if later justified)
```

The earlier per-attachment shared-memory grid is no longer the intended production text/grid path. It may remain temporarily as comparator/reference code while the selected UDS path is implemented and measured.

Do not preserve shared-memory grid machinery in production merely because future images may benefit from shared buffers. Text/grid and bulk graphics are different workloads and have separate transport decisions.

## Pass-5 decisive benchmark

Before PR #106 may be Ready for Review/merge, benchmark the real selected path:

```text
real shell/process
-> real PTY
-> Seyal VT mutation
-> canonical TerminalState
-> canonical damage extraction
-> terminal-model update construction
-> compact binary UDS delivery
-> client RenderState apply/readable state
```

### Required fanout

Use multiple viewers of the **same execution**:

- 1;
- 2;
- 4;
- 8;
- 16.

Fanout must not be represented only as one viewer per many independent executions.

### Required workloads

Include:

- sparse interactive output;
- normal shell command output;
- sustained high-volume streaming/logs;
- burst output;
- scrolling;
- full-screen redraw/TUI-like churn;
- primary and alternate screen.

### Required geometry

Include at least:

- 80x24;
- 120x40;
- 200x60.

### Required lifecycle/failure cases

Cover or cross-reference production tests for:

- first attach;
- detach/reattach;
- reconnect;
- explicit/generation-gap resync;
- resize;
- slow client;
- killed client;
- execution finalization after final PTY output.

### Required metrics

Record at least:

- PTY-read/terminal-mutation -> client-state-ready p50/p95/p99;
- throughput;
- Runtime CPU and meaningful client CPU where measurable;
- Runtime/client RSS or explicit process-model limitations;
- allocations/reallocations/bytes allocated;
- bytes copied/written;
- socket write/send syscall count where instrumentable;
- queue depth, coalescing and resync frequency;
- descriptor/thread/resource counts;
- cleanup state.

The decisive stress combination is:

```text
sustained high-output streaming
x same-execution fanout
x real PTY -> VT -> model update -> UDS -> client cache
```

including 16 viewers and a large representative geometry.

## Fanout implementation guardrail

The expensive execution-level work should be approximately:

```text
1 x canonical damage consumption
1 x terminal-model update construction
1 x binary encoding
N x bounded socket delivery/reference
```

Do not intentionally perform N terminal traversals, N delta calculations or N serializations solely because N viewers are attached when the payload is otherwise identical.

Immutable encoded presentation data may be shared/referenced by bounded per-connection delivery queues where practical. This is an optimization seam, not permission for unbounded retention.

## Backpressure performance rule

Presentation state is replaceable/coalescible. A slow client must not cause an unbounded generation queue.

If bounded continuity cannot be retained, the client is marked for resync and later rebuilt from a current snapshot. Control/input/lifecycle semantics remain ordered and independently bounded.

No renderer acknowledgement is allowed in the canonical PTY/VT progress path.

## Comparator evidence

Existing benches such as `runtime_scalability`, `pass5_transport_stress` and `pass5_shared_projection` remain useful evidence for understanding copies, allocations, fanout and the previous shared-grid candidate. They are comparator/diagnostic evidence only unless they exercise the exact selected production path.

Earlier single-sample or asymmetric results must not be promoted to final architecture claims. Host PTY ceilings must be reported as platform-limited evidence, not hidden or reinterpreted as a Seyal scalability limit.

Benchmark output is evidence only for the exact commit/build/environment that produced it.

## Reopen rule

ADR-001 may be reopened if production-equivalent measurements show that UDS model-delta fanout materially violates Seyal's latency/CPU/RSS goals and a simpler execution-scoped shared publication mechanism demonstrates a substantial measured advantage.

If evidence forces a revisit, do not automatically restore one shared-memory grid per attachment. Choose the smallest mechanism that fixes the measured bottleneck while preserving one canonical terminal authority and renderer independence.

## Baselines and CI

Keep reproducible benchmark definitions separate from production code and record environment metadata with results.

Fast PR checks use stable smoke benchmarks/guardrails only where noise is controlled. Broader matrices belong in explicit performance validation, scheduled/release jobs or performance-sensitive change gates. A noisy benchmark must not become a fake gate.

## Regression handling

A material regression blocks merge unless explicitly accepted through the correct ADR/spec authority with measured tradeoff evidence. Never hide a regression by changing benchmark workload or thresholds in the same opaque implementation change.
