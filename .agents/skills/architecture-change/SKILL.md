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
5. Review/accept the ADR independently before substantial implementation.
6. Update affected specifications/milestone/Issue after the ADR, then return the implementation Issue to Ready.

Never mix a major architecture decision and a large implementation in one opaque PR. Never justify a decision only because existing code already does it.
