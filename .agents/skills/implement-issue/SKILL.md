---
name: implement-issue
description: Mandatory workflow for implementing one Ready Seyal GitHub Issue in an isolated branch/worktree with test-first validation, documentation assessment, and a scoped PR.
---

# Implement Issue

1. Read `AGENTS.md`, `docs/engineering/DEVELOPMENT.md`, the Issue, and every authority reference it links.
2. Verify Project status is **Ready**, dependencies are complete, and module/state ownership is explicit. Stop if not.
3. Confirm one Issue → one isolated worktree → `issue/<number>-<short-name>` → one PR.
4. Write/enable the required failing test, fixture or verification first for core behavior.
5. Implement only Issue scope. Do not add unrelated cleanup, speculative abstractions, or temporary production VT/render/runtime paths.
6. If implementation evidence conflicts with architecture/spec: stop; run `architecture-change`; update authority/spec/Issue before resuming.
7. Before final validation, assess the Issue's **Documentation impact**. Run `docs-authoring` whenever the change adds or alters user-visible behavior, configuration, workflows, troubleshooting, contributor workflow, architecture orientation, screenshots/diagrams, or documentation media. Update the User Guide and/or Developer Guide in the same Issue/PR when applicable. If no documentation is required, record a concrete `N/A` rationale in the PR instead of silently skipping it.
8. Run the smallest relevant checks continuously, then `make check` and issue-specific integration/fuzz/benchmark/security checks. When documentation changed, also run `make docs-check` and `make docs-build`.
9. Compare performance/memory baselines when the Issue is performance-sensitive. Do not make unsupported claims.
10. Open a PR using `.github/pull_request_template.md`; link the Issue, list documentation changed (or the `N/A` rationale), and provide reproducible verification evidence.
11. Do not self-approve high-risk/core work. Wait for CI and independent review required by `ISSUE-PROTOCOL.md`.

Documentation is part of feature completeness, not a default follow-up task. Never document planned behavior as shipped merely to satisfy the documentation gate.

Never weaken a valid test to make implementation pass. Never infer architecture from existing code when authority documents say otherwise.
