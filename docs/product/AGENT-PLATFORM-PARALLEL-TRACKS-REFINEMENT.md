# Agent Platform Parallel Tracks Refinement

**Status:** Proposed refinement; does not replace `ROADMAP.md` until #838 review/promotion  
**Issue:** #838  
**Purpose:** Define what may proceed in parallel while M002 is active and freeze the production dependency boundaries for Universal Agent Sessions, memory/context and the first-party Seyal AI Agent.

## 1. Product split

### Universal Agent Sessions

Seyal observes and, where the upstream integration genuinely supports it, controls external CLI agents through a capability-based adapter/presence model. External agents remain ordinary workloads and never become PTY/VT/TerminalState authority.

Agent Session is a user-facing projection over durable `AgentRun`; it is not a second durable state machine.

### Seyal AI Agent

Seyal also owns a first-party harness using the same `WorkItem -> Attempt -> AgentRun` authority, Attention/Artifact/Evaluation models, Local Context Engine, MemoryStore and durable capability/action/effect plane.

The first-party harness must not become a special terminal path or a vendor-specific durable domain model.

## 2. Parallel tracks during M002

### Track A — Terminal foundation

**Production now: M002.**

```text
PTY -> VT -> TerminalState -> history/reflow -> projection -> Metal
```

No agent/memory/context/model semantics enter the terminal hot path. M002 only needs the terminal compatibility required to run CLI-agent TUIs correctly as ordinary workloads.

### Track B — Universal Agent Sessions

**May proceed now as R&D/spec/conformance work. Production home: M005.**

Current owners:

- #678 durable WorkItem/Attempt/AgentRun event authority;
- #679 external adapter/presence/conformance;
- #680 Attention/approval/artifact semantics.

Allowed during M002:

- AgentRun vs Agent Session semantics;
- binding generation/fencing and reconnect transition fixtures;
- detection tiers and capability negotiation;
- explicit `Observed` vs `UpstreamRequestable` vs `SeyalEnforced` capability semantics;
- replay/fake-adapter conformance;
- external-agent subagent/usage/artifact/approval mapping where upstream evidence exists;
- security/performance/failure fixtures.

Do not ship M005 agent integration during M002.

### Track C — Seyal AI Agent

**May proceed now as architecture/R&D/spec/evaluation work. Production home: M005/M006.**

Current/future owners:

- #678 durable identity/event foundation;
- #681 Context Engine, MemoryStore, privacy, evaluation and deterministic routing;
- #680 Attention/approval semantics;
- #841 reusable durable capability/action/effect dispatch safety;
- #839 first-party Seyal AI Agent harness;
- M006 #682 workflow/multi-agent orchestration;
- M006 #683 coding/SCM/DevOps/LSP and CLI/SDK/MCP/control projections.

Allowed during M002:

- MemoryRecord/MemoryStore lifecycle, revocation-race and evaluation fixtures;
- ContextBundle/SelectionTrace/RunWorkingSet/retention behavior;
- harness/model-provider contract and synthetic alternate-provider conformance;
- single-agent action/effect/recovery state-machine design;
- permission/approval/capability threat models;
- skills/MCP boundaries;
- evaluation/regression corpus;
- worktree/context/memory/evaluator isolation rules;
- resource-pressure/failure benchmarks design.

Do not ship M005/M006 production code during M002 merely because R&D is parallel.

## 3. Production dependency plan

```text
M002
  terminal compatibility breadth

M003
  core workspace/resource/navigation/presentation primitives

M004
  durable local workspace/storage/security foundation

M005
  #678 WorkItem/Attempt/AgentRun durable event + transition authority
       |
       +--> #679 external Agent Sessions/adapters
       +--> #680 Attention/approval/artifact semantics
       +--> #681 Context Engine + MemoryStore + evaluation/routing
       |
       +--> #841 durable capability/action/effect safety
       |      consumes the exact #680 approval seam it needs
       |
       +--> #839 first-party Seyal AI Agent harness
              consumes #681 + #841 + exact #680 seams

M006
  #682 workflow DAG + multi-agent orchestration
  #683 coding/SCM/DevOps/LSP + CLI/SDK/MCP/control projections
       consumes #841 rather than creating another control/effect authority
```

After M004, #678 remains the first M005 implementation dependency. After #678, #679/#680/#681 may proceed in parallel when their exact remaining dependencies are satisfied.

#841 is the reusable effect/replay safety primitive discovered by #838 review. It must exist before #839 because typed approval alone cannot make ambiguous external effects replay-safe.

#839 starts once its exact #681/#841/#680 seams are accepted and implemented. It does **not** need to wait for all of M006.

The live issue plan has been reconciled accordingly:

- #667 records the M005 substrate and explicit provider disposition;
- #678 owns AgentRun transition/binding writer authority;
- #680 owns human Attention/Approval semantics and exact action binding;
- #681 owns MemoryStore, RunWorkingSet, retention/resumability and use-time revocation;
- #841 owns durable capability/action/effect dispatch safety;
- #839 consumes #681/#841/#680 for the first-party harness;
- #683 consumes #841 for later CLI/SDK/MCP/control projections.

## 4. Memory/context milestone mapping

### M005 foundation — #681

Must include:

- durable `MemoryStore` lifecycle/scope/provenance/privacy;
- `Accepted` meaning policy-eligible, not automatically true;
- deterministic Local Context Engine;
- RunWorkingSet and explicit retention/resumability availability;
- Memory Disabled/ReadOnly/Curated/Assisted modes;
- use-time revocation/forgetting checks and invalidation of queued bundles, compactions, provider continuations and derived caches;
- re-extraction suppression from the same pre-revocation evidence;
- immutable ContextBundle + SelectionTrace;
- baseline workspace/worktree/user-scope isolation;
- optional model-assisted proposal/rerank/compaction only above deterministic baseline;
- retrieval/memory evaluation and terminal-resource isolation gates.

