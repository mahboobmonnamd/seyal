# Seyal Agent Memory, Context and Harness Architecture R&D

**Status:** Proposed refinement; not accepted ADR/spec authority  
**Issue:** #838  
**Related:** #48, #52, #54, #57, #255, #645, #667, #678–#683, #839, #841  
**Scope:** OSS architecture and roadmap refinement only. No production implementation is authorized by this document.

## 1. Purpose

Seyal needs two complementary agent capabilities:

1. **Universal Agent Sessions** for external CLI agents; and
2. a **first-party Seyal AI Agent** with a strong local harness, context, memory, tools, permissions, evaluation and later multi-agent orchestration.

Both must compose over the same durable Seyal identities without creating a second terminal/runtime authority.

The terminal path remains independent:

```text
PTY -> VT/parser -> TerminalState -> damage/projection -> Metal
```

Agent, memory, context, model, indexing, evaluation, workflow, persistence, cloud, telemetry and licensing work must never synchronously gate terminal progress.

## 2. Durable ownership and mutation authority

### 2.1 Durable identity

```text
Workspace
  -> WorkItem
      -> Attempt
          -> AgentRun
              -> ExecutionRef(s)
              -> ArtifactRef(s)
              -> AttentionRef(s)
              -> EvaluationRef(s)
              -> ActionRef(s)
```

- `Workspace` owns product/workspace identity.
- `WorkItem` owns the durable user goal and final accepted outcome.
- `Attempt` owns one bounded try at satisfying the WorkItem.
- `AgentRun` owns one agent/harness execution history within an Attempt.
- `TerminalExecution` continues to own its PTY/process/terminal state where terminal execution is involved.
- provider/harness conversation/session IDs are external references only.

### 2.2 Agent Session is a projection

The user-facing **Agent Session** is a projection over an `AgentRun` plus its active binding/capability snapshot, execution references and attention/artifact/evaluation state.

```text
Agent Session UI
      |
      v
AgentRun authority
      |
      +-- external harness binding? -> AgentAdapter
      +-- first-party harness?      -> SeyalAgentHarness
      +-- execution refs
      +-- artifacts / attention / evaluations / actions
```

Do **not** introduce a durable `AgentSession` state machine competing with `AgentRun`.

### 2.3 Single AgentRun mutation authority

The Seyal Runtime/domain layer is the sole authority allowed to commit durable `AgentRun` lifecycle/control transitions.

Adapters, provider clients, harness workers and UI components may submit typed observations or control intents; they do not directly mutate durable `AgentRun` state.

For every live adapter/worker binding, Seyal records a monotonically increasing **binding generation** and fencing token. Only the current generation may issue Seyal control requests or commit authoritative lifecycle transitions. Late evidence from an older generation may be retained as explicitly late/observational evidence when safe, but stale generations may not consume approvals, dispatch actions, cancel/resume runs or overwrite current lifecycle state.

An upstream PID, executable path or provider session ID is never sufficient durable identity by itself.

### 2.4 Agent execution ownership and GUI detach

The GUI does not own continued agent work.

- an external CLI agent inside a `TerminalExecution` survives GUI detach according to the terminal/runtime persistence contract;
- a first-party API-driven Seyal agent uses a supervised non-terminal execution/worker owned by the durable runtime/execution layer, not by AppKit/Swift presentation state;
- tool commands that need a terminal create/reference normal Runtime-owned `TerminalExecution` objects rather than giving the harness another PTY implementation;
- UI/adapter detach may reduce observation capability but must not fabricate run termination while the underlying execution remains live;
- reconnect rebinds presentation/adapters to existing durable identities through a new binding generation.

The exact worker/process topology remains a later ADR decision. GUI lifetime may not become AgentRun lifetime authority.

### 2.5 Recovery/identity transition matrix

The implementation ADR/spec must preserve these semantics:

