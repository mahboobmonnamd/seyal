# Seyal Local Workflows + Multi-Agent Coordination R&D

**Status:** Proposed  
**Issue:** #56  
**Dependencies:** #48; execution contracts depend on #51/#52  
**Scope:** OSS local orchestration; no production implementation.

## Decision

Use a durable local DAG with explicit `WorkflowRun`/`NodeRun` state, typed handoffs and bounded scheduling. Do not build a distributed fleet control plane into OSS.

```text
Workflow(versioned DAG)
  ↓
WorkflowRun
  ├─ NodeRun: planner
  ├─ NodeRun: implementer A ─┐
  ├─ NodeRun: implementer B ─┼→ evaluation/reconciliation
  ├─ NodeRun: tester ────────┤
  ├─ NodeRun: reviewer ──────┘
  └─ NodeRun: approval/attention
```

A node may run an agent, evaluator, approval, deterministic transform or handoff. Workflow state never becomes terminal state.

## Workflow definition

Each versioned Workflow records:

- node IDs/types and typed inputs/outputs;
- dependency edges/conditions;
- execution/isolation requirement;
- timeout, retry and cancellation policy;
- budget policy;
- required harness/context capabilities;
- approval/attention gates;
- artifact ownership expectations.

A running workflow pins the definition version that created it.

## Scheduling

Local scheduler rules:

1. only dependency-ready nodes enter the runnable queue;
2. concurrency is bounded globally and per workflow;
3. explicit budgets cover attempts, direct AI cost/usage and elapsed limits where known;
4. cancellation propagates along dependency/ownership edges, not by killing unrelated executions;
5. a node retry creates a new `Attempt`;
6. workflow restart reconstructs metadata state, then reattaches/resumes only where execution/harness capabilities prove it is safe.

A journal cannot resurrect a dead PTY. If a referenced TerminalExecution is gone and a harness cannot resume, the NodeRun becomes interrupted/recovery-required.

## Repository/worktree isolation

Default for concurrent **writers**:

```text
one editing NodeRun
 → one dedicated git worktree
 → one base revision recorded
```

Read-only evaluators/reviewers may share immutable/repository views when policy allows. Two writers do not silently edit the same worktree.

Worktrees share Git object storage naturally; context/cache sharing follows #52/#53 content-hash rules rather than copying entire repositories.

## Artifact ownership

Artifacts are immutable/versioned references owned by the producing NodeRun/Attempt. Downstream nodes consume explicit versions.

A handoff should contain references plus bounded structured claims, for example:

```text
Handoff
  from/to NodeRun
  WorkItem/Attempt refs
  artifact refs
  selected ContextItem refs
  findings/assumptions
  unresolved questions
  required next action
```

Do not pass full hidden transcripts or mutable global scratch state by default.

## Duplicate-work detection

Start deterministic:

- same repository/base/worktree scope;
- same/ancestor WorkItem relationship;
- overlapping declared target paths/symbols;
- matching normalized task fingerprint;
- overlapping claimed artifact/output intent.

This can warn/block/require confirmation. Semantic duplicate detection can be evaluated later; it is not required to prevent the most dangerous obvious duplication.

## Conflicting edits

Conflict detection is based on concrete Git/file state:

1. record each writer's base revision;
2. inspect changed path sets and merge applicability;
3. automatically combine only mechanically safe/non-conflicting results;
4. route semantic or textual conflict to a reconciliation NodeRun or `AttentionItem`;
5. preserve both candidate artifacts until a decision is accepted.

### Rejected: autonomous conflict resolution by default

Having another model silently choose between conflicting patches hides intent and can destroy valid work. Reconciliation is an explicit evaluatable step.

## Planner → implementer → tester → reviewer baseline

This is the first demonstration workflow, not a universal architecture:

```text
plan
 ↓ typed implementation handoff
implement in isolated worktree
 ↓ patch/artifacts
run project-defined evaluators/tests
 ↓ evidence
independent review
 ↓
accepted | revision Attempt | AttentionItem
```

Planner output is advisory input. It does not gain execution authority merely by being produced by an agent.

## Approval nodes

Typed upstream approvals/questions may be answered through the existing Attention model. Unknown/raw terminal prompts require focusing the exact TerminalExecution; the workflow engine never synthesizes arbitrary keystrokes to “approve”.

Approval timeouts yield explicit workflow state. They do not block PTY rendering or hold unbounded worker resources by default.

## Persistence/recovery

Persist workflow metadata/events/artifact refs asynchronously:

- WorkflowRun/NodeRun state transitions;
- Attempt/AgentRun refs;
- routing/context fingerprints;
- artifact/handoff refs;
- evaluation/attention state;
- retry/budget counters.

On restart, reconcile persisted metadata with the authoritative Runtime/execution registry and harness session capabilities. Never reconstruct a live PTY from workflow records.

## Rejected approaches

- Kubernetes/general distributed scheduler for local workflows;
- shared mutable prompt/context blackboard as authority;
- one hidden PTY per workflow node regardless of need;
- unrestricted parallel writers in one worktree;
- planner output treated as trusted policy;
- organization queues/fleet quotas/team tenancy in OSS foundation.

## Evaluation

Retained scenarios:

- planner→implementer→tester→reviewer happy path;
- two parallel read-only analyses;
- two isolated candidate implementations;
- duplicate WorkItem detection;
- conflicting patch reconciliation;
- node timeout/retry budget exhaustion;
- user cancellation propagation;
- harness crash with resume support;
- harness crash without resume support;
- Runtime survives GUI reconnect;
- Runtime/execution loss demonstrating non-restorable PTY semantics;
- attention/approval pause and resume.

Measure accepted WorkItems, attempts, elapsed time, direct AI cost, human intervention, conflicts, duplicate work and scheduler overhead.

## OSS/commercial boundary

**OSS:** workflow DAG/model, local scheduler/queue, retries/timeouts/cancel, local parallel runs, worktree isolation, typed handoffs, local duplicate/conflict detection, approval nodes and recovery metadata.  
**Commercial:** organization fleet queues, cross-user ownership, shared/team workflow catalogs, managed reliability/SLA, quotas, organization-wide duplicate detection/reconciliation and collaboration.

## Success / kill criteria

Pass when the baseline workflow survives process/UI interruption according to actual harness/runtime capabilities, isolates concurrent writers, makes retries/conflicts visible, and adds no dependency to terminal progress.

Reject if ordinary local orchestration requires a cloud service, distributed control plane, duplicate terminal state, or hidden shared-worktree mutation.

## ADR/spec before implementation

- Workflow ownership/persistence/isolation ADR after #51/#52 interfaces stabilize.
- Workflow/NodeRun state-machine spec.
- Worktree writer-isolation and conflict/reconciliation spec.
- Typed Handoff and Attention integration spec.
