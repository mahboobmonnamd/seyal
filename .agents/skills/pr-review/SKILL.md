---
name: pr-review
description: Seyal facade for AI-SDLC code review, adding Seyal architecture, terminal hot-path, repository, and specialist evidence requirements.
---

# Pull request review

Follow the canonical generic procedure in `.sdlc/framework/skills/code-review/SKILL.md`. If it is unavailable, run `make bootstrap-agents` first.

Apply only these Seyal-specific rules on top of the generic procedure:

1. Review the linked GitHub Issue, `AGENTS.md`, and every governing architecture/ADR/spec/milestone source before accepting implementation rationale.
2. Enforce the Issue's in/out scope and Seyal's single-authoritative-state rules. Flag duplicate VT/grid/runtime ownership, temporary production paths, or architecture-by-precedent as blocking.
3. Treat new synchronous terminal hot-path dependencies, unnecessary IPC/serialization/allocations/locks/language round trips, or licensing/cloud coupling as architecture/performance concerns requiring explicit authority.
4. Inspect required terminal evidence where applicable: conformance fixtures, fuzz/regression corpus, PTY/integration/failure tests, renderer checks, and measured latency/CPU/RSS/GPU results.
5. Require `security-review`, documentation validation, macOS UI/accessibility evidence, or other specialist review only when the Issue's risk/impact classification requires it.
6. Verify tests were not weakened and claims do not exceed measurements. `make check`/CI success is required evidence, not sufficient proof by itself.
7. A clean review maps to AI-SDLC `APPROVE_FOR_VERIFICATION`; it does not mark the Issue or milestone Done. Route next to `verification`.
8. Core/high-risk work must retain the independent-review requirement in `ISSUE-PROTOCOL.md`.

If a reusable review-rule defect is found, fix it in `ai-sdlc` rather than duplicating generic code-review procedure here.
