---
name: implement-issue
description: Seyal facade for AI-SDLC implementation, adding mandatory GitHub issue claiming, plan-first confirmation, the one-Issue/worktree/PR workflow, and terminal-specific engineering gates.
---

# Implement Issue

This is the mandatory entrypoint for production implementation of a Seyal GitHub Issue. Requests such as implement, fix, finish, code, or complete a specific Issue must use this skill before production edits; do not bypass it by editing directly.

Follow the canonical generic procedure in `.sdlc/framework/skills/implementation/SKILL.md`. If it is unavailable, run `make bootstrap-agents` first.

## Exclusive GitHub Issue claim

Claiming the Issue is a coordination preflight, not implementation permission. Perform it before planning, worktree/branch creation, generated files, or production edits.

1. Resolve the current implementer's **authenticated GitHub login** using the project-approved GitHub tooling. Do not guess from git author name, OS username, chat name, or repository owner. If the authenticated login cannot be resolved uniquely, stop with `BLOCKED: implementation identity unavailable` and do not claim or edit the Issue.
2. Fetch the owning GitHub Issue fresh from GitHub immediately before pickup. Cached project context, chat state, an earlier fetch, or the Issue body alone is not sufficient for assignee state.
3. Verify the Issue is still open and **Ready** under `docs/engineering/ISSUE-PROTOCOL.md`. Readiness and ownership are separate gates.
4. Inspect the complete assignee list:
   - **No assignee:** assign exactly the authenticated current implementer, then fetch the Issue again.
   - **Exactly the current implementer:** treat this only as a potential resume; continue the collision checks below.
   - **Exactly another implementer:** stop before planning or edits and report `Issue #N is already taken by @login` with the Issue URL.
   - **Multiple assignees:** stop as an ownership collision. Seyal implementation Issues have exactly one active implementer.
5. After any assignment write, re-fetch the Issue and require the current implementer to be the **sole** assignee. A failed write, overwritten assignment, multiple assignees, or ambiguous result is `BLOCKED`; never overwrite another valid claim to win a race.
6. Do not clear, replace, or steal another implementer's assignment. Ownership transfer requires an explicit handoff/reassignment under `ISSUE-PROTOCOL.md`.

The GitHub assignee is the human-visible ownership claim. Project status such as `In Progress` is lifecycle metadata and must never substitute for the assignee check.

## Plan first

Do not create the implementation worktree/branch, generate files, or start production edits until the implementation approach is confirmed in chat. Ready/claimed status is not permission to skip the plan.

1. Restate the owning Issue, in/out scope, production vs exploratory classification, and the concrete production path you will change.
2. If the request is ambiguous or the Issue leaves a material choice open, ask before assuming scope. Do not silently pick architecture, file layout, or extra work.
3. If the work needs more than about three file changes, or any new module/boundary, outline the plan in chat first: files, tests/evidence, and risks. Wait for confirmation before generating files.
4. After the plan is confirmed, deliver execution-ready implementation. Do not leave scaffolds, placeholder modules, or outline-only trees as the result.
5. Flag uncertainty explicitly rather than resolving it silently. If two approaches are viable, state the tradeoff and ask.
6. When iterating, make targeted corrections to the agreed plan. Do not rewrite the whole change unless the plan itself changed.

## Production-grade merge invariant

Anything that can reach `master` must be production-grade for its intended repository role. This applies to product code, developer tooling, scripts, fixtures, generated artifacts, and tests that are committed on a mergeable path.

- Mergeable implementation work must use the accepted permanent architecture and must be intended to remain, be maintained, and evolve in production/contributor use.
- Throwaway, demo-only, temporary, fake-data, prototype, spike, benchmark-experiment, or compatibility-bridge implementation code is never a merge candidate merely because it demonstrates progress or passes a narrow test.
- Exploratory code must remain on an explicitly non-mergeable R&D path. Useful findings may graduate only as independently valid tests, fixtures, measurements, documentation, or decision evidence; shipping code is implemented cleanly afterward through the normal Ready/implementation/review flow.
- Do not copy exploratory implementation wholesale into a production branch. Re-implement the accepted production solution cleanly so review can establish that every merged path is intentional and supportable.
- If a requested feature cannot yet be implemented production-grade because architecture or dependencies are unresolved, stop and route the uncertainty instead of creating a temporary production path.

## Deterministic branch collision backstop

After the plan is confirmed but before creating the worktree or editing production files, use the exact remote branch name `issue/<number>` for new implementation pickups.

1. Fetch remote refs immediately before branch creation.
2. If `origin/issue/<number>` already exists, **do not create another implementation worktree or alternate branch**. Stop and report that the Issue has an active/resumable branch. Resume that branch only when the user explicitly asked to continue/resume the existing work and the fresh GitHub Issue read still shows the current implementer as sole assignee.
3. If the branch does not exist, create `issue/<number>` from the current accepted `master`. Branch creation is the collision backstop: failure because the ref appeared concurrently means another pickup won the race; stop rather than selecting a different branch name.
4. Immediately after successfully creating the branch, fetch the Issue again and require the current implementer to remain the sole assignee. If assignment and branch state disagree, stop before production edits and surface the collision for explicit resolution.
5. Create the isolated worktree from that exact branch only after both the assignee claim and deterministic branch checks pass.

Legacy implementation branches already created as `issue/<number>-<short-name>` may be completed under their existing owning Issue. Do not create new branches in that legacy form after this rule is merged.

Then apply only these Seyal-specific rules on top of the generic procedure:

1. The GitHub Issue must already be **Ready** under `docs/engineering/ISSUE-PROTOCOL.md`. Re-run `development-readiness` if scope, authority, dependencies, or acceptance changed materially.
2. Use one Issue → one sole GitHub assignee → one isolated worktree → deterministic `issue/<number>` → one scoped PR.
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

- **Normal completion:** keep the sole assignee through review/validation so ownership remains visible; the Issue closes through the verified closing PR.
- **Explicit mid-work handoff:** current owner stops editing, records the exact branch/PR/check state, and the Issue is explicitly reassigned to the new GitHub login. The new implementer re-runs the full claim/readiness preflight and resumes the existing `issue/<number>` branch; no second branch is created.
- **Abandoned before implementation:** remove the unused deterministic branch if it was created, then explicitly unassign/reassign the Issue. Do not leave an assignee or branch that falsely advertises active work.
- **Stale claim suspected:** never self-clear it. Report the assignee/branch and require explicit ownership resolution.

Useful findings from an isolated POC may be carried forward as measurements, docs, ADR evidence, fixtures, or independently valid tests. Production code must then be implemented cleanly from the accepted architecture/specification after readiness passes.

If a reusable implementation-rule defect is found, fix it in `ai-sdlc` rather than expanding this facade into a second generic implementation skill. The generic exclusive-claim contract is tracked in `mahboobmonnamd/ai-sdlc#10`; this facade owns only Seyal's GitHub-specific identity, assignee, deterministic-branch, and handoff mapping.
