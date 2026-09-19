---
name: implement-issue
description: Seyal facade for AI-SDLC implementation, adding mandatory GitHub issue claiming, plan-first confirmation, the one-Issue/worktree/PR workflow, and terminal-specific engineering gates.
---

# Implement Issue

This is the mandatory entrypoint for production implementation of a Seyal GitHub Issue. Requests such as implement, fix, finish, code, or complete a specific Issue must use this skill before production edits; do not bypass it by editing directly.

Follow the canonical generic procedure in `.sdlc/framework/skills/implementation/SKILL.md`. If it is unavailable, run `make bootstrap-agents` first.

## Auditable GitHub Issue claim

The exact remote `issue/<number>` Git ref is the exclusive race lock. New implementation Issues stay unassigned; never set, clear, or change GitHub assignees. Claim comments make ownership auditable but do not replace atomic branch creation. The preflight establishes eligibility before plan confirmation; the branch claim happens only after the plan is confirmed and a second fresh read passes.

1. Resolve the current implementer's **authenticated GitHub login** using `gh api user --jq .login`. Do not guess from git author name, OS username, chat name, or repository ownership. If authentication or identity cannot be verified uniquely, stop with `BLOCKED: implementation identity unavailable` and do not claim or edit.
2. Fetch the owning Issue fresh from GitHub. Require it to be open, its body `State` field explicitly to be Ready, and its Ready checklist to pass under `docs/engineering/ISSUE-PROTOCOL.md`. If linked Project items exist, read them too: a new pickup requires Ready status, while resuming a matching active claim may have a compatible in-progress status. No linked Project item is required. Inspect dependencies, the complete assignee list, existing claim/handoff comments, active PRs, and exact remote `issue/<number>` branch. If required Issue state or any present Project status is unavailable or ambiguous, fail closed.
3. Inspect, but do not mutate, the complete assignee list. An unassigned Ready Issue is eligible for a new claim. One assignee equal to the authenticated login may continue or resume only under the branch rules below. A different assignee retains ownership unless that assignee explicitly comments a handoff to the authenticated login; multiple assignees or an assignee/claim conflict stops work. Handoff never changes the assignee field.
4. If this is a child Issue, fresh-read its parent and dependencies. A planning parent's assignee does not lock an independent Ready child. Verify that parent/child scopes do not overlap active branch/PR work and that explicit dependencies are satisfied; stop on overlap or unresolved ownership. Do not claim or assign the parent on behalf of the child.
5. If `issue/<number>` already exists, do not create another branch or worktree. It may be resumed only by its recorded active claimant after an explicit user request to continue, by the named handoff recipient after a handoff from the recorded owner, or after an explicit maintainer resolution. A branch without a matching claim record, a legacy active branch, or any ambiguous state is `BLOCKED`; never create a suffix branch.
6. Before creating a new branch, record the execution plan as an Issue comment and explicitly confirm that it matches the Issue's Ready State, the Ready checklist, and accepted authority. The Issue body/comments are the durable plan and confirmation record; chat is not a prerequisite. Then re-fetch the Issue, any linked Project status, dependencies, assignees, parent/child state, claims, and exact remote branch. Fetch the current accepted `master` SHA for the claim base. If eligibility changed, stop before branch creation.
7. Atomically create `refs/heads/issue/<number>` through GitHub's create-reference API (`POST /repos/{owner}/{repo}/git/refs`) with the accepted base SHA. The first unambiguous successful creation is the only winner. An existing ref, failed creation, competing creation, or ambiguous response means stop; do not overwrite the ref or choose another name. If the create response is ambiguous, a matching SHA is not proof that this claimant won.
8. Immediately re-fetch the Issue, any linked Project/dependency state, assignees, and remote ref. Require the Issue still open with its body `State` field Ready, compatible Project/ownership state, and branch SHA equal to the recorded base. Then post and verify an Issue comment containing the authenticated claimant, exact branch, base SHA, and a link to the confirmed plan comment. If the post-create verification or comment fails, do not edit; leave the ref for explicit owner/maintainer resolution.
9. Create the isolated worktree from the verified exact branch only after the claim comment is present. The branch remains reserved through implementation and review.

Project status such as `In Progress` is lifecycle metadata and never substitutes for branch creation, dependency, or ownership verification.

## Plan first

Do not create the implementation worktree/branch, generate files, or start production edits until the implementation approach is recorded and confirmed on the owning GitHub Issue. The Issue body/comments are the durable authority; chat is not a prerequisite. Ready/claimed status is not permission to skip the plan.