### M006 extension

Adds:

- memory/context handoff across workflow nodes under explicit scope;
- orchestration-aware isolation/conflict rules;
- recurring engineering-memory UX over the same M005 store;
- LSP/SCM/CI/DevOps sources feeding the same Context Engine;
- workflow-level evaluation/routing evidence.

M006 does not introduce another MemoryStore/Context Engine or postpone the first safe worktree boundary.

## 5. Durable action/effect ownership

#841 owns the minimum reusable M005 action/effect contract:

```text
ActionIntent
  -> exact capability/resource/effect/policy binding
  -> durable authorization / single dispatch generation
  -> dispatch
  -> known success / known failure / effect-unknown reconciliation
```

Rules:

- approval is exact, expiring and replay-safe;
- changed arguments/resource/policy require reauthorization;
- ambiguous non-replayable effects are never retried blindly;
- cancellation after dispatch never claims rollback;
- two workers cannot consume the same approval/action concurrently;
- storage failure may pause the affected agent but must not block unrelated terminal progress.

#839 consumes this contract for first-party tools. #683 later projects it through CLI/SDK/MCP/control surfaces. Neither may create a competing permission/effect authority.

## 6. AgentRun binding/recovery ownership

The Runtime/domain AgentRun authority is the sole durable lifecycle writer. Adapters/harness workers submit typed observations/intents through binding generations/fencing.

Before production, specs must cover:

- idempotent external detection;
- adapter replacement + stale reconnect;
- GUI detach/reconnect;
- worker/provider loss;
- same Run vs new Run vs new Attempt semantics;
- provider continuation loss;
- effect-unknown reconciliation;
- fork lineage with fresh authorization.

Execution liveness, observation availability, behavioral resumability and outcome stay orthogonal.

## 7. Provider scope decision

The product disposition is:

```text
Universal Agent Sessions
  provider-agnostic

Context / Memory / Evaluation
  local and provider-neutral; no first-party model required

First-party Seyal AI Agent model implementation
  OpenAI only for the initial shipped provider

Additional direct/BYOK/local first-party model providers
  later roadmap capability, not an M005 launch requirement
```

The durable `ModelProvider` seam remains provider-neutral. Before implementation it must pass a synthetic materially different provider conformance fixture so the neutrality is demonstrated rather than asserted.

Existing broad no-account/local wording in #667/#681 is refined to mean the local agent substrate continues functioning without a first-party model provider; it does not require a local first-party model adapter at initial #839 launch.

Universal Agent Sessions remain able to observe external agents regardless of which model/provider those tools use internally.

## 8. Isolation is broader than worktrees

Independent agent writers default to isolated worktrees, but production isolation also scopes:

- RunWorkingSet;
- memory reads/proposals;
- temporary directories;
- evaluator inputs;
- mutable test/service state;
- action/resource authority.

A worktree is not an OS sandbox and does not itself prove independent evaluation.

## 9. LSP/code intelligence boundary

LSP is optional/lazy context infrastructure. The cold developer/agent subsystem owns its process lifecycle/configuration. Results carry workspace/worktree/path/document/index generations; stale results cannot become current source truth.

#683 owns the richer user-facing LSP/developer surfaces but consumes the M005 context/source contract.

## 10. Performance/failure gate

Do not validate only zero-agent or enabled-idle overhead. Production gates must include representative active/failure load:

- indexing/context builds;
- event/tool traffic;
- provider streaming;
- adapter crash loops;
- queue saturation;
- persistent/disk-full storage failure;
- LSP load;
- memory revocation/invalidation.

All remain cold/background relative to terminal I/O and must prove no material PTY/VT/render regression under accepted budgets.

## 11. Review reconciliation state

The independent #840 review identified four blocking contracts. The proposal now explicitly addresses them:

1. **B1 memory forgetting/revocation:** use-time revalidation before dispatch; revocation invalidates queued bundles, RunWorkingSet compactions, provider continuation eligibility, indexes and caches; stale privacy/security dependencies cannot be intentionally retained.
2. **B2 durable tool/effect recovery:** #841 owns one reusable action/effect lifecycle with single dispatch fencing, exact approval binding, `effect_unknown` reconciliation and no blind retry of ambiguous non-replayable effects.
3. **B3 AgentRun mutation/recovery ownership:** #678 owns the single durable transition writer, binding generations/fencing and same-Run/new-Run/new-Attempt recovery matrix.
4. **B4 retention-dependent continuation:** identity survival, historical explainability and behavioral resumability are distinct; missing retained prerequisites make behavioral resume explicitly unavailable.

The review's important non-blocking findings are also reflected: accepted memory is not automatically truth, observed/upstream-requestable/Seyal-enforced capabilities are distinct, provider neutrality requires a synthetic alternate-provider fixture, isolation is broader than worktrees, and performance includes active/failure load.

## 12. Architecture promotion gate

#838 must pass **re-review of the reconciled exact PR head** before these decisions become accepted authority.

Before production implementation:

1. re-run independent `pr-review` on the reconciled #840 head;
2. resolve any remaining blocking findings;
3. promote ownership/lifecycle/memory/context/action/provider decisions through dedicated ADR/spec PRs where required;
4. refine #667/#678–#683/#839/#841 against those accepted authorities;
5. run development-readiness independently per implementation issue;
6. mark only dependency-complete production slices Ready.

M002 production scope remains unchanged by this refinement.
