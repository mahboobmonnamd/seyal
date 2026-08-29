# Agent Execution Workflow Product Refinement

**Status:** Accepted product refinement for existing Seyal capabilities  
**Scope:** Seyal OSS product behavior and stable seams only  
**Roadmap home:** M005–M008  
**Implementation authority:** Existing owning issues, ADRs and specifications remain authoritative

This document sharpens existing product contracts already represented by `SY-007`, `SY-009`–`SY-011`, `SY-020`–`SY-024`, `F-110`–`F-124`, `F-136`–`F-153`, `F-195`, `F-202`–`F-203` and the M005–M008 roadmap packages.

It is **not** a second feature registry and does not authorize implementation ahead of the owning milestone. The canonical feature inventory remains `docs/product/FEATURES.md`.

## 1. Product outcome

Seyal should let humans and agents coordinate many independent or cooperating executions without turning the terminal into an agent-specific UI or making agent state authoritative over terminal state.

The desired workflow is:

```text
intent / task / operational goal
        ↓
WorkItem
        ↓
coordination topology
├─ isolated Attempt/worktree/executions
└─ shared workspace/branch with cooperating AgentRuns
        ↓
terminal + agent + SCM/CI + workflow + operational signals
        ↓
Readiness + Attention
        ↓
review / approval / merge / continue / reconcile
```

All derived workflow state remains off the terminal hot path.

## 2. Unified Readiness projection

Seyal needs one typed, provider-neutral readiness model that answers whether a unit of work is ready for its next meaningful action.

Examples for development work may include:

- working-tree state;
- branch/base divergence;
- pull/merge request state;
- CI/status checks;
- deployments;
- unresolved review comments;
- workflow validations;
- explicit todos;
- required approvals.

Examples for operational work may include:

- process/service health;
- rollout state;
- failed workloads;
- error-rate or log-derived signals from trusted adapters;
- workflow validation nodes;
- required human approvals.

### Contract

- Readiness is a **derived projection**, never a new execution authority.
- Each signal records source, timestamp/freshness and `pass/fail/pending/unknown/not-applicable` semantics.
- Unknown or stale evidence must remain visibly unknown/stale rather than being treated as success.
- Provider adapters map external systems into common typed capabilities.
- Readiness may recommend or gate a Seyal action only when the underlying capability/policy explicitly authorizes that gate.
- Raw terminal text may contribute low-confidence advisory evidence but can never become approval, security, audit or merge authority.
- Collection, polling and refresh must be asynchronous and visibility-aware; it must never delay PTY input/output/rendering.

This refines `SY-007`, `SY-022`, `SY-024`, `F-146`, `F-202`, `F-243` and M006 issue #683.

## 3. Explicit multi-agent coordination topology

Parallel agents need an explicit choice between **isolated work** and **cooperating work**.

### Isolated work

Use a distinct `Attempt` and dedicated worktree/execution context when work should:

- ship independently;
- produce a separate review/merge path;
- be compared as an alternative approach;
- be safely discarded without affecting another attempt;
- run independent build/test/dev processes.

### Cooperating work

Multiple `AgentRun`s may intentionally share one workspace/branch/code state when they are collaborating on the same outcome, for example:

- implementer + reviewer;
- implementation + test-repair agent;
- coordinated frontend/backend work that must land together;
- independent review agents inspecting the same current diff.

### Contract

- The coordination topology is explicit and visible; Seyal must not silently change isolation mode.
- Concurrent independent writers default to isolated worktrees.
- Shared-workspace writers require explicit workflow ownership/coordination semantics; conflict detection remains mandatory.
- A Git worktree is development isolation, **not a security boundary**.
- Worktree identity never replaces Seyal Workspace, WorkItem, Attempt, AgentRun or TerminalExecution identity.
- Moving between worktree contexts follows `SY-005`; a busy shell is never silently `cd`'d.

This refines `SY-004`, `SY-005`, `SY-021`, `F-148`, `F-153`, `F-195` and M006 issue #682.

## 4. Scheduled and triggered workflow runs

Reusable workflows should support execution without requiring a person to manually recreate the same setup each time.

Supported trigger classes should be introduced incrementally:

1. explicit manual run;
2. local time/schedule trigger;
3. local typed event trigger;
4. later hosted/team event and webhook triggers through higher-edition services using the same OSS workflow seam.

### Contract

- Every trigger creates or resumes a durable `WorkflowRun`/`WorkItem`; it does not create an invisible background side channel.
- The resulting run is inspectable, cancellable where safe, and openable as normal Seyal work.
- Trigger metadata records identity, source and schedule/event provenance.
- Repeated triggers obey concurrency, deduplication and effect/replay rules.
- Failed/blocked runs surface through Attention rather than requiring the user to watch them continuously.
- A schedule never grants additional capabilities; execution keeps the same permission/effect/policy model as a manually started run.
- Local scheduled workflows remain useful without a Seyal account. Managed organization scheduling/fleet execution may build above the OSS seam.