| Event | Durable outcome |
|---|---|
| first trusted detection of a previously unbound external agent execution | create/bind one `AgentRun`; repeated detection is idempotent against durable binding evidence |
| GUI detach/reconnect | same `Attempt` and same `AgentRun`; presentation attachment changes only |
| adapter channel loss while external process remains live | same `Attempt` and same `AgentRun`; observation/control capability degrades; next valid adapter uses a new binding generation |
| stale old adapter reconnects after replacement | evidence may be marked late if safe; stale control is fenced/rejected |
| first-party worker crash with resumable retained state and no ambiguous effect | same `Attempt` and same `AgentRun`; new worker generation may resume |
| first-party worker/provider loss with ambiguous external effect | same `Attempt` and same `AgentRun` enters reconciliation-required; no blind retry |
| provider continuation lost but local continuation prerequisites remain | same `Attempt` and same `AgentRun`; rebuild local context and continue |
| provider continuation lost and required retained payload is unavailable | current `AgentRun` cannot claim behavioral resume; explicit reconciliation may restore missing prerequisites and continue the same bounded try, otherwise a retry from scratch creates a new `Attempt` + new `AgentRun` |
| retry from scratch, regardless of whether the strategy is unchanged | preserve/disposition the prior `Attempt`; create a new `Attempt` + new `AgentRun`, linked to the prior evidence |
| materially different strategy/branch intended as a separate acceptance candidate | new `Attempt` + new `AgentRun` |
| fork within an explicitly defined cooperative/candidate attempt | new `AgentRun` with explicit lineage; pending actions/approvals are not inherited automatically |
| Runtime restart | recover durable identity/evidence, then independently reconcile execution liveness, binding generation, pending actions and resumability; persisted metadata never proves an old PTY/process is live, and any replacement `TerminalExecution` receives a new `ExecutionId` |

A single `Attempt` may contain multiple `AgentRun`s only when the product/workflow explicitly models cooperating roles or concurrent candidates inside that bounded try. A **fresh retry is never represented merely as another AgentRun in the failed Attempt**.

Two acceptance examples are mandatory:

1. run R1 in Attempt A fails; the user retries the same strategy from scratch; Attempt A keeps its failed disposition/evidence and retry-budget accounting, while R2 is created in Attempt B; if R2 succeeds, both bounded tries remain measurable;
2. a live run temporarily loses its adapter/provider continuation, safely reconnects from retained prerequisites, and continues in the same Attempt/AgentRun without consuming retry budget.

Execution liveness, observation availability, behavioral resumability and work outcome are orthogonal facts and must not be collapsed into one state.

## 3. Universal external Agent Sessions

External agents remain ordinary workloads. Seyal gains richer semantics only through evidence provided by the environment/upstream harness.

### 3.1 Detection/integration tiers

```text
Tier 0: process/PTY lifecycle only
Tier 1: process/shell signals + bounded heuristics
Tier 2: official hooks/events exposed by the external harness
Tier 3: structured AgentAdapter protocol
```

Every non-authoritative state field carries provenance/confidence. Raw terminal text is never trusted for approval, security, billing, audit or final outcome authority.

### 3.2 Capability model

Adapters negotiate optional capabilities rather than forcing a lowest-common-denominator interface.

Candidate capabilities include:

```text
ObserveLifecycle
ObserveActivity
ObserveTaskState
ObserveSubagents
ObserveUsage
ObserveArtifacts
ObserveToolCalls
ObserveCheckpoints
RequestInput
RequestApproval
SendPrompt
Resume
Cancel
Fork
EnumerateSkills
EnumerateTools
```

Unsupported capability is explicit, not inferred.

### 3.3 Capability does not imply enforcement

Every capability also carries an enforcement class:

```text
Observed
  Seyal can observe/report only; the upstream agent may continue independently.

UpstreamRequestable
  Seyal can request a supported upstream action/approval but cannot claim OS-level enforcement.

SeyalEnforced
  the operation is dispatched through Seyal's own typed capability/action plane and policy is enforceable at that boundary.
```

Loss of structured observation must expose degraded/incomplete evidence. Seyal must not claim it paused, denied or fully accounted for an external agent unless the integration actually provides that guarantee.

### 3.4 External harness binding

A binding records at least:

```text
adapter identity/version
external harness kind/version
external session/conversation reference?
observed vs Seyal-started mode
capability + enforcement snapshot
binding generation/fencing token
binding start/end/reconnect metadata
```

External session/conversation references are resumability hints only. Losing them must not corrupt Seyal durable identity/evidence.

