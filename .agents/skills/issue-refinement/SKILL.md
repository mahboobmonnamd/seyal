---
name: issue-refinement
description: Turn an accepted Seyal milestone or feature into implementation-ready GitHub Issues without changing architecture or duplicating planning systems.
---

# Issue refinement

Read `AGENTS.md`, `docs/engineering/ISSUE-PROTOCOL.md`, the applicable architecture/ADR/spec/milestone documents, and existing related Issues.

For each proposed implementation Issue:

1. Define one coherent independently reviewable outcome.
2. State Goal and Why.
3. Link exact authority documents/sections.
4. Define in-scope and explicit out-of-scope work.
5. Identify native GitHub dependencies/sub-issues and the owning module/state boundary.
6. Write measurable acceptance criteria.
7. Define tests first for core behavior; include fixture/conformance/fuzz needs where relevant.
8. Identify performance, memory, security and documentation impact.
9. Give a reproducible demo/verification procedure and Definition of Done.
10. Check parallelizability: do not allow concurrent mutation of the same authoritative subsystem unless independence is proven.

Mark **Ready** only when every readiness checkbox in `ISSUE-PROTOCOL.md` passes. Otherwise leave in Refinement/Blocked and state the missing authority/evidence. Never invent missing architecture or broaden scope to make the Issue executable.
