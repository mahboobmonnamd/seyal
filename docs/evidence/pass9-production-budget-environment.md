# Pass 9 production budget environment report

- **Status:** `ENVIRONMENT_UNSUPPORTED`

## Local paired resize reproduction (2026-09-01)

Five exact-head runs of `pass7_input_resize` were collected on the Apple-Silicon
host at commit `f649e035dc9ab071e1a146cb9f49b8fb898c58b7`. The 120×40 paired
Pass 8 p99 deltas were `-0.94%`, `-3.75%`, `+3.47%`, `+3.66%`, and `-7.48%`.
All five stayed below the fixed `+10%` blocking threshold. Raw logs are
retained in `/private/tmp/seyal-pass9-cohorts/` for reviewer inspection; this
local reproduction does not replace the required independently reviewed CI
and controlled production evidence.
- **Date:** 2026-09-01
- **Scope:** physical controlled-host Pass 9 lifecycle/performance cohorts only

This report also records a governance limitation: the production branch was
started before Issue #719 had been explicitly marked `Ready`. That ordering
violation must be recorded in the Issue and the implementation must be
re-baselined, frozen, and independently reviewed before this evidence can be
used for acceptance.

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
The artifact must additionally link the exact production head, host
preconditions, five independent cohorts, raw resource counters, and the
release package/signing inspection. Calibration artifacts from PR #726 cannot
close these exact-head production gates.
