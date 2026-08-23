# Benchmark harness

Issue #11 establishes the reproducible benchmark metadata contract before performance-sensitive production code exists.

`environment-fields.toml` defines the metadata every future benchmark result must carry. `scripts/benchmark-smoke.py` records a real harness-smoke environment snapshot under `target/benchmarks/` and explicitly marks it as **not a performance result**.

Future benchmark workloads belong here or in a justified harness package. Generated measurements belong under ignored build/artifact locations, not committed production modules.

Do not claim latency, CPU, RSS, throughput or zero-copy results from the harness smoke. Real measurements must identify workload, hardware/OS/build mode, commit, terminal dimensions, font/scale, shell, run count and percentile method as required by `docs/engineering/PERFORMANCE.md` and M001.
