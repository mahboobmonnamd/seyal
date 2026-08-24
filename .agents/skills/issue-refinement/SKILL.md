---
name: issue-refinement
description: Seyal facade for AI-SDLC work-item design, adding GitHub Issue, terminal-engine, documentation, and repository-specific readiness rules.
---

# Issue refinement

Follow the canonical generic procedure in `.sdlc/framework/skills/work-item-design/SKILL.md`. If it is unavailable, run `make bootstrap-agents` first.

Apply only these Seyal-specific rules on top of the generic procedure:

1. GitHub Issues + Projects are Seyal's execution system; use native dependencies/sub-issues and the required fields in `docs/engineering/ISSUE-PROTOCOL.md`.
2. Link exact accepted architecture/ADR/spec/milestone authority. Existing code and `.sdlc` summaries are never architectural authority.
3. Preserve one coherent independently reviewable outcome and the owning module/state boundary. Do not bundle unrelated cleanup or cross-authority work.
4. For terminal/runtime work, classify required unit/integration/fixture/conformance/fuzz/failure/performance evidence and identify any applicable domain skill such as `vt-tdd`, `terminal-conformance`, `performance-gate`, `metal-renderer`, or `security-review`.
5. Classify performance, memory, security, and documentation impact. `Documentation impact: none` requires a concrete reason.
6. Respect the active milestone/dependency frontier; do not pre-create speculative downstream implementation work merely to fill a roadmap.
7. After the work item is designed, run the `development-readiness` skill. Set Project status to **Ready** only when its generic verdict is `READY` and every Seyal Ready checkbox in `ISSUE-PROTOCOL.md` also passes.

If generic AI-SDLC behavior is insufficient, record the reusable defect in `ai-sdlc`; do not permanently fork the generic procedure here.