Third-party adapters remain outside the authoritative Runtime process unless a later accepted ADR demonstrates a safer/better topology. Adapter failure must not kill or corrupt a live `TerminalExecution`.

## 4. First-party Seyal AI Agent

The first-party agent is a Seyal-owned harness, not a privileged alternate runtime.

```text
WorkItem / Attempt / AgentRun
            |
            v
     SeyalAgentHarness
       |       |        |
       |       |        +--> Action/capability plane (#841)
       |       +-----------> Context Engine / MemoryStore (#681)
       +-------------------> ModelProvider adapter
```

The harness uses the same AgentRun, Artifact, Attention, Evaluation and action/effect semantics used by the external-agent projection model.

### 4.1 Initial provider scope

The first shipped provider for the optional first-party Seyal AI Agent is **OpenAI-only**.

This is a product/implementation scope decision, not a core-domain decision.

Core interfaces remain provider-neutral:

```text
ModelProvider
  describe_capabilities()
  execute_turn(request)
  stream_events(...)
  usage_metadata(...)
  cancel(...)
```

No durable `WorkItem`, `Attempt`, `AgentRun`, `ContextBundle`, `MemoryRecord`, action, permission or evaluation schema may require OpenAI-specific semantics.

Deterministic context retrieval, memory correctness, privacy filtering, evaluation and Universal Agent Sessions must work without a first-party model provider.

Before implementation, provider neutrality is proven with a synthetic materially different provider fixture that exercises missing/unknown usage, different continuation semantics, unsupported content/tool capability and cancellation behavior. Shipping a second production provider is not required to prove the seam.

### 4.2 Provider continuation is an optimization

Provider continuation/session IDs are external references. They never become `AgentRun`, memory or working-state authority.

Seyal distinguishes:

```text
identity survival
  the durable WorkItem/Attempt/AgentRun still exists

historical explainability
  retained local evidence is sufficient to explain supported sent/received/tool decisions

behavioral resumability
  retained local payload/reference classes are sufficient to construct the next valid turn safely
```

These are separate guarantees.

When policy/retention removes prerequisites for behavioral resume, Seyal must report continuation as unavailable instead of pretending hashes, source ranges or provider IDs can reconstruct erased content.

## 5. Data planes and retention availability

| Plane | Durable? | Authority | Purpose |
|---|---:|---|---|
| project/source truth | external/durable | source-policy authority | code, ADRs, specs, instructions, git/worktree state |
| AgentRun event evidence | durable | factual event/effect provenance | lifecycle, artifacts, action/evaluation evidence |
| RunWorkingSet | reconstructable only when dependencies retained | current-run working evidence | bounded recent messages/tools/plans/compaction |
| MemoryStore | durable | scoped reusable claim/knowledge eligibility | decisions/facts/patterns/preferences with provenance |
| ContextBundle | immutable per build, usually ephemeral | one-turn assembled input | exact material selected for one dispatch |
| derived caches/indexes | disposable | none | performance/retrieval acceleration |
| raw provider transcript | retention-policy dependent | conversation evidence only | display/debug/resume where policy allows |

A transcript is **not memory**. A summary is **not source truth**. A cache is **not memory**. A `ContextBundle` is **not durable knowledge authority**.

### 5.1 Three context horizons

```text
Turn context
  = immutable ContextBundle for one request/turn

Run working context
  = bounded recent AgentRun messages/tool results/artifact refs + derived compaction

Long-term memory
  = accepted/eligible reusable MemoryRecords across allowed scopes
```

### 5.2 RunWorkingSet

`RunWorkingSet` is a derived view over retained AgentRun/transcript/action evidence, not a competing authority.

It may contain:

```text
recent user/agent messages
recent tool requests/results
current plan/task-list/checkpoint metadata
selected artifact references
provider continuation reference?
derived compacted summaries with source event/message ranges
working-set dependency set
working-set fingerprint/version
```

A compaction is rebuildable only if its required source payload/reference classes are still retained. If retention removed those inputs, the compaction may be kept only as its own explicitly retained derived evidence if policy permits; it must not be described as reconstructable from absent sources.

### 5.3 Retention availability record

For every resumable run Seyal records policy-safe availability metadata for the classes required by the supported continuation mode, including where applicable:

