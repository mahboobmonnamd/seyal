---
name: implement-issue
description: Seyal facade for AI-SDLC implementation, adding the one-Issue/worktree/PR workflow and terminal-specific engineering gates.
---

# Implement Issue

Follow the canonical generic procedure in `.sdlc/framework/skills/implementation/SKILL.md`. If it is unavailable, run `make bootstrap-agents` first.

Apply only these Seyal-specific rules on top of the generic procedure:

1. The GitHub Issue must already be **Ready** under `docs/engineering/ISSUE-PROTOCOL.md`. Re-run `development-readiness` if scope, authority, dependencies, or acceptance changed materially.
2. Use one Issue → one isolated worktree → `issue/<number>-<short-name>` → one scoped PR.
3. Before implementation, classify the work as **production** or **exploratory**. Mergeable Issue branches are production only. A spike/prototype/POC must use an explicitly isolated non-mergeable branch/worktree and must never be promoted wholesale into `master`.
4. MVP is valid only when it is a narrow slice of the permanent architecture. Never add fake UI/data, temporary VT/renderer/runtime, duplicate state, alternate implementation, compatibility shim, feature-flag POC, or parallel old/new production path merely to demonstrate progress or bridge an unready dependency.
5. If the permanent production path is blocked by an unresolved dependency/architecture question, stop. Route to `development-readiness`, `architecture-change`, or isolated evidence work instead of coding a temporary production path.
6. Core behavior is test/evidence-first. Never add a temporary production VT, renderer, runtime, or duplicate-state path to make the Issue pass.
7. If implementation evidence conflicts with accepted architecture/specification, stop and run `architecture-change`; do not create architecture by precedent.
8. Invoke Seyal domain skills only when applicable: `vt-tdd`, `terminal-conformance`, `performance-gate`, `metal-renderer`, `rust-fuzzing`, `security-review`, macOS UI/accessibility skills, or others required by the Issue.
9. Re-assess documentation impact before handoff. Run `docs-authoring` when applicable; otherwise record a concrete `N/A` rationale.
10. Run the narrow checks continuously, then the required repository gates including `make check`; run issue-specific integration/fuzz/benchmark/security checks and `make docs-check` / `make docs-build` when documentation changed.
11. Open the PR with `.github/pull_request_template.md`, trace it to the Issue, and provide reproducible evidence. The implementation handoff is **implemented for review**, never final verification.
12. Do not self-approve core/high-risk work. Route next to `pr-review`, then `verification` as required.

Useful findings from an isolated POC may be carried forward as measurements, docs, ADR evidence, fixtures, or independently valid tests. Production code must then be implemented cleanly from the accepted architecture/specification after readiness passes.

If a reusable implementation-rule defect is found, fix it in `ai-sdlc` rather than expanding this facade into a second generic implementation skill.
