---
name: performance-gate
description: Measure Seyal latency, throughput, CPU, RSS, threads, allocations/copies and related hot-path costs against recorded baselines.
---

# Performance gate

Read `docs/engineering/PERFORMANCE.md` and the active Issue/milestone budgets.

1. Define the workload, dimensions, hardware/OS/build mode, run count and percentile method before measuring.
2. Select the exact baseline commit/result and target/threshold.
3. Measure only relevant metrics, including staged terminal latency and CPU/RSS where the Issue affects hot paths.
4. Record raw/reproducible result metadata; do not cherry-pick a best run.
5. Compare baseline vs candidate and identify noise/variance.
6. Investigate material regressions rather than hiding them through threshold/workload changes.
7. Report targets separately from achieved measurements.
8. If a regression is intentional, require explicit documented approval at the correct authority level before merge.

Never claim zero-copy, faster, lower CPU/RSS or equivalent without measurement evidence.