```text
user instructions/corrections
model-visible assistant messages
pending action intents/results
selected artifact references
current plan/task state
context dependency references
provider continuation reference
retention/policy generation
```

Hidden model reasoning is never required.

If required classes are unavailable, behavioral resume fails closed and requires a fresh safe retry/new `Attempt` or explicit user reconciliation that restores enough eligible state to continue the same bounded try.

## 6. MemoryStore

### 6.1 MemoryRecord

```text
MemoryRecord {
  MemoryId
  scope
  kind
  statement / structured_payload
  applicability
  evidence_refs[]
  provenance
  authority_class
  sensitivity
  confidence?
  state
  created_at
  validated_at?
  revalidate_after?
  expires_at?
  supersedes[]
  conflicts_with[]
  source_fingerprints[]
  record_version
  schema_version
}
```

Recommended kinds:

```text
Decision
EngineeringFact
FailurePattern
Procedure
EnvironmentFact
UserPreference
Heuristic
```

`Accepted` means **eligible for retrieval under current policy**, not "proven true". An AgentRun event can prove that a source reported a claim; it does not make that claim factual.

A memory mirroring an ADR never gains ADR authority; it points back to the ADR.

### 6.2 Scope

Initial scopes:

```text
user-local
project/repository
workspace
worktree
WorkItem/Attempt
```

Cross-scope reads require explicit policy. User-local memory is not implicitly injected into every project. Worktree-local facts do not silently leak into sibling worktrees.

### 6.3 Lifecycle and conflicts

```text
Proposed
   |
   +--> Accepted
          |  \
          |   +--> Superseded
          +-----> Revoked
          +-----> Expired
```

- `Proposed` is not normal accepted knowledge.
- `Accepted` is eligible only while current scope/policy/evidence/revalidation predicates still permit it.
- `Superseded`, `Revoked` and `Expired` are immediately ineligible for normal retrieval; no cleanup/index maintenance job is required to make that eligibility decision effective.

Conflicts do not silently overwrite. Multiple records may coexist with explicit conflict/supersession edges until current authority/evidence resolves them.

Concurrent updates use transactional version checks. Storage race protection does not imply epistemic truth.

### 6.4 Memory modes

```text
Disabled
  no memory read or proposal/write

ReadOnly
  eligible accepted memory may be retrieved; no new memory creation

Curated
  explicit typed human/product actions may create/update memory

Assisted
  eligible evidence may asynchronously propose memory;
  policy decides which low-risk kinds may auto-accept
```

Normative project decisions, security/policy rules, permission expansions, destructive procedures and other high-impact knowledge may not auto-accept solely from model output.

Auto-accepted low-risk kinds require deterministic eligibility/revalidation rules. Repeated proposals, self-referential memory or a model citing its own prior memory cannot launder confidence/authority.

### 6.5 Memory write pipeline

```text
eligible source/evidence
   -> candidate extraction
   -> claim-vs-observation provenance binding
   -> sensitivity/secret policy
   -> duplicate/conflict detection
   -> deterministic eligibility/revalidation policy
   -> Proposed MemoryRecord
   -> human/policy acceptance
   -> Accepted MemoryRecord
```

Model-assisted extraction produces a proposal, never an untraceable authoritative write.

### 6.6 Revocation/forgetting is a use-time boundary

Every `ContextBundle`, `RunWorkingSet` compaction and provider-continuation dependency tracks the memory/source dependencies and relevant privacy/policy generation used to build it.

Revoking/deleting a `MemoryRecord` must:

1. make it immediately ineligible for future selection;
2. increment the relevant memory/privacy revocation generation;
3. invalidate lexical/vector indexes, summaries, working-set compactions and selection/prompt caches that depend on it;
4. make already-built queued bundles from an older generation **undispatchable until revalidated**;
5. prevent provider-continuation reuse when the provider-side continuation may contain the revoked material and Seyal cannot prove that dependency was removed;
6. remove/redact locally retained payload according to retention policy;
7. retain only minimum policy-safe tombstone/provenance needed for consistency and re-extraction suppression.