1. Restate the owning Issue, in/out scope, production vs exploratory classification, and concrete production path in the Issue body/comment. Link the controlling spec/ADR and acceptance criteria.
2. Record an execution plan as an Issue comment before branch reservation. For changes across more than about three files or any new module/boundary, include the file/module ownership, sequence, tests/evidence, and risks. Confirm in the comment that the plan stays within the Ready Issue and accepted authority; the later branch-claim comment links this plan.
3. If the request is ambiguous or the Issue leaves a material choice open, do not assume scope. Ask and resolve it in the Issue body/comments. If that changes scope, architecture, dependency, or acceptance materially, return the Issue to Refinement and wait until it is Ready again.
4. After the plan comment is confirmed and the atomic branch claim succeeds, deliver execution-ready implementation. Do not leave scaffolds, placeholder modules, or outline-only trees as the result.
5. Flag uncertainty explicitly in the Issue rather than resolving it silently. If two approaches are viable, record the tradeoff there and wait for the Issue to be refined/resolved before claiming.
6. When iterating, make targeted corrections to the Issue plan and record the updated confirmation there. Do not rewrite the whole plan unless its authority or scope changed.

## Failure remediation loop

A reproducible failure discovered while implementing, validating, or closing the owning Issue is active engineering work. Recording or classifying the failure is not completion when the failure is fixable within the Issue's accepted scope.

For every reproducible failure:

1. Reproduce it with the narrowest deterministic test, fixture, workload, or native gate available.
2. Diagnose whether the root cause is product code, test/harness lifecycle or isolation, environment/setup, or an external platform limitation. Do not guess from a single green rerun.
3. If the root cause is within the owning Issue scope, implement the smallest production-grade fix immediately. Test/harness fixes are valid only when they make the test more truthful; never weaken, skip, retry-away, serialize-away, or relabel a valid failure merely to obtain green.
4. Rerun the narrow failing gate until stable, then rerun the applicable exact-head repository gates (`make check`, `make test`, native/XCUI, fuzz/bench/security as required) and CI.
5. Continue diagnose → fix → rerun until green. An isolated pass does not override a later combined/full-suite failure.
6. Create a blocking child Issue and stop only when diagnosis establishes that the required fix materially exceeds the owning Issue scope, changes an accepted architecture/authority boundary, requires a new non-local module/redesign, or belongs to a separate ownership boundary. Link the blocker and keep the original Issue open.
7. Hosted/cloud CI is the default development loop. A physical or dedicated platform machine is required only for gates that hosted CI cannot truthfully establish, such as hardware-specific interactive performance. Lack of a developer-owned physical machine is not itself a reason to stop normal implementation/debugging.

A known reproducible failure may be explicitly classified only after diagnosis. `ENVIRONMENT_UNSUPPORTED` / `PLATFORM_LIMITED` must identify the unavailable external capability and must not be used for an in-repository defect or test-lifecycle leak.

## Production-grade merge invariant

Anything that can reach `master` must be production-grade for its intended repository role. This applies to product code, developer tooling, scripts, fixtures, generated artifacts, and tests that are committed on a mergeable path.

- Mergeable implementation work must use the accepted permanent architecture and must be intended to remain, be maintained, and evolve in production/contributor use.
- Throwaway, demo-only, temporary, fake-data, prototype, spike, benchmark-experiment, or compatibility-bridge implementation code is never a merge candidate merely because it demonstrates progress or passes a narrow test.
- Exploratory code must remain on an explicitly non-mergeable R&D path. Useful findings may graduate only as independently valid tests, fixtures, measurements, documentation, or decision evidence; shipping code is implemented cleanly afterward through the normal Ready/implementation/review flow.
- Do not copy exploratory implementation wholesale into a production branch. Re-implement the accepted production solution cleanly so review can establish that every merged path is intentional and supportable.
- If a requested feature cannot yet be implemented production-grade because architecture or dependencies are unresolved, stop and route the uncertainty instead of creating a temporary production path.

## Branch-claim handoff and release

- A handoff is an Issue comment from the recorded owner naming the recipient and recording the same branch, current head/base SHA, PR/check state, and remaining plan. The recipient verifies it fresh, posts an acknowledgement, and resumes the existing branch. Do not create a replacement branch.
- A claim remains active through review. To release work without an active PR, the recorded owner or a maintainer must explicitly record release and remove the unused branch. Branch disappearance alone is not a release. Merged/deleted branches for still-open Issues may be re-reserved only by the recorded owner or after a maintainer resolves the claim.
- Claims never expire automatically. Report suspected stale claims to the recorded owner or a maintainer; agents may not self-clear, steal, overwrite, or bypass an existing claim.

Then apply only these Seyal-specific rules on top of the generic procedure:

