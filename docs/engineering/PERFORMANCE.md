# Performance engineering

Performance claims require measurements. Terminal latency, CPU and RSS are architectural constraints, not polish.

## Hot-path rule

No avoidable synchronous IPC, JSON, serialization, copies, allocations, locks, agent calls, persistence, cloud/licensing/telemetry, Lua, or Block semantics may enter canonical terminal progress.

## Evidence

Performance-sensitive Issues must define before implementation:

- workload and terminal dimensions;
- hardware/OS/build mode;
- metrics and percentile method;
- baseline commit/result;
- expected budget/target;
- acceptable regression threshold or explicit decision process.

Measure applicable:

- key/input latency stages;
- PTY read → TerminalState mutation;
- TerminalState/damage → projection;
- projection → present;
- end-to-end output latency;
- throughput;
- idle/active CPU;
- Runtime and app RSS;
- thread count;
- allocations/copies;
- reconnect/snapshot cost;
- GPU resources for visible/hidden surfaces.

Never state “faster”, “zero-copy”, “low CPU”, or equivalent as achieved without evidence.

## Baselines

Keep reproducible benchmark definitions separate from production code and record environment metadata with results. M001 targets in the accepted architecture remain targets until measured.

## CI strategy

Fast PR checks use stable smoke benchmarks or guardrails only where noise is controlled. Broader benchmark matrices run scheduled/release and on performance-sensitive changes. A noisy benchmark must not become a fake gate; investigate measurement quality instead.

## Regression handling

A material regression blocks merge unless explicitly accepted through the correct authority/change process with measured tradeoff evidence. Never hide a regression by changing the benchmark workload or threshold in the same opaque implementation change.