Immediately before every model/tool/provider dispatch, Seyal rechecks current policy, revocation generation and dependency eligibility. Ordinary freshness staleness may sometimes be intentionally tolerated; **security denial, privacy revocation and permission revocation never may**.

The implementation spec must define the final eligibility/revocation check relative to the **irrevocable external handoff**. Revocation observed before that handoff prevents/rebuilds the dispatch. Revocation after the handoff is classified honestly as already in flight/already transmitted; Seyal must not promise that such content was preventable or retroactively retractable. This ordering must not add locking to the terminal hot path.

If a queued bundle or working-set compaction depends on revoked content, Seyal rebuilds it from still-eligible sources or declares continuation unavailable. If provider continuation contains the revoked dependency, abandon that continuation and rebuild locally; if local retention is insufficient, stop/reconcile instead of sending hidden stale state.

Deleting semantic memory does not erase independent source artifacts, git commits or separately retained AgentRun evidence governed by another retention authority. Already transmitted external-provider content cannot be retroactively unsent.

Pre-revocation evidence may not automatically recreate the same forgotten memory. A policy-safe tombstone/fingerprint suppresses re-proposal from the same retained evidence without retaining the deleted semantic payload. New independent post-revocation evidence may propose a new record only through normal policy.

## 7. Context Engine

```text
Task + AgentRun + RunWorkingSet + policy/capability + budget
        |
        v
Sources
  -> normalize + provenance
  -> permission/sensitivity filter
  -> freshness validation
  -> deterministic retrieval
  -> authority + relevance ranking
  -> MemoryStore retrieval
  -> conflict/deduplication
  -> optional semantic rerank/compaction
  -> budget partitions
  -> immutable ContextBundle + SelectionTrace
```

### 7.1 Context sources

- repository/source files and structure;
- authoritative project instructions, ADRs and specs;
- git/worktree/branch/diff state;
- optional LSP/symbol/index metadata;
- current eligible RunWorkingSet;
- retained AgentRun events/artifacts explicitly eligible for context;
- accepted/eligible MemoryRecords;
- user-pinned context;
- typed external/plugin sources through the provenance contract.

### 7.2 Immutable ContextBundle

```text
ContextBundle {
  ContextBundleId
  WorkItemId / AttemptId / AgentRunId
  ordered_items[]
  source_fingerprints[]
  working_set_fingerprint
  memory_ids[]
  dependency_set
  policy_version
  privacy_revocation_generation
  builder_version
  budget
  token_estimate
  created_at
}
```

Bundles are immutable snapshots. A changed dependency makes a bundle stale. Normal source staleness may follow explicit policy, but dispatch always revalidates privacy/security/permission eligibility as specified in §6.6.

### 7.3 Authority and ranking

Authority and relevance are separate dimensions. Similarity never outranks a current normative instruction merely because it scores higher.

Recommended ordering:

1. security/capability policy;
2. current normative instruction authority;
3. current source/worktree truth;
4. exact task/path/symbol references;
5. eligible scoped memory with valid provenance;
6. current RunWorkingSet/recent typed run evidence;
7. lexical/task/worktree relevance;
8. optional semantic rerank;
9. diversity/deduplication;
10. token-budget selection.

Conflicting/stale memory cannot silently override current source truth.

### 7.4 SelectionTrace

`SelectionTrace` records policy-safe reasons for inclusion/exclusion, provenance, memory/working-set contribution and budget drops without becoming another secret store.

## 8. Durable action/capability/effect plane

The first-party harness needs typed tools without creating a second control model. This reusable M005 primitive is owned by #841 and later consumed by #683 projections.

```text
SeyalAgentHarness
   -> typed ActionIntent
   -> capability/permission/effect authorization
   -> durable action lifecycle
   -> existing authoritative resource/runtime/file/git operation
   -> typed result/evidence
```

### 8.1 Action identity

Before Seyal-controlled dispatch of a mutating or externally effectful operation, persist an `ActionIntent` containing at least:

```text
ActionId
AgentRunId
capability
resource identity + relevant version
normalized arguments fingerprint
effect class
policy version
required approval identity?
created_at
```

Changing arguments, target resource/version, effect class or relevant policy invalidates prior authorization and requires re-evaluation.

### 8.2 Minimal action lifecycle

