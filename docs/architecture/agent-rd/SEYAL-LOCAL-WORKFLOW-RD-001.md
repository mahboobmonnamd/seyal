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

A node may run an agent, evaluator, approval, deterministic transform, operational action or handoff. Workflow state never becomes terminal state.

## Workflow definition

Each versioned Workflow records:

- node IDs/types and typed inputs/outputs;
- dependency edges/conditions;
- execution/isolation requirement;
- timeout, retry and cancellation policy;
- **effect/replay class** for any node that may mutate state;
- budget policy;
- required harness/context capabilities;
- approval/attention gates;
- artifact ownership expectations.

A running workflow pins the definition version that created it.

## Effect and replay model

Git worktree isolation is sufficient only for repository mutations. Seyal workflows may eventually drive infrastructure, deployment, databases and other external systems where an ambiguous retry can repeat a side effect.

Every mutating node declares one effect class:

```text
pure                    # no externally visible mutation; replay safe
filesystem_isolated     # mutation confined to declared disposable/isolated workspace
idempotent_external     # external operation has a verified idempotency/fencing key or equivalent contract
non_idempotent_external # replay may duplicate an externally visible effect
non_replayable          # workflow cannot safely infer/reconstruct whether replay is valid
```

Unknown effect semantics default to `non_replayable`, not `pure`.

A node also records the concrete effect scope/identity where applicable, for example repository/worktree, deployment target, cluster/resource, database migration ID, cloud request/idempotency key, ticket/PR ID or other external operation reference.

### Retry rules

- `pure` nodes may retry subject to normal retry/budget policy.
- `filesystem_isolated` nodes may retry only into a fresh or deterministically reset isolation boundary unless the node proves replay safety.
- `idempotent_external` nodes may retry only when the idempotency/fencing mechanism is part of the recorded node contract and the adapter/tool confirms the same logical operation identity.
- `non_idempotent_external` and `non_replayable` nodes **must not automatically retry after ambiguous start/execution failure**. They enter `reconciliation_required`/`AttentionItem` until Seyal or an authorized evaluator can establish whether the effect occurred.
- A timeout does not imply that an external operation failed. Timeout is an observation about Seyal's wait, not proof of side-effect absence.
- Cancellation is best-effort for in-flight external operations and must never be reported as rollback unless the integration proves rollback occurred.

Examples that require explicit external-effect semantics include `terraform apply`, `kubectl apply`, deployment APIs, database migrations, cloud resource mutation, payment/ticketing APIs and incident-response actions.

## Scheduling

Local scheduler rules:

1. only dependency-ready nodes enter the runnable queue;
2. concurrency is bounded globally and per workflow;
3. explicit budgets cover attempts, direct AI cost/usage and elapsed limits where known;
4. cancellation propagates along dependency/ownership edges, not by killing unrelated executions;
5. a safe node retry creates a new `Attempt`; an unsafe/ambiguous external-effect node enters reconciliation instead of retrying;
6. workflow restart reconstructs metadata state, then reattaches/resumes only where execution/harness/effect capabilities prove it is safe.

A journal cannot resurrect a dead PTY. If a referenced TerminalExecution is gone and a harness cannot resume, the NodeRun becomes interrupted/recovery-required.

Likewise, persisted workflow metadata cannot prove an external mutation did or did not happen unless the external system/idempotency contract supplies that evidence.

## Repository/worktree isolation

Default for concurrent **writers**:

```text
one editing NodeRun
 → one dedicated git worktree
 → one base revision recorded
```

Read-only evaluators/reviewers may share immutable/repository views when policy allows. Two writers do not silently edit the same worktree.

Worktrees share Git object storage naturally; context/cache sharing follows #52/#53 content-hash rules rather than copying entire repositories.

Worktree isolation must not be generalized into a claim that all workflow side effects are isolated. External systems use the effect/replay contract above.

## Artifact ownership

Artifacts are immutable/versioned references owned by the producing NodeRun/Attempt. Downstream nodes consume explicit versions.

A handoff should contain references plus bounded structured claims, for example:

