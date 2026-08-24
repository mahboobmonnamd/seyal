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

The M001 benchmark harness contract lives under `benches/`. `benches/environment-fields.toml` defines the required metadata fields. `scripts/benchmark-smoke.py` writes a real environment record under `target/benchmarks/` solely to prove that recording is reproducible; it sets `performance_claim = false` and is not a latency/CPU/RSS/throughput result.

Future measured workloads must replace `not-applicable` smoke fields with real terminal dimensions, font/scale, shell, workload, run count and percentile method. Generated benchmark records stay out of production modules and should be retained as explicit CI/release artifacts when a measurement Issue requires them.

For M001 Pass 5 transport evidence, run:

```bash
cargo bench -p seyal-runtime --bench runtime_scalability -- --nocapture
```

The Pass-5 comparator is intentionally an **equivalent display-delivery** comparison, not “IPC disabled” versus “IPC enabled”. `transport=socket-only` is a benchmark-only reference path that encodes the same fixed-width visible `CellRecord`/`DamageRecord` state and actually copies the complete snapshot through a nonblocking Unix stream. `transport=hybrid` uses the production control UDS, `SCM_RIGHTS`, read-only shared memory and `GenerationWake`. Both paths assert that the delivered state equals a snapshot of the same canonical `TerminalExecution` before their measurements are accepted.

The benchmark emits paired `runtime_resource` records at required populations (`1/10/50/100`) plus representative geometry/screen scenarios. It records visible attachment count separately from total execution population because SPEC-004 currently caps live attachments at 16. It reports display setup, update-to-readable and signal-to-readable timing, copied/projection bytes, socket write count, resync, reconnect, Runtime/child RSS, CPU, threads, fds and teardown state.

Records marked `classification=PLATFORM_LIMITED` are host-ceiling evidence and must be reported, not filtered out. In particular, a host PTY allocation ceiling is not evidence that the Runtime only scales to that population, and it must not be silently converted into a Pass-5 performance claim.

The comparator source and contributor/debugging notes are documented in `docs/engineering/LOCAL-ATTACHMENT.md`. Benchmark output is evidence only for the exact commit/build/environment that produced it; the benchmark's startup banner deliberately keeps `performance_claim=false` until reviewed results are recorded in the owning PR/milestone evidence.

## CI strategy

Fast PR checks use stable smoke benchmarks or guardrails only where noise is controlled. Broader benchmark matrices run scheduled/release and on performance-sensitive changes. A noisy benchmark must not become a fake gate; investigate measurement quality instead.

## Regression handling

A material regression blocks merge unless explicitly accepted through the correct authority/change process with measured tradeoff evidence. Never hide a regression by changing the benchmark workload or threshold in the same opaque implementation change.