```text
Prepared
  -> Authorized
      -> Dispatching
          -> Succeeded
          -> FailedKnown
          -> EffectUnknown
          -> CancelledAfterDispatch   # does not imply rollback

Prepared/Authorized
  -> CancelledBeforeDispatch
```

- authorization/approval is bound to exact `ActionId`/run/resource/version/arguments and consumed according to durable single-use/expiry semantics;
- one current dispatch generation/fencing token owns dispatch;
- two workers cannot concurrently consume the same approval/action;
- when crash/restart cannot establish whether a non-replayable effect occurred, state becomes `EffectUnknown`/reconciliation-required;
- ambiguous non-replayable effects are never retried automatically;
- provider/tool idempotency keys are used only when the target genuinely supports trustworthy semantics;
- cancellation after dispatch never claims rollback;
- local durable storage cannot be transactionally atomic with arbitrary external effects, so the design must model uncertainty instead of pretending SQLite solves it.

Affected agent work may pause for reconciliation while unrelated terminal progress continues normally.

### 8.3 External agents

The action lifecycle applies where Seyal controls dispatch. Observed external-agent actions may be recorded as evidence but must not be presented as Seyal-enforced unless the integration guarantees it.

## 9. Storage and derived state

Use the M004 durable local storage/security foundation; backend/table details remain implementation decisions.

Logical namespaces remain separate for:

```text
agent_events
action_intents/action_results
agent_transcript_metadata?   # retention-policy dependent
memory_records/memory_edges
context_source_metadata
working_set_compactions
selection_traces
provider_transmission_log
index/cache metadata
```

Repository bytes remain repository truth.

`provider_transmission_log` is an audit/provenance index, not a second prompt/transcript archive: store policy-safe references/fingerprints and the minimum metadata required to explain the transmission. Do not duplicate secret-bearing prompt/tool payloads merely for logging.

Derived chunks/indexes/embeddings/summaries/selection caches are content/fingerprint/version dependent and rebuildable. Secrets are excluded from persistent memory/derived storage by default unless an accepted policy explicitly permits otherwise.

A dedicated vector database is not required by architecture.

## 10. LSP/code intelligence ownership

LSP is an optional/lazy **context source**, not memory/source/terminal authority.

The cold developer/agent subsystem owns LSP process lifecycle/configuration. Every result is bound to workspace/worktree, document path and document/source version. Unsaved overlay content is a separate versioned context source; it cannot silently masquerade as repository truth. Late diagnostics/symbol results for an old document/index generation are stale and may not become current context authority.

Exact language server vendors, IPC encoding and cache implementation remain implementation choices.

## 11. Skills and MCP

Skills and MCP are harness inputs/extensions, not terminal dependencies.

- skills remain repository/user-discoverable through stable interoperable formats where possible;
- repository content cannot silently install executable adapters/tools;
- MCP/tool servers require explicit trust/install/enable and capability scope;
- MCP/tool outputs become typed AgentRun/action evidence and enter context/memory only through normal provenance/privacy rules.

## 12. Checkpoint/fork semantics

A checkpoint/fork is an immutable logical cut of eligible source/artifact/run evidence plus lineage metadata. It does not rewind a live PTY/process, remote resource or already-observed external effect.

Forking creates new run identity/lineage and fresh authorization. Pending actions/approvals are not inherited automatically. A physical filesystem restore is a separately authorized mutating action through the normal action/effect plane.

## 13. Multi-agent preparation and isolation

M005 establishes single-run identity, context/memory, permissions/effects and evaluation. M006 adds orchestration.

Independent writers default to isolated worktrees. But worktree isolation alone is insufficient: run working context, memory scope, temporary directories, evaluator inputs, shared service state and action resources also carry explicit isolation/provenance.

Independent validation must not silently consume an implementer's mutable test state or accepted memory when that would compromise independence.

Shared-writer mode is deliberate, observable and conflict-detected. Concurrent memory proposals/updates are versioned and preserve conflict/provenance edges.

## 14. Security and failure model

Required threats/failures include:

