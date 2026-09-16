# M002 #824 performance coordination with #673

- **Issue:** #824
- **Sibling authority:** #673 (versioned M002 release-performance contract)
- **Date (UTC):** 2026-09-16
- **`performance_claim`:** `false`

#824 must not duplicate or weaken #673. This validation stream **did not**
start the PHYSICAL_ARM64 five-cohort / 20-warmup / 100-sample matrix. #837
remains deferred until after that measurement pass.

## What this PR claims

Nothing quantitative. High-volume VT/PTY fixtures prove history stays within
`HISTORY_PER_EXECUTION_BYTE_CAP` and that feed continues; they are not latency,
CPU, RSS, or key-to-photon evidence.

Existing #819/#842/#673 comparative rows on `master` are not re-interpreted as
a new baseline. Foundation `make bench` with
`SEYAL_REQUIRE_DISPLAY_LINK_BENCHMARK=0` remains class `CI` harness smoke.

## Required follow-up (not this PR)

1. After #824 independent review, #673 owners run the PHYSICAL_ARM64 matrix on
   the accepted exact head.
2. Comparative benches, if re-run, stay `performance_claim=false` until the
   contract validator accepts a release row.
3. Do not treat this document as headed presentation proof.
