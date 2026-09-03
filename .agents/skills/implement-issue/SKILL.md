---
name: implement-issue
description: Seyal facade for AI-SDLC implementation, adding plan-first confirmation, the one-Issue/worktree/PR workflow, and terminal-specific engineering gates.
---

# Implement Issue

Follow the canonical generic procedure in `.sdlc/framework/skills/implementation/SKILL.md`. If it is unavailable, run `make bootstrap-agents` first.

## Plan first

Do not create the worktree, generate files, or start production edits until the implementation approach is confirmed in chat. Ready status is not permission to skip the plan.

1. Restate the owning Issue, in/out scope, production vs exploratory classification, and the concrete production path you will change.
2. If the request is ambiguous or the Issue leaves a material choice open, ask before assuming scope. Do not silently pick architecture, file layout, or extra work.
3. If the work needs more than about three file changes, or any new module/boundary, outline the plan in chat first: files, tests/evidence, and risks. Wait for confirmation before generating files.
4. After the plan is confirmed, deliver execution-ready implementation. Do not leave scaffolds, placeholder modules, or outline-only trees as the result.
5. Flag uncertainty explicitly rather than resolving it silently. If two approaches are viable, state the tradeoff and ask.
6. When iterating, make targeted corrections to the agreed plan. Do not rewrite the whole change unless the plan itself changed.

Then apply only these Seyal-specific rules on top of the generic procedure:

1. The GitHub Issue must already be **Ready** under `docs/engineering/ISSUE-PROTOCOL.md`. Re-run `development-readiness` if scope, authority, dependencies, or acceptance changed materially.
2. After the plan is confirmed, use one Issue → one isolated worktree → `issue/<number>-<short-name>` → one scoped PR.
3. Before implementation, classify the work as **production** or **exploratory**. Mergeable Issue branches are production only. A spike/prototype/POC must use an explicitly isolated non-mergeable branch/worktree and must never be promoted wholesale into `master`.
4. MVP is valid only when it is a narrow slice of the permanent architecture. Never add fake UI/data, temporary VT/renderer/runtime, duplicate state, alternate implementation, compatibility shim, feature-flag POC, or parallel old/new production path merely to demonstrate progress or bridge an unready dependency.
5. If the permanent production path is blocked by an unresolved dependency/architecture question, stop. Route to `development-readiness`, `architecture-change`, or isolated evidence work instead of coding a temporary production path.
6. Core behavior is test/evidence-first. Never add a temporary production VT, renderer, runtime, or duplicate-state path to make the Issue pass.
7. If implementation evidence conflicts with accepted architecture/specification, stop and run `architecture-change`; do not create architecture by precedent.
8. Invoke Seyal domain skills only when applicable: `vt-tdd`, `terminal-conformance`, `performance-gate`, `metal-renderer`, `rust-fuzzing`, `security-review`, macOS UI/accessibility skills, or others required by the Issue.
9. Re-assess documentation impact before handoff. Run `docs-authoring` when applicable; otherwise record a concrete `N/A` rationale.
10. Run the narrow checks continuously, then the required repository gates including `make check`; run issue-specific integration/fuzz/benchmark/security checks and `make docs-check` / `make docs-build` when documentation changed.
11. Every mergeable PR must name exactly one **owning Issue** in the PR's `## Issue` section. Use `Closes #N`, `Fixes #N`, or `Resolves #N` only when this PR, once merged, satisfies that owning Issue's acceptance criteria and Definition of Done. If the PR is refinement, evidence, a partial implementation, a prerequisite, or otherwise does not make the Issue Done, use a non-closing reference such as `Refs #N` or `Part of #N`. Never use a closing keyword merely because the PR works on the Issue.
12. Before opening the PR, compare the final diff/evidence against the owning Issue. If acceptance criteria changed during implementation, update/refine the Issue first; do not make the PR description silently redefine Done.
13. Open the PR with `.github/pull_request_template.md`, preserve the exact owning-Issue reference, and provide reproducible evidence. The implementation handoff is **implemented for review**, never final verification.
14. Do not self-approve core/high-risk work. Route next to `pr-review`, then `verification` as required.
15. At final verification/merge handoff, explicitly verify the owning Issue's state: a closing PR may close it only if all Done gates are evidenced; a non-closing PR must leave it open. Also correct stale Issue status/checklist text when it would contradict the verified state.

Useful findings from an isolated POC may be carried forward as measurements, docs, ADR evidence, fixtures, or independently valid tests. Production code must then be implemented cleanly from the accepted architecture/specification after readiness passes.

If a reusable implementation-rule defect is found, fix it in `ai-sdlc` rather than expanding this facade into a second generic implementation skill.