- malicious/compromised/stale external adapter;
- forged/out-of-order events or duplicate binding writers;
- prompt injection in repository/context/memory content;
- model-generated memory poisoning/confidence laundering;
- secret/PII persistence or provider leakage;
- cross-workspace/worktree/user-memory leakage;
- stale memory overriding current truth;
- memory revocation races with queued bundles/working sets/provider continuation;
- deleted memory being automatically re-extracted from pre-revocation evidence;
- poisoned embeddings/summaries/selection caches;
- capability escalation through tools/MCP/skills;
- approval replay/duplicate dispatch;
- crash after effect but before durable result;
- disk-full/repeated persistence failure;
- provider outage/cancel/reconnect/continuation loss;
- context build/index overload;
- adapter/harness crash while underlying execution remains live;
- Runtime restart with stale process/binding/action evidence;
- GUI close being mistaken for execution/run termination.

Security/privacy filters run before provider/tool dispatch and are rechecked at use time. Context/memory content never gains execution authority merely because it appears in a prompt.

## 15. Performance isolation

Agent features are cold/background relative to terminal I/O.

```text
model / context / memory / index / cache / LSP / persistence / network / actions
                                  X
                                  | never synchronously gates
                                  v
PTY -> VT -> TerminalState -> projection -> Metal
```

Required measurements include both idle and active/failure load:

- zero-agent idle CPU/RSS;
- many detected external-agent sessions idle overhead;
- active event ingest/replay;
- RunWorkingSet rebuild/compaction;
- memory lookup/write/revocation invalidation;
- large-project context/indexing load;
- active provider stream/tool traffic;
- malicious/crash-looping adapter load;
- durable queue saturation and persistent storage failure;
- LSP startup/idle/active resource bounds;
- cache/index disk and RSS bounds;
- proof of no material terminal latency/throughput regression under disabled, enabled-idle and representative active/failure agent loads.

Background work is bounded, cancellable, priority-aware and visibility-aware. Asynchronous work is not automatically harmless: CPU, memory and disk contention must be measured.

## 16. Required test/evaluation matrix before production

### Identity/binding/recovery
- idempotent first detection;
- adapter generation fencing and stale reconnection;
- GUI detach/reconnect;
- external process alive while structured observation disappears;
- first-party worker crash/restart;
- provider loss with/without resumable local state;
- fresh retry always creates a new Attempt + AgentRun and preserves prior Attempt disposition/retry accounting;
- reconnect/resume stays in the same Attempt/AgentRun and consumes no retry budget;
- Runtime restart reconciles persisted metadata versus actual execution/action liveness; replacement TerminalExecution gets a new ExecutionId;
- same-run vs new-run vs new-attempt transition matrix;
- fork lineage and non-inherited pending actions.

### Memory/context/privacy
- MemoryRecord lifecycle/conflict/revalidation/version races;
- `Accepted` claim-vs-truth semantics;
- Superseded/Revoked/Expired memory becomes immediately retrieval-ineligible without waiting for cleanup;
- Assisted-mode auto-accept eligibility and confidence-laundering negatives;
- memory Disabled/ReadOnly/Curated/Assisted modes;
- revocation after bundle build but before irrevocable provider/tool handoff prevents/rebuilds dispatch;
- revocation after irrevocable handoff is classified as already in-flight/transmitted rather than falsely claimed preventable;
- revocation while RunWorkingSet compaction contains memory;
- revocation while provider continuation may contain memory;
- same-evidence re-extraction suppression after forgetting;
- deletion/derived-cache/index invalidation;
- retention loss causing behavioral resume to become unavailable;
- worktree/workspace/user-scope isolation;
- deterministic ContextBundle fingerprints/authority precedence;
- stale LSP/document/index-generation handling.

### Action/effect safety
- authorization/action identity binding;
- concurrent dispatcher fencing;
- approval single-use/expiry/replay;
- crash before dispatch;
- crash after dispatch before result;
- external effect followed by failed local result persistence;
- idempotent vs non-replayable retry behavior;
- cancellation before/after dispatch;
- changed argument/resource/policy reauthorization;
- disk-full/repeated persistence failure without terminal starvation.

### Provider neutrality
- OpenAI adapter failure/cancel/usage handling;
- synthetic second-provider conformance with materially different capabilities;
- unknown usage preserved as unknown;
- unsupported provider features remain explicit capability limits;
- provider continuation loss never changes durable AgentRun identity.

