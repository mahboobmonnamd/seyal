# Agent Platform Parallel Tracks Refinement

**Status:** Proposed refinement; does not replace `ROADMAP.md` until #838 review/promotion  
**Issue:** #838  
**Purpose:** Make clear what can proceed in parallel while M002 is active and where the first-party Seyal AI Agent fits.

## 1. Product split

Seyal has two complementary agent products:

### Universal Agent Sessions

Seyal observes and, where supported, controls external CLI agents through a capability-based adapter/presence model. External agents remain ordinary `TerminalExecution` workloads and never become terminal authority.

### Seyal AI Agent

Seyal also owns a first-party agent harness using the same `WorkItem -> Attempt -> AgentRun` authority, Attention/Artifact/Evaluation models, Local Context Engine, MemoryStore and capability/permission plane.

The first-party harness must not be implemented as a special terminal path or as a vendor-specific domain model.

## 2. Parallel tracks

### Track A — Terminal foundation

**Current production track: M002.**

Owns terminal compatibility and performance only:

```text
PTY -> VT -> TerminalState -> history/reflow -> projection -> Metal
```

No agent semantics enter M002 production code except terminal compatibility required to run CLI-agent TUIs correctly as ordinary workloads.

### Track B — Universal Agent Sessions

**May start now during M002 as R&D/spec/conformance work.**

Current production home remains M005:

- #678 durable WorkItem/Attempt/AgentRun event authority;
- #679 adapter/presence/conformance;
- #680 Attention/approval/artifact UX.

Work that may proceed now:

- freeze AgentRun vs user-facing Agent Session semantics;
- freeze capability negotiation and detection tiers;
- build vendor-neutral adapter capability matrices/spec fixtures;
- define replay/fake adapter conformance;
- threat/performance review;
- define external-agent subagent/usage/artifact/approval mappings where upstream evidence exists.

Do not ship M005 production integration during M002.

### Track C — Seyal AI Agent

**May start now during M002 as architecture/R&D/spec/evaluation work.**

Current/future production components:

- #678 identity/event foundation;
- #681 Local Context Engine, privacy, memory, evaluation and routing foundations;
- #680 typed approvals/Attention;
- a new explicit first-party Seyal AI Agent harness package to be materialized after #838 review;
- M006 #682 for workflow/multi-agent orchestration;
- M006 #683 for bounded coding/SCM/DevOps/LSP surfaces and external control projections.

Work that may proceed now:

- MemoryRecord/MemoryStore architecture and evaluation corpus;
- ContextBundle/SelectionTrace behavior and benchmark corpus;
- first-party harness/tool-loop state machine;
- provider adapter contract with initial OpenAI-only implementation scope;
- permission/capability policy model;
- skills/MCP integration boundaries;
- evaluation metrics and regression fixtures;
- multi-agent isolation/handoff architecture;
- security/performance threat models.

Do not ship M005/M006 production code during M002 merely because these R&D streams are parallel.

## 3. Exact production dependency plan

The current milestone discipline remains authoritative:

```text
M002
  terminal compatibility breadth

M003
  core workspace/resource/navigation/presentation primitives

M004
  durable local workspace/storage/security foundation

M005
  #678 WorkItem/Attempt/AgentRun + durable event authority
       |
       +--> #679 external Agent Sessions/adapters
       +--> #680 Attention/approval/artifact UX
       +--> #681 context + memory + evaluation + deterministic routing
       |
       +--> first-party Seyal AI Agent harness package
             depends on the exact subset of #680/#681 it consumes

M006
  #682 workflow DAG + multi-agent orchestration/worktree isolation
  #683 coding/SCM/DevOps/LSP/control surfaces
```

After M004 is accepted, #678 is the first M005 implementation dependency. Once #678 is merged and exact readiness is rechecked, #679, #680 and #681 should proceed in parallel where their remaining dependencies are satisfied.

The first-party Seyal AI Agent package should then proceed as soon as the required #680/#681 seams are available; it should not wait for all of M006. M006 extends the single-agent local harness into workflows, parallel/cooperating agents and richer coding/DevOps surfaces.

## 4. Provider scope

Product scope may choose **OpenAI as the only initially implemented first-party model provider**.

Architecture must still keep the `ModelProvider` seam provider-neutral so this is a launch-scope decision rather than irreversible coupling.

This restriction applies only to the first-party Seyal AI Agent's initial model implementation. Universal Agent Sessions remain able to observe/integrate external agent harnesses regardless of the model/provider they internally use.

## 5. Memory/context milestone mapping

### M005 foundation

Must include:

- local durable `MemoryStore` with explicit lifecycle/scope/provenance/privacy;
- deterministic Local Context Engine;
- memory modes and deletion/invalidation semantics;
- ContextBundle + SelectionTrace;
- optional model-assisted memory proposal/reranking/compaction;
- evaluation of retrieval and memory usefulness;
- terminal-isolation/resource gates.

### M006 extension

Adds:

- memory/context handoff across multi-agent workflow nodes under explicit scope;
- worktree-aware isolation and conflict rules;
- recurring engineering-memory UX over the same M005 MemoryStore;
- LSP/SCM/CI/DevOps sources feeding the same Context Engine;
- workflow-level evaluation and routing evidence.

No second memory store or context engine is introduced in M006.

## 6. Required architecture gate before implementation

#838 must pass independent architecture review before these decisions become ADR/spec authority.

The review must specifically challenge:

- duplicate AgentSession vs AgentRun authority;
- first-party harness special cases;
- memory vs transcript/event/cache conflation;
- stale/conflicting memory and supersession;
- privacy/delete semantics and derived data cleanup;
- OpenAI lock-in hidden in core schemas;
- tool/approval capability escalation;
- multi-agent/worktree leakage;
- terminal hot-path dependency.

Only reviewed and reconciled decisions should be promoted into canonical roadmap/ADR/spec documents.
