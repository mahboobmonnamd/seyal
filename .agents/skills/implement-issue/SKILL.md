---
name: implement-issue
description: Seyal facade for AI-SDLC implementation, adding the one-Issue/worktree/PR workflow and terminal-specific engineering gates.
---

# Implement Issue

Follow the canonical generic procedure in `.sdlc/framework/skills/implementation/SKILL.md`. If it is unavailable, run `make bootstrap-agents` first.

Apply only these Seyal-specific rules on top of the generic procedure:

1. The GitHub Issue must already be **Ready** under `docs/engineering/ISSUE-PROTOCOL.md`. Re-run `development-readiness` if scope, authority, dependencies, or acceptance changed materially.
2. Use one Issue → one isolated worktree → `issue/<number>-<short-name>` → one scoped PR.
3. Core behavior is test/evidence-first. Never add a temporary production VT, renderer, runtime, or duplicate-state path to make the Issue pass.
4. If implementation evidence conflicts with accepted architecture/specification, stop and run `architecture-change`; do not create architecture by precedent.
5. Invoke Seyal domain skills only when applicable: `vt-tdd`, `terminal-conformance`, `performance-gate`, `metal-renderer`, `rust-fuzzing`, `security-review`, macOS UI/accessibility skills, or others required by the Issue.
6. Re-assess documentation impact before handoff. Run `docs-authoring` when applicable; otherwise record a concrete `N/A` rationale.
7. Run the narrow checks continuously, then the required repository gates including `make check`; run issue-specific integration/fuzz/benchmark/security checks and `make docs-check` / `make docs-build` when documentation changed.
8. Open the PR with `.github/pull_request_template.md`, trace it to the Issue, and provide reproducible evidence. The implementation handoff is **implemented for review**, never final verification.
9. Do not self-approve core/high-risk work. Route next to `pr-review`, then `verification` as required.

If a reusable implementation-rule defect is found, fix it in `ai-sdlc` rather than expanding this facade into a second generic implementation skill.
