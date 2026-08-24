# GitHub Issue protocol

GitHub Issues + GitHub Projects are Seyal's canonical execution system. Architecture/specifications stay in the repository; the Project is a view over Issues, never a second source of truth.

## Hierarchy

Use native GitHub hierarchy where available:

```text
Milestone / Epic
  ├─ Feature
  │   ├─ Task
  │   └─ Task
  ├─ Feature
  └─ Validation / benchmark
```

Use native sub-issues, dependencies, milestones, issue types, assignees and Project fields rather than Markdown TODO duplication.

## Project workflow

```text
Backlog → Refinement → Ready → In Progress → In Review → Validation → Done
                         ↘ Blocked ↗
```

Only **Ready** items may be picked up by implementation agents.

## Required implementation-Issue fields

Every implementation Issue must state:

- Goal
- Why this exists
- Architecture/spec references
- In scope
- Explicitly out of scope
- Dependencies / blocked-by
- Expected ownership/module boundaries
- Acceptance criteria
- Tests required
- Performance impact
- Memory impact
- Security impact
- Documentation impact
- Demo / verification procedure
- Definition of Done

`Documentation impact` must identify whether the change affects User Guide, Developer Guide, authoritative engineering docs, media/screenshots, or none. `None` is valid only with a reason.

## Ready gate

An Issue is Ready only when all are true:

- [ ] goal is unambiguous
- [ ] relevant architecture/spec exists
- [ ] dependencies are complete
- [ ] ownership boundary is known
- [ ] acceptance criteria are measurable
- [ ] test strategy is defined
- [ ] performance/security requirements are identified
- [ ] documentation impact is classified
- [ ] no unresolved architecture question remains

If any item is false, return the Issue to Refinement or Blocked. An agent must not silently fill the gap.

## Definition of Done

Done means applicable evidence exists, not merely that the feature appears to work. Select only relevant gates, with a higher bar for core terminal/runtime work:

- unit/integration/property tests
- VT byte fixtures/reference/conformance checks
- fuzzing/regression corpus
- PTY integration
- deterministic renderer verification
- latency/throughput/CPU/RSS/thread/GPU measurements
- failure injection
- security analysis
- documentation impact re-assessed against the final implementation
- affected user/developer/authority docs updated in the same Issue/PR, or a concrete `N/A` rationale recorded
- documentation validation (`make docs-check` / `make docs-build`) when site docs changed
- CI evidence
- reproducible demo/verification

Documentation is not considered complete merely because an Issue originally said `N/A`; implementation evidence can change the documentation impact and must be re-assessed before Done.

A correctness, latency, CPU or memory regression cannot be silently accepted. If a regression is intentional, it needs explicit documented approval/evidence at the correct authority level.

## Architecture-change trigger

An ADR is required when changing authority/ownership, PTY lifecycle, VT semantics/state model, renderer boundary, process/thread model, IPC/protocol architecture, persistence guarantees, Block semantics, headless/embed model, security boundary, public API/ABI, or OSS/commercial boundary.

Ordinary local implementation choices do not require an ADR.

## Scope changes

An active Issue may not absorb unrelated refactoring. Create/link another Issue. If the new finding invalidates architecture/spec/acceptance criteria, stop and run the architecture/spec change process before continuing.

## Independent validation

Architecture, VT/parser, persistence/process-lifetime, security-boundary and performance-sensitive changes require evidence beyond the implementation agent's self-report. Preferred flow:

```text
implementation agent → CI → independent human/review agent → merge
```

## M001

M001's dependency-safe decomposition is documented in `M001-DISTRIBUTION.md`. Detailed implementation Issues should be created only for the active pass/near-term dependency frontier, not for the full future roadmap.
