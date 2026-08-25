# Performance engineering

Performance claims require measurements. Terminal latency, CPU and RSS are architectural constraints, not polish.

## Hot-path rule

No avoidable synchronous IPC, JSON, serialization, copies, allocations, locks, agent calls, persistence, cloud/licensing/telemetry, Lua, or Block semantics may enter canonical terminal progress.

The repository enforces this rule in two layers:

1. Deterministic structural CI guardrails reject known forbidden primitives in explicitly registered hot-path functions.
2. Controlled Apple-Silicon measurements establish absolute latency, CPU, RSS and renderer budgets where shared CI runners are too noisy for trustworthy thresholds.

A new or renamed terminal hot-path function must be registered in `scripts/check-hot-path.py` in the same PR. Removing a function from that registry requires explicit performance-review justification.

## Deterministic CI guardrails

Every PR and master push must reject:

- blocking locks in registered terminal hot paths;
- thread/process hops and blocking sleeps in those paths;
- serialization/JSON and filesystem/network I/O in those paths;
- obvious per-call heap construction/copy helpers such as `Vec::new`, `vec!`, `to_vec`, `to_owned`, `String` construction and `format!` in those paths;
- unbounded channels in those paths;
- benchmark targets that omit the non-claim marker or timing primitive;
- benchmark targets that silently assert `performance_claim=true`.

The validator has controlled negative fixtures. A validator that cannot reject its own bad fixture is not a guardrail.

These checks deliberately do not outlaw every allocation or clone globally. Lifecycle/setup paths legitimately allocate. The protected scope is canonical progress: input ingress, Runtime dispatch/read/write service, and terminal parser/state feed. Future renderer/projection hot paths must be added as they become production-authoritative.

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

## CI strategy

Fast PR checks use deterministic structural guards and stable benchmark contract checks. Shared CI may run benchmark workloads as smoke/evidence, but it must not enforce noisy absolute latency/CPU/RSS thresholds that can randomly pass or fail due to host contention.

Broader benchmark matrices run scheduled/release and on performance-sensitive changes. Absolute product budgets should be enforced on a controlled Apple-Silicon runner with pinned hardware, OS/build mode and toolchain once that runner exists.

Pre-commit hooks may duplicate fast checks for developer feedback, but they are convenience only. CI is authoritative for mergeability.

## Regression handling

A material regression blocks merge unless explicitly accepted through the correct authority/change process with measured tradeoff evidence. Never hide a regression by changing the benchmark workload or threshold in the same opaque implementation change.

Any change that adds a new hot-path primitive not understood by the structural validator requires explicit review and, where practical, a new validator rule plus negative fixture. The goal is monotonic guardrail coverage: the protected surface should become stricter as production renderer, projection, reconnect and remote paths are introduced.