This refines `SY-020`, `SY-021`, `F-140`–`F-142`, `F-203` and M006 issue #682.

## 5. One control authority, multiple protocol projections

Seyal should expose its typed local control capabilities to humans, scripts and agents without creating multiple control models.

The authoritative model remains the capability-scoped Control API (`SY-011`). It may be projected through:

- Seyal CLI / Shell API;
- a local SDK where justified;
- an MCP-compatible adapter;
- later secure remote/team service APIs.

### Initial operation families

- resolve/list/open resources;
- create/open panes or executions through normal Runtime ownership;
- inspect bounded execution/readiness/attention state;
- create/start/prompt/cancel supported AgentRuns;
- create/run supported workflows;
- publish typed progress/artifacts/attention;
- perform explicitly authorized typed input/control actions.

### Contract

- Protocol projections share the same resource IDs, authz, capability checks and stale-resource rules.
- No projection owns PTYs, VT state, terminal grids, AgentRun state or WorkflowRun state.
- Arbitrary unrestricted key injection is not the default automation interface.
- Terminal reads must be bounded and explicit about snapshot/freshness semantics.
- Automation/control traffic must never synchronously gate terminal I/O/rendering.
- Local control must work without cloud connectivity.

This refines `SY-002`, `SY-010`, `SY-011`, `F-118`, `F-143`, `F-150`, `F-222` and M006 issue #683.

## 6. Agent lifecycle, status lanes and Attention

Seyal should summarize many running agents/workspaces without forcing users to watch terminal output.

### Contract

- Agent lifecycle uses structured adapters/hooks where available and preserves source/confidence.
- Stable navigation order remains the default (`SY-009`).
- Derived status lanes such as `working`, `needs attention`, `needs review`, `ready`, `failed` or `completed` may be shown as filters/boards, but must not silently reorder or redefine Workspace identity.
- Attention remains the canonical human-intervention model for approvals, questions, failures, conflicts and important completion events.
- Readiness and Attention are related but distinct: readiness describes evidence toward the next action; Attention says a human should act.

This refines `SY-006`, `SY-009`, `F-110`–`F-124`, `F-136`, `F-138`, `F-162` and `F-202`.

## 7. Checkpoint semantics remain honest

Agent/workflow checkpoints are useful, but Seyal must keep different kinds of state separate.

```text
code/files checkpoint
workflow/agent conversational checkpoint
terminal scrollback/history
live PTY + child process state
```

Reverting code or conversation state must never claim to rewind a live process, external side effect, remote system or PTY. Any reversible workflow effect must be proven by its owning effect contract.

This refines `F-147` and `SY-020`.

## 8. Milestone placement

| Milestone | Refinement outcome |
|---|---|
| M005 | Structured agent lifecycle, Attention, evaluation/provenance and capability-scoped foundations required by these projections. |
| M006 | Implement local readiness projection, explicit coordination topology, local scheduled/triggered workflows and CLI/control/MCP projections. |
| M007 | Reuse the same identities/control/readiness semantics across remote execution without adding another state model. |
| M008 | Extend generic seams to authorized shared work, presence and team handoff; proprietary managed collaboration remains outside OSS. |

These refinements **do not move anything into the M004 market-ready critical path**.

## 9. Acceptance gates

Implementation is incomplete unless it proves:

- one authoritative Runtime/TerminalExecution state model;
- no new PTY or terminal grid for workflow/agent/readiness surfaces;
- deterministic lifecycle and stale-resource behavior;
- explicit shared-vs-isolated coordination tests;
- worktree conflict/reconciliation tests;
- schedule dedupe/concurrency/recovery/effect-safety tests;
- adapter fixtures for pass/fail/pending/unknown/stale readiness signals;
- capability/authz tests across CLI/control/MCP projections;
- checkpoint tests proving no false process/external-effect rollback claim;
- terminal latency/throughput/idle CPU/RSS isolation benchmarks with these services active and inactive.

## 10. Product boundary

These capabilities strengthen Seyal's execution workspace without redefining it as a code-only agent orchestrator.

The same primitives must remain useful for:

- ordinary interactive shells;
- long-running processes;
- SSH and nested tools;
- coding-agent workflows;
- build/test/review flows;
- DevOps/SRE operations;
- future remote and shared execution.

Terminal correctness, local execution and core runtime behavior remain excellent independently of every feature in this refinement.