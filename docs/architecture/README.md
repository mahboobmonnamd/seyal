# Seyal Architecture

This directory is the canonical entry point for Seyal foundation architecture.

## Read in this order

1. [`SEYAL-ARCH-FOUNDATION-RD-001.md`](SEYAL-ARCH-FOUNDATION-RD-001.md) — accepted canonical foundation architecture.
2. [`rationale/SEYAL-ARCH-FOUNDATION-RATIONALE-001.md`](rationale/SEYAL-ARCH-FOUNDATION-RATIONALE-001.md) — reasons, rejected alternatives, failure modes, and revisit conditions for foundation decisions and prohibitions.
3. [`ADR-001-LOCAL-DISPLAY-PROJECTION.md`](ADR-001-LOCAL-DISPLAY-PROJECTION.md) — accepted local macOS display-projection decision for M001.
4. [`ADR-003-OSS-COMMERCIAL-REPOSITORY-BOUNDARY.md`](ADR-003-OSS-COMMERCIAL-REPOSITORY-BOUNDARY.md) — accepted public-OSS/private-commercial repository and dependency boundary.
5. [`ADR-004-VT-STATE-OWNERSHIP.md`](ADR-004-VT-STATE-OWNERSHIP.md) — accepted M001 incremental VT parser, authoritative terminal state, logical-line identity and generation-damage ownership decision.
6. [`ADR-005-PTY-EXECUTION-LIFECYCLE.md`](ADR-005-PTY-EXECUTION-LIFECYCLE.md) — accepted M001 PTY endpoint, child lifecycle, detach/terminate, readiness and execution-ownership decision.
7. [`ADR-006-RUNTIME-REACTOR.md`](ADR-006-RUNTIME-REACTOR.md) — accepted M001 macOS multi-execution reactor, bounded fairness, child-exit and nonblocking Runtime-termination decision.
8. [`ADR-007-WORKSPACE-PERSISTENCE-AGENT-CONTINUITY.md`](ADR-007-WORKSPACE-PERSISTENCE-AGENT-CONTINUITY.md) — accepted pre-Pass-4 decision separating Workspace/domain ownership, persistence classes, resource tiers and agent work from presentation/provider-chat lifetime.
9. [`SEYAL-RUNTIME-WORKSPACE-CONTINUITY-RD-001.md`](SEYAL-RUNTIME-WORKSPACE-CONTINUITY-RD-001.md) — focused evidence/alternatives/memory-accounting research behind ADR-007.
10. [`../milestones/MILESTONE-001.md`](../milestones/MILESTONE-001.md) — authoritative M001 implementation scope, passes, tests, security gates, benchmarks, acceptance criteria, and demo procedure.
11. [`ui/SEYAL-UI-ARCHITECTURE-001.md`](ui/SEYAL-UI-ARCHITECTURE-001.md) — presentation architecture for Flow/Raw/TUI, history, Blocks, workspace chrome, inspectors, attention/approvals, desktop/mobile continuity, and render priority.
12. [`SEYAL-AGENT-PLATFORM-RD-PLAN-001.md`](SEYAL-AGENT-PLATFORM-RD-PLAN-001.md) — agent-native OSS foundation research plan; consumes stable Runtime/Workspace identities and remains outside terminal hot-path ownership.
13. [`source/FOUNDATION-RD-BRIEF.md`](source/FOUNDATION-RD-BRIEF.md) — source requirements that initiated the architecture pass; not an implementation specification.

## Authority

- The foundation architecture is **Accepted** and owns foundation architecture decisions.
- The rationale explains **why** those decisions exist; it does not create competing architecture.
- ADRs exist only for distinct architectural decisions that deserve an independent lifecycle. They are not used as amendment or correction files for canonical documents.
- `MILESTONE-001.md` owns the complete M001 implementation contract. M001 corrections and readiness gates are edited directly into that file.
- The UI architecture is subordinate to terminal/runtime ownership and performance invariants.
- ADR-003 owns the repository/dependency boundary between public Seyal OSS and the private `seyal-commercial` superproject; headless, lightweight and full OSS variants remain compositions of the same public terminal/runtime authority.
- ADR-004 owns the permanent M001 VT parser/terminal-state separation and one-authoritative-state rule; sequence semantics remain governed by the VT specification and milestone matrix.
- ADR-005 owns the PTY/child execution boundary: `seyal-exec` owns endpoint/process lifecycle, detach is not terminate, and terminal bytes feed the single `seyal-terminal` authority without a second grid/state model.
- ADR-006 owns the M001 macOS many-execution readiness composition: one bounded Runtime reactor over execution-owned PTYs, no thread-per-PTY, explicit primary-child exit observation, bounded input/fair output progress, and nonblocking Runtime termination scheduling.
- ADR-007 owns the Workspace/domain versus presentation boundary, execution→Workspace ownership association, persistence-class separation, memory/resource-tier contract and the rule that future agent work identity is independent of chat/provider-session identity. It does not authorize production persistence or agent implementation.
- The Agent Platform R&D is subordinate to the terminal/runtime/workspace ownership architecture and must consume stable identities/capabilities without reverse ownership.
- Source briefs preserve research inputs and historical requirements only.
- Git history and pull requests preserve superseded wording; duplicate `-v2`, `-final`, `-new`, `-amendment`, or correction documents are not required.

## Canonical ownership for M001

```text
TerminalExecution
  = ExecutionId + PTY/endpoint + child lifecycle + authoritative TerminalState + attachment/projection state

Workspace association
  = Runtime/workspace metadata: one owning WorkspaceId per ExecutionId

BlockTimeline
  = seyal-workspace / Runtime workspace metadata keyed by ExecutionId + logical history anchors
```

Workspace association never makes Workspace the PTY/VT owner. Blocks never own PTY/VT/grid/process/output copies, and PTY → VT → TerminalState → damage progress never synchronously waits for Workspace/Block/agent/context persistence.

## Change discipline

Update the document that owns the subject. Use a new ADR only for a genuinely separate architecture decision with its own alternatives, rationale, and reopen conditions.

Repository changes use **branch → pull request → review/validation → merge**.
