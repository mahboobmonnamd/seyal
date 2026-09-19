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

Use native sub-issues, dependencies, milestones, issue types, and (where a Project exists) Project fields rather than Markdown TODO duplication. Leave new implementation Issues unassigned. Existing assignee metadata is read-only under this workflow; existing work remains with its current owner unless that owner explicitly hands it off.

## Issue and optional Project workflow

The required lifecycle state is recorded in the GitHub Issue body under `## State` (or an equivalent explicit `State: ...` field); for example, `## State` followed by `Ready — ...` means the Issue's State field is Ready. Valid values are Backlog, Refinement, Ready, In Progress, In Review, Validation, Blocked, and Done. A linked GitHub Project is an optional view of that lifecycle, not a prerequisite for implementation. Keep its status aligned when an Issue is linked.

```text
Backlog → Refinement → Ready → In Progress → In Review → Validation → Done
                         ↘ Blocked ↗
```

Only open Issues whose body `State` field is explicitly **Ready** and whose Ready checklist passes may be picked up by implementation agents. If linked Project items exist, a new pickup requires their workflow status to be Ready. For an existing matching claim, a lifecycle status such as In Progress is compatible only with resuming that claim. No linked Project item is required.

## Active implementation claim

Seyal implementation work is exclusively owned while active. For new work the exact remote `issue/<number>` Git ref is the atomic claim; GitHub assignees are never used as a lock and are never set, cleared, or changed by this workflow. An Issue comment records the claim for people and agents, but comments and Project status do not provide mutual exclusion.

The pickup contract is:

```text
fresh open Ready Issue and dependencies
→ authenticated login from `gh api user`
→ inspect assignees, parent/child scope, claim comments, and exact branch
→ execution plan and confirmation recorded in Issue comment
→ fresh Ready/branch read and accepted master base SHA
→ atomic create of remote issue/<number>
→ fresh Issue/branch verification and claim comment linking the plan
→ isolated worktree
→ implementation may begin
```

Rules:

- Production implementation of a GitHub Issue must enter through `.agents/skills/implement-issue/SKILL.md`.
- Resolve the authenticated GitHub login using `gh api user --jq .login`. Never infer identity from git author configuration, local username, chat name, or repository ownership. If authentication or identity lookup is unavailable or ambiguous, fail closed.
- Read the owning Issue fresh immediately before pickup. Require that it is open, its body `State` field is explicitly Ready, and every Ready checkbox below passes. Read any linked Project items too: a new pickup requires their workflow status to be Ready; when resuming a matching active claim, a status such as In Progress is compatible only with that claim. Absence of a linked Project item is not a blocker. Also read dependencies, parent/child relationships, the complete assignee list, claim/handoff comments, and existing implementation PRs/branches. Cached context is not authority. If required Issue state or any present Project status cannot be verified, do not reserve a branch or edit.
- Read assignee state but never call an assignment or unassignment API. A new unassigned Ready Issue is eligible. Existing assigned work remains with its current owner: an Issue with one different assignee is blocked unless that assignee explicitly records a handoff to the authenticated login; an Issue assigned to the current login may be resumed by that owner. Multiple assignees or conflict between assignee and active claim fail closed. An explicit handoff does not change the assignee field.
- A parent Issue's assignee or planning branch does not automatically lock an independent child. Fresh-read the parent and child, verify the child is Ready and its explicit dependencies are satisfied, and ensure its scope does not overlap active parent/child work. A conflicting active branch or unresolved boundary blocks the child until the owners or a maintainer resolve it.
- Issue `State` and any Project status are lifecycle metadata, never the ownership lock. Ready cannot override an existing branch claim, active handoff, dependency, or ownership conflict.
- Before branch creation, record an execution-plan comment on the Issue and explicitly confirm there that the plan matches the Ready Issue and accepted authority. The Issue body/comments are the durable plan and confirmation record; chat is not a prerequisite. The Issue's Ready status means its scope and acceptance are approved. If the execution plan exposes a material unresolved scope or architecture decision, return the Issue to Refinement and resolve it on the Issue before pickup.
- After the Issue plan comment is confirmed, re-read the Issue, any linked Project status, dependencies, assignees, and exact remote branch. Fetch the current accepted `master` SHA as the claim's base. Do not create the branch before the plan is confirmed in the Issue.
- Create the exact ref `refs/heads/issue/<number>` through GitHub's atomic Git reference creation endpoint (for example, `gh api --method POST "repos/{owner}/{repo}/git/refs" -f "ref=refs/heads/issue/<number>" -f "sha=<base-sha>"`). Only an unambiguous successful create-ref response wins the race. If the ref exists, another claimant wins, the create request fails, or its result is ambiguous, stop; do not overwrite, infer ownership, or select a suffix branch. An ambiguous create must be resolved by the recorded claim owner or a maintainer even if the ref points at the proposed base SHA.
- Immediately after a successful create, fresh-read the Issue, any linked Project items, dependencies, and the remote ref. Require the Issue to remain open with its body `State` field still Ready, any present Project status to remain compatible, the assignment state to remain compatible, and the ref to point to the recorded base SHA. Then post an Issue comment recording the authenticated claimant, exact branch, base SHA, and a link to the confirmed plan comment. If a post-create check or comment fails, do no production work and report the reserved ref for explicit resolution; do not automatically delete it.
- After the verified claim comment, set the Issue body field `State` to **In Progress** and any linked Project item's workflow status to the matching value. Verify that lifecycle update before starting the worktree or production edits. If the update fails or its result is ambiguous, stop and report the reserved branch; do not change assignees or release the branch automatically.
- If `issue/<number>` already exists, do not start a second worktree or implementation. Resume only when the authenticated identity matches the recorded active claimant and the user explicitly requested continuation, or the recorded owner explicitly handed off the existing branch to this login, or a maintainer explicitly resolved a stale claim. Re-run the full Ready/dependency preflight, verify the remote branch/head, and record resumption before editing. A branch without a matching claim record is unresolved and fails closed.
- Legacy `issue/<number>-<short-name>` branches may finish only under their existing owner. Treat an active legacy branch as a collision for a new pickup; do not create the deterministic branch in parallel and do not create another suffix branch.

### Ownership handoff and release

A handoff or release is explicit, never inferred from inactivity, a status change, or branch disappearance.

- The current branch owner stops editing and comments with the recipient (for handoff), exact branch/head, base SHA, PR/check state, remaining plan, and whether work is being handed off or released. A handoff keeps the same branch; the recipient fresh-reads state, verifies the comment author/recipient and branch, then acknowledges in an Issue comment before resuming. No assignee field changes.
- A claim remains active through implementation and review. To release work with no active PR, the current owner or a maintainer must explicitly record release and remove the unused claim branch; branch deletion without a release comment does not by itself authorize another pickup. If a merged or externally deleted branch disappears while the Issue remains open, only the recorded owner may re-reserve it or a maintainer may resolve the claim.
- There is no automatic claim expiry. A suspected stale claim is reported to its recorded owner or a maintainer. Only that owner may hand off/release, or a maintainer may document a stale-claim resolution; no agent may self-clear, steal, overwrite, or bypass the claim.

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

An implementation Issue is Ready only when it is open, its body `State` field is explicitly Ready, and all of the following are true:

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
