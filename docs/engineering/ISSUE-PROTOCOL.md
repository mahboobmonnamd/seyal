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

## Production implementation vs POC

A small MVP is valid production work. A POC/spike/prototype is not.

```text
MVP
= deliberately small production scope
+ accepted permanent architecture
+ normal tests/review/evidence
+ mergeable when Done gates pass

POC / spike / prototype
= uncertainty-reduction experiment
+ isolated non-mergeable branch/worktree/environment
+ evidence may be retained
+ exploratory code never merges to master
```

Rules:

- If the permanent implementation is not Ready because architecture/dependencies are unresolved, do not create a temporary production implementation.
- Do not merge fake UI/data, temporary VT/renderer/runtime paths, duplicate state authorities, alternate implementations, feature-flag POCs, compatibility shims, or parallel old/new production paths merely to demonstrate progress.
- Useful experimental findings may become docs, measurements, ADR evidence, fixtures, or independently valid tests. Production code starts cleanly from accepted architecture/specification after readiness passes.
- If an experiment is later intended to ship, first reclassify/refine it as production work and run the full Ready/implementation/review/verification flow. Do not treat a successful POC branch as a merge candidate by default.
- Legitimate product presentations such as Flow/Raw/TUI may coexist only as presentations over the same authoritative terminal execution/state; they are not permission for competing terminal engines.

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
- [ ] the mergeable implementation is a permanent production path, not a POC/spike/temporary parallel implementation

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
- no exploratory/temporary/duplicate production path remains in the merge candidate

Documentation is not considered complete merely because an Issue originally said `N/A`; implementation evidence can change the documentation impact and must be re-assessed before Done.

A correctness, latency, CPU or memory regression cannot be silently accepted. If a regression is intentional, it needs explicit documented approval/evidence at the correct authority level.

## Architecture-change trigger

An ADR is required when changing authority/ownership, PTY lifecycle, VT semantics/state model, renderer boundary, process/thread model, IPC/protocol architecture, persistence guarantees, Block semantics, headless/embed model, security boundary, public API/ABI, or OSS/commercial boundary.

Ordinary local implementation choices do not require an ADR.

Any ADR create/amend/reopen/supersede must land in its own Architecture/R&D PR. Implementation PRs that amend ADRs are rejected; stop implementation, accept the ADR separately, update affected specs/Issues, then resume.

## Scope changes

An active Issue may not absorb unrelated refactoring. Create/link another Issue. If the new finding invalidates architecture/spec/acceptance criteria, stop and run the architecture/spec change process before continuing.

## PR → Issue closure contract

Every mergeable implementation PR has exactly one **owning Issue**. Supporting Issues may be referenced for context, but they are not silently treated as closure targets.

Use a GitHub closing keyword (`Closes #N`, `Fixes #N`, `Resolves #N`) only when all of the following are true:

- the referenced Issue is the PR's owning Issue;
- the final PR stays within that Issue's scope;
- the final implementation satisfies the Issue's acceptance criteria;
- all applicable Definition-of-Done evidence is present or explicitly approved as an allowed exception;
- merging the PR is expected to make the Issue genuinely **Done**.

Use a non-closing relationship (`Refs #N`, `Part of #N`, or equivalent plain reference) when the PR is any of the following:

- architecture/specification refinement;
- prerequisite or dependency work;
- benchmark/evidence gathering that does not complete the owning Issue;
- partial implementation;
- follow-up hardening where the parent Issue must remain open;
- any change whose merge must not move the Issue to Done.

Agents must not use a closing keyword merely because a PR "works on" an Issue. The PR description must not redefine the Issue's acceptance criteria to justify closure; if acceptance materially changes, refine the Issue first.

At final review/verification handoff, explicitly compare the final PR evidence against the owning Issue and record the expected post-merge state. If a closing PR still has an unmet Done gate, change it to a non-closing relationship and leave the Issue open. If a non-closing PR is merged, verify that the Issue remains open. When an Issue is genuinely completed, update stale status prose/checklists so the body does not continue to say `Ready`, `Refinement`, `In Review`, or otherwise contradict its verified state.

Historical imported issues labeled `historical-evidence` are evidence records, not executable current backlog. Their GitHub open/closed state must never be used as proof of current Seyal implementation. Current work must be represented by a Seyal-native owning Issue. Historical records may be archived/closed once their evidence/disposition role is complete, without implying that the capability is implemented or rejected in current Seyal.

## Independent validation

Architecture, VT/parser, persistence/process-lifetime, security-boundary and performance-sensitive changes require evidence beyond the implementation agent's self-report. Preferred flow:

```text
implementation agent → CI → independent human/review agent → merge
```

Review must block a merge candidate that contains POC/spike code, a disposable alternate implementation, or a second authoritative path that was not explicitly accepted as a production migration.

## M001

M001's dependency-safe decomposition is documented in `M001-DISTRIBUTION.md`. Detailed implementation Issues should be created only for the active pass/near-term dependency frontier, not for the full future roadmap.
