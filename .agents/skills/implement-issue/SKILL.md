---
name: implement-issue
description: Mandatory workflow for implementing one Ready Seyal GitHub Issue in an isolated branch/worktree with test-first validation and a scoped PR.
---

# Implement Issue

1. Read `AGENTS.md`, `docs/engineering/DEVELOPMENT.md`, the Issue, and every authority reference it links.
2. Verify Project status is **Ready**, dependencies are complete, and module/state ownership is explicit. Stop if not.
3. Confirm one Issue → one isolated worktree → `issue/<number>-<short-name>` → one PR.
4. Write/enable the required failing test, fixture or verification first for core behavior.
5. Implement only Issue scope. Do not add unrelated cleanup, speculative abstractions, or temporary production VT/render/runtime paths.
6. If implementation evidence conflicts with architecture/spec: stop; run `architecture-change`; update authority/spec/Issue before resuming.
7. Run the smallest relevant checks continuously, then `make check` and issue-specific integration/fuzz/benchmark/security checks.
8. Compare performance/memory baselines when the Issue is performance-sensitive. Do not make unsupported claims.
9. Open a PR using `.github/pull_request_template.md`; link the Issue and provide reproducible verification evidence.
10. Do not self-approve high-risk/core work. Wait for CI and independent review required by `ISSUE-PROTOCOL.md`.

Never weaken a valid test to make implementation pass. Never infer architecture from existing code when authority documents say otherwise.
