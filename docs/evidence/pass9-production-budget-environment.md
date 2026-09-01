# Pass 9 production budget environment report

- **Status:** `ENVIRONMENT_UNSUPPORTED`
- **Date:** 2026-09-01
- **Scope:** physical controlled-host Pass 9 lifecycle/performance cohorts only

No performance measurement is recorded by this artifact. The current
production worktree contains the deterministic budget validator but does not
contain the pre-implementation calibration branch's controlled cohort
generator. This sandbox also denies process-table inspection, so it cannot
establish the otherwise-idle/exclusive-host precondition required for retained
RSS and detached-CPU evidence.

Older calibration logs and calibration-branch results are not exact-head
production evidence and were not copied, transformed, or represented as a run
of this branch. The validator self-test uses synthetic boundary fixtures solely
to prove fail-closed validation behavior; those fixtures are not measurements.

When an exact-head production cohort artifact is collected on an idle
Apple-Silicon host, validate it without changing its recorded commit:

```sh
python3 scripts/check-pass9-production-budget.py \
  --expected-head <full-40-character-production-head> \
  <retained-pass9-production-evidence.json>
```

Until that command passes against retained raw evidence, the physical stress,
RSS, detached-idle CPU, native-ready, and paired Pass 8 gates remain unproven.
