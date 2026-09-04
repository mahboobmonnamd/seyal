---
name: code-review
description: Seyal adapter for focused AI-SDLC implementation/diff review with terminal architecture, hot-path, repository, and specialist-risk rules.
---

# Code review

Follow the canonical focused procedure in `.sdlc/framework/skills/code-review/SKILL.md`. If it is unavailable, run `make bootstrap-agents` first.

Use this skill when the requested question is specifically whether an implementation/diff contains defects, regressions, unsafe behavior, architecture drift, weakened tests, or evidence problems. It is not the final merge-readiness gate; use `pr-review` for that.

Apply only these Seyal-specific rules on top of the generic procedure:

1. Read the linked owning Issue, `AGENTS.md`, and governing architecture/ADR/spec/milestone sources before accepting implementation rationale.
2. Enforce the Issue's in/out scope and Seyal's single-authoritative-state rules. Duplicate PTY/VT/grid/runtime authority, temporary production paths, or architecture-by-precedent are blocking. Any ADR create/amend/reopen/supersede in an implementation PR is blocking; ADR changes must land in a separate Architecture/R&D PR first.
3. Treat new synchronous terminal hot-path dependencies, unnecessary IPC/serialization/allocations/locks/language round trips, or licensing/cloud coupling as architecture/performance risks requiring explicit authority and evidence.
4. Inspect affected production paths beyond the diff when surrounding lifecycle, concurrency, ownership, failure or backpressure state controls correctness.
5. Inspect applicable conformance, fuzz/regression, PTY/integration/failure, renderer/native, benchmark, security/privacy and macOS/accessibility tests instead of trusting test names or green CI alone.
6. Require specialist review only where the Issue/risk classification makes it material; do not duplicate specialist procedures in this adapter.
7. A clean result maps to generic `APPROVE_FOR_VERIFICATION`. It does not mean the PR is ready to merge and it does not mark the Issue/milestone Done.
8. Core/high-risk work must retain the independent-review requirement in `docs/engineering/ISSUE-PROTOCOL.md`.

If a reusable review-rule defect is found, fix it in `ai-sdlc` rather than duplicating generic code-review procedure here.