```text
Handoff
  from/to NodeRun
  WorkItem/Attempt refs
  artifact refs
  selected ContextItem refs
  effect refs / reconciliation state
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
- overlapping claimed artifact/output intent;
- overlapping declared external effect scope/identity.

This can warn/block/require confirmation. Semantic duplicate detection can be evaluated later; it is not required to prevent the most dangerous obvious duplication.

## Conflicting edits and effects

Repository conflict detection is based on concrete Git/file state:

1. record each writer's base revision;
2. inspect changed path sets and merge applicability;
3. automatically combine only mechanically safe/non-conflicting results;
4. route semantic or textual conflict to a reconciliation NodeRun or `AttentionItem`;
5. preserve both candidate artifacts until a decision is accepted.

For external effects, conflict/reconciliation uses the integration's authoritative resource/version/request identity. Two operations targeting the same resource are never declared safe merely because their local worktrees differ.

### Rejected: autonomous conflict resolution by default

Having another model silently choose between conflicting patches or ambiguous external effects hides intent and can destroy valid work. Reconciliation is an explicit evaluatable step.

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

The first implementation milestone should keep this baseline repository-scoped; infrastructure/operational mutation support must not be added until effect/replay behavior has its own spec and retained failure fixtures.

## Approval nodes

Typed upstream approvals/questions may be answered through the existing Attention model. Unknown/raw terminal prompts require focusing the exact TerminalExecution; the workflow engine never synthesizes arbitrary keystrokes to “approve”.

Approval timeouts yield explicit workflow state. They do not block PTY rendering or hold unbounded worker resources by default.

Approval does not itself make a non-idempotent operation replay-safe.

## Persistence/recovery

Persist workflow metadata/events/artifact refs asynchronously:

- WorkflowRun/NodeRun state transitions;
- Attempt/AgentRun refs;
- routing/context fingerprints;
- artifact/handoff refs;
- effect class/scope/idempotency/fencing refs;
- evaluation/attention/reconciliation state;
- retry/budget counters.

On restart, reconcile persisted metadata with the authoritative Runtime/execution registry, harness session capabilities and any authoritative external-effect status query. Never reconstruct a live PTY from workflow records.

If Seyal cannot determine whether a non-idempotent external effect happened, state remains ambiguous/reconciliation-required. Do not convert uncertainty into a retry.

## Rejected approaches

- Kubernetes/general distributed scheduler for local workflows;
- shared mutable prompt/context blackboard as authority;
- one hidden PTY per workflow node regardless of need;
- unrestricted parallel writers in one worktree;
- assuming worktree isolation protects infrastructure/external mutations;
- retrying external operations merely because Seyal timed out or lost connectivity;
- planner output treated as trusted policy;
- organization queues/fleet quotas/team tenancy in OSS foundation.

## Evaluation

Retained scenarios:

- planner→implementer→tester→reviewer happy path;
- two parallel read-only analyses;
- two isolated candidate implementations;
- duplicate WorkItem detection;
- conflicting patch reconciliation;
- pure node timeout/retry budget exhaustion;
- filesystem-isolated retry into fresh isolation;
- idempotent external retry with stable operation key;
- ambiguous timeout after non-idempotent external mutation → no automatic retry;
- cancellation during external operation → no false rollback claim;
- user cancellation propagation;
- harness crash with resume support;
- harness crash without resume support;
- Runtime survives GUI reconnect;
- Runtime/execution loss demonstrating non-restorable PTY semantics;
- attention/approval pause and resume.

Measure accepted WorkItems, attempts, elapsed time, direct AI cost, human intervention, conflicts, duplicate work, ambiguous/reconciled effects and scheduler overhead.

## OSS/commercial boundary

**OSS:** workflow DAG/model, local scheduler/queue, effect/replay classes, safe retries/timeouts/cancel, local parallel runs, worktree isolation, typed handoffs, local duplicate/conflict/effect detection, approval nodes and recovery metadata.  
**Commercial:** organization fleet queues, cross-user ownership, shared/team workflow catalogs, managed reliability/SLA, quotas, organization-wide duplicate/conflict/effect reconciliation and collaboration.

The commercial fleet layer may add stronger distributed fencing but must consume the OSS effect/replay semantics rather than invent a second mutation-safety model.

## Success / kill criteria

Pass when the baseline workflow survives process/UI interruption according to actual harness/runtime capabilities, isolates concurrent writers, makes retries/conflicts visible, refuses unsafe automatic replay of ambiguous external effects, and adds no dependency to terminal progress.

Reject if ordinary local orchestration requires a cloud service, distributed control plane, duplicate terminal state, hidden shared-worktree mutation, or unbounded/unsafe side-effect retry.

## ADR/spec before implementation

- Workflow ownership/persistence/isolation ADR after #51/#52 interfaces stabilize.
- Workflow/NodeRun state-machine spec.
- Worktree writer-isolation and conflict/reconciliation spec.
- **Effect/replay/idempotency/reconciliation specification before operational mutation nodes exist.**
- Typed Handoff and Attention integration spec.