1. The GitHub Issue must already be **Ready** under `docs/engineering/ISSUE-PROTOCOL.md`. Re-run `development-readiness` if scope, authority, dependencies, or acceptance changed materially.
2. Use one Issue → one authenticated branch-reserved claimant → one isolated worktree → deterministic `issue/<number>` → one scoped PR. Keep new implementation Issues unassigned and never modify assignees. When the work is a child slice, claim that Ready child only after verifying its own dependencies and non-overlapping parent/child scope; the parent need not be assigned or handed off when it is only a planning umbrella. If the user asked for a parent end-to-end outcome that still has multiple sub-issues, implement one independently Ready slice at a time and keep other slices on their own Issues/PRs.
3. Before implementation, classify the work as **production** or **exploratory**. Mergeable Issue branches are production only. A spike/prototype/POC must use an explicitly isolated non-mergeable branch/worktree and must never be promoted wholesale into `master`.
4. MVP is valid only when it is a narrow slice of the permanent architecture. Never add fake UI/data, temporary VT/renderer/runtime, duplicate state, alternate implementation, compatibility shim, feature-flag POC, or parallel old/new production path merely to demonstrate progress or bridge an unready dependency.
5. If the permanent production path is blocked by an unresolved dependency/architecture question, stop. Route to `development-readiness`, `architecture-change`, or isolated evidence work instead of coding a temporary production path.
6. Core behavior is test/evidence-first. Never add a temporary production VT, renderer, runtime, or duplicate-state path to make the Issue pass.
7. If implementation evidence conflicts with accepted architecture/specification, stop and run `architecture-change`; do not create architecture by precedent. Never create, amend, reopen, or supersede an ADR inside an implementation PR—land any ADR change in a separate Architecture/R&D PR first, update affected specs/Issues, then resume implementation against the accepted authority.
8. Invoke Seyal domain skills only when applicable: `vt-tdd`, `terminal-conformance`, `performance-gate`, `metal-renderer`, `rust-fuzzing`, `security-review`, macOS UI/accessibility skills, or others required by the Issue.
9. Re-assess documentation impact before handoff. Run `docs-authoring` when applicable; otherwise record a concrete `N/A` rationale.
10. Run the narrow checks continuously, then the required repository gates including `make check`; run issue-specific integration/fuzz/benchmark/security checks and `make docs-check` / `make docs-build` when documentation changed.
11. Every mergeable PR must name exactly one **owning Issue** in the PR's `## Issue` section. Use `Closes #N`, `Fixes #N`, or `Resolves #N` only when this PR, once merged, satisfies that owning Issue's acceptance criteria and Definition of Done. If the PR is refinement, evidence, a partial implementation, a prerequisite, or otherwise does not make the Issue Done, use a non-closing reference such as `Refs #N` or `Part of #N`. Never use a closing keyword merely because the PR works on the Issue.
12. Before opening the PR, compare the final diff/evidence against the owning Issue. If acceptance criteria changed during implementation, update/refine the Issue first; do not make the PR description silently redefine Done.
13. Open the PR with `.github/pull_request_template.md`, preserve the exact owning-Issue reference, and provide reproducible evidence. The implementation handoff is **implemented for review**, never final verification.
14. Do not self-approve core/high-risk work. Route next to `pr-review`, then `verification` as required.
15. At final verification/merge handoff, explicitly verify the owning Issue's state: a closing PR may close it only if all Done gates are evidenced; a non-closing PR must leave it open. Also correct stale Issue status/checklist text when it would contradict the verified state.

## Claim handoff and release

- **Normal completion:** keep the branch claim through review/validation; the Issue closes through the verified closing PR when all Done gates pass.
- **Explicit mid-work handoff:** current owner stops editing and records the recipient and exact branch/PR/check state in an Issue comment. The new implementer verifies and acknowledges that handoff, re-runs the Ready/dependency preflight, and resumes the same branch. Do not change assignees or create a second branch.
- **Abandoned before implementation:** the current owner or a maintainer records release and removes the unused branch. No assignee cleanup is performed.
- **Stale claim suspected:** never self-clear it. Report the claim and require explicit resolution by its recorded owner or a maintainer; there is no automatic expiry.

Useful findings from an isolated POC may be carried forward as measurements, docs, ADR evidence, fixtures, or independently valid tests. Production code must then be implemented cleanly from the accepted architecture/specification after readiness passes.

If a reusable implementation-rule defect is found, fix it in `ai-sdlc` rather than expanding this facade into a second generic implementation skill. The generic claim contract is tracked in `mahboobmonnamd/ai-sdlc#10`; this facade maps it to Seyal's authenticated GitHub identity, Ready gate, atomic deterministic-branch reservation, claim comments, and handoff rules. Do not edit generated `.sdlc/framework` material here.
