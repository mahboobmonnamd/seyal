---
name: architecture-change
description: Determine whether a Seyal change requires an ADR and prevent silent architecture drift.
---

# Architecture change

Use when implementation or design may alter authority/ownership, PTY lifecycle, VT semantics/state, renderer boundary, process/thread model, IPC/protocol architecture, persistence guarantees, Block semantics, headless/embed model, security boundary, public API/ABI, or OSS/commercial boundary.

1. Read `AGENTS.md`, `docs/architecture/README.md`, relevant rationale IDs/ADRs/specs, and `docs/engineering/ISSUE-PROTOCOL.md`.
2. State the observed evidence/problem and the exact accepted decision it conflicts with or proposes to change.
3. Decide whether this is ordinary implementation detail or architecture. When in doubt, prefer an R&D/ADR Issue over silent precedent.
4. For architecture: stop substantial implementation. Create/refine an Architecture/R&D Issue, collect alternatives and measurable/security evidence, and draft a scoped ADR citing affected rationale/authority.
5. Review/accept the ADR in its own PR before substantial implementation. Any create/amend/reopen/supersede of an ADR must be a separate PR from implementation code.
6. Update affected specifications/milestone/Issue after the ADR is accepted, then return the implementation Issue to Ready.

Never mix an ADR create/amendment with implementation in one PR. ADR PRs decide architecture; implementation PRs only consume already-accepted authority. Never justify a decision only because existing code already does it.