### Evaluation corpus
Measure authoritative-source inclusion, stale-item rate, retrieval precision/recall, memory usefulness/false-positive rate, compaction information retention, task success, retries, latency, tokens and cost. Model-assisted enhancement must beat the deterministic baseline on the same corpus/budget before becoming required.

## 17. Roadmap boundary

While M002 is active:

- Track A/M002 production continues terminal compatibility work only.
- Tracks B/C may proceed as isolated R&D/specification/conformance/evaluation work under #838 and existing R&D owners.
- Do not introduce M005/M006 production code into M002 production branches.

Production dependency shape:

```text
M002 terminal compatibility
  -> M003 workspace
  -> M004 durable local storage/security/product foundation
  -> M005
       #678 WorkItem/Attempt/AgentRun durable authority
           |
           +--> #679 external Agent Sessions/adapters
           +--> #680 Attention/approval/artifact semantics
           +--> #681 Context Engine + MemoryStore + evaluation/routing
           |
           +--> #841 durable capability/action/effect safety
                  depends on exact #680 semantics it consumes
           |
           +--> #839 first-party Seyal AI Agent harness
                  consumes #681 + #841 + exact #680 seams
  -> M006
       #682 workflow/multi-agent orchestration
       #683 coding/SCM/DevOps/LSP + CLI/SDK/MCP/control projections
            consumes #841; does not create a second action/control authority
```

After #678, #679/#680/#681 can proceed in parallel where their exact dependencies are satisfied. #841 may proceed when its exact #680 dependency is accepted. #839 starts only when the exact #681/#841/#680 seams it consumes are ready.

Baseline worktree-scoped memory/context correctness belongs in M005/#681. M006 extends it across workflow nodes; it does not introduce the first safe worktree boundary.

The live roadmap issues have been aligned with this proposed split: #667 records the provider disposition and M005 substrate; #678 owns AgentRun transition/binding authority; #680 owns human approval semantics while #841 owns durable consumption/dispatch; #681 owns MemoryStore/RunWorkingSet/revocation; #839 consumes #681/#841; #683 consumes #841 for later projections.

## 18. Explicit provider product disposition

The product decision for the first implementation is:

```text
Universal Agent Sessions
  remain provider-agnostic and usable without a Seyal first-party model provider

Local Context Engine / MemoryStore / Evaluation
  remain local/provider-neutral and usable without a Seyal first-party model provider

First-party Seyal AI Agent model implementation
  ships initially with OpenAI only

Additional direct/BYOK/local first-party model providers
  remain future roadmap capability, not an M005 launch requirement
```

Existing #667/#681 no-account/local wording is refined to preserve no-account local substrate functionality while not requiring a local model provider for the first-party harness at M005 launch. This decision must still be promoted through the accepted ADR/spec/roadmap process before #839 becomes Ready.

## 19. Decisions intentionally left to implementation ADR/spec detail

The architecture does **not** freeze:

- SQLite vs another accepted embedded transactional backend;
- exact table layout;
- exact worker process topology;
- IPC wire encoding;
- LSP vendor;
- vector index/provider;
- ranking weights;
- cache implementation;
- UI presentation details;
- provider SDK choice.

These choices must obey the frozen authority, recovery, privacy, effect and performance contracts above.

## 20. Promotion gate

This document remains proposed R&D. Before production implementation:

1. independently review the reconciled exact PR head;
2. resolve any remaining blocking findings;
3. promote stable ownership/lifecycle/memory/context/provider/action decisions through dedicated accepted ADR/spec PRs as required by Seyal architecture-change policy;
4. create behavior specs for AgentRun/binding projection, RunWorkingSet/retention availability, MemoryRecord/revocation, ContextBundle/SelectionTrace and Action/effect lifecycle;
5. refine #667/#678–#683/#839/#841 against those accepted authorities;
6. only then mark individual implementation slices Ready according to dependencies.

The reviewer must actively test duplicate authority, binding split-brain, retry/Attempt accounting, revocation races, retention/resume false claims, effect ambiguity/replay, provider lock-in, cross-scope leakage and terminal-resource starvation. Green CI or prose consistency alone is not architecture proof.
