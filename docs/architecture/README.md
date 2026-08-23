# Seyal Architecture

This directory is the canonical entry point for Seyal foundation architecture.

## Read in this order

1. [`SEYAL-ARCH-FOUNDATION-RD-001.md`](SEYAL-ARCH-FOUNDATION-RD-001.md) — canonical foundation architecture and Milestone-001 decision package.
2. [`rationale/SEYAL-ARCH-FOUNDATION-RATIONALE-001.md`](rationale/SEYAL-ARCH-FOUNDATION-RATIONALE-001.md) — reasons, rejected alternatives, failure modes, and revisit conditions for foundation decisions and prohibitions.
3. [`ADR-001-LOCAL-DISPLAY-PROJECTION.md`](ADR-001-LOCAL-DISPLAY-PROJECTION.md) — accepted local macOS display-projection mechanism for M001.
4. [`ADR-002-M001-READINESS-CORRECTIONS.md`](ADR-002-M001-READINESS-CORRECTIONS.md) — accepted pre-implementation corrections resolving BlockTimeline ownership, foundation acceptance state, configuration/Lua M001 scope, VT conformance, and local Runtime security gates.
5. [`../milestones/MILESTONE-001.md`](../milestones/MILESTONE-001.md) — authoritative M001 implementation slice.
6. [`../milestones/MILESTONE-001-READINESS-AMENDMENT-001.md`](../milestones/MILESTONE-001-READINESS-AMENDMENT-001.md) — required M001 acceptance-gate additions from the independent readiness review.
7. [`ui/SEYAL-UI-ARCHITECTURE-001.md`](ui/SEYAL-UI-ARCHITECTURE-001.md) — presentation architecture for Flow/Raw/TUI, history, Blocks, workspace chrome, inspectors, attention/approvals, desktop/mobile continuity, and render priority.
8. [`source/FOUNDATION-RD-BRIEF.md`](source/FOUNDATION-RD-BRIEF.md) — source requirements that initiated this architecture pass.

## Authority

- The foundation architecture is **accepted** and is the canonical foundation decision document.
- The rationale document explains **why** each rule exists. It does not create a competing architecture.
- Accepted ADRs refine specific foundation decisions and supersede older conflicting wording only for the decision they explicitly amend.
- `ADR-002-M001-READINESS-CORRECTIONS.md` is authoritative for the pre-M001 contradictions identified by the independent readiness audit.
- `MILESTONE-001.md` narrows the foundation into the implementation slice; its readiness amendment adds mandatory gates without expanding product scope.
- The UI architecture is subordinate to terminal/runtime ownership and performance invariants.
- The source brief records requirements/research questions; it is not an implementation specification.
- Future ADRs may refine a foundation decision only when they cite the affected rationale ID and provide new evidence, measurements, or platform constraints.

## M001 contradiction resolution

For implementation, the following interpretations are fixed:

```text
BlockTimeline authority = Runtime/workspace metadata keyed by ExecutionId
TerminalExecution       = PTY + child + TerminalState + attachment/projection state
production Lua/config   = deferred beyond M001
foundation status       = Accepted
VT conformance corpus   = required M001 gate
local IPC/shm security  = required M001 gate
```

Where an older sentence conflicts with these specific corrections, `ADR-002` and the M001 readiness amendment take precedence. No other foundation decision is reopened.

## Change discipline

Do not create competing `-v2`, `-final`, `-new`, or similarly duplicated architecture copies. Amend the canonical architecture through scoped ADR/rationale updates and preserve decision history.

Repository changes should be made through **branch → pull request → review/validation → merge**.
