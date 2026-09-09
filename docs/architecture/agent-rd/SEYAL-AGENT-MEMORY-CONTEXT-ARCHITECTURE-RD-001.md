# Seyal Agent Memory, Context and Harness Architecture R&D

**Status:** Proposed refinement; not accepted ADR/spec authority  
**Issue:** #838  
**Related:** #48, #52, #54, #57, #255, #645, #667, #678–#683  
**Scope:** OSS architecture and roadmap refinement only. No production implementation is authorized by this document.

## 1. Purpose

Seyal needs two capabilities at the same time:

1. a **universal Agent Sessions experience** for external CLI agents; and
2. a **first-party Seyal AI Agent** with a strong local harness, context, memory, tools, permissions, evaluation and later multi-agent orchestration.

These must compose over the same durable Seyal identities without creating a second terminal/runtime authority or making future provider changes require a domain rewrite.

The terminal path remains independent:

```text
PTY -> VT/parser -> TerminalState -> damage/projection -> Metal
```

Agent, memory, context, model, indexing, evaluation, workflow, persistence, cloud, telemetry and licensing work must never synchronously gate terminal progress.

## 2. Non-negotiable ownership model

### 2.1 Durable authority

```text
Workspace
  -> WorkItem
      -> Attempt
          -> AgentRun
              -> ExecutionRef(s)
              -> ArtifactRef(s)
              -> AttentionRef(s)
              -> EvaluationRef(s)
```

- `Workspace` owns product/workspace identity.
- `WorkItem` owns the durable user goal and final accepted outcome.
- `Attempt` owns one distinct attempt at the WorkItem.
- `AgentRun` owns one agent/harness execution history within an Attempt.
- `TerminalExecution` continues to own its PTY/process/terminal state where terminal execution is involved.
- provider/harness conversation/session IDs are external references only.

### 2.2 Agent Session is a product projection, not a second authority

The user-facing **Agent Session** surface is a projection over an `AgentRun` plus its current adapter binding, capability snapshot, execution references and attention/artifact state.

```text
Agent Session UI
      |
      v
AgentRun authority
      |
      +-- external harness binding? -> AgentAdapter
      |
      +-- first-party harness?      -> SeyalAgentHarness
      |
      +-- execution refs
      +-- artifacts / attention / evaluations
```

Do **not** introduce a separate durable `AgentSession` state machine competing with `AgentRun`.

An externally started CLI agent detected inside a Seyal terminal may create/bind an `AgentRun` with an explicit origin such as `external_detected`. The external harness's own session identifier remains metadata, not Seyal identity.

### 2.3 Agent execution ownership and GUI detach

The GUI must not own continued agent work.

- an external CLI agent inside a `TerminalExecution` survives GUI detach exactly according to the terminal/runtime persistence contract;
- a first-party API-driven Seyal agent uses a supervised non-terminal `Execution`/agent worker owned by the durable runtime/execution layer, not by an AppKit/Swift view controller;
- tool commands that require a terminal create/reference normal Runtime-owned `TerminalExecution` objects rather than giving the harness its own PTY implementation;
- adapter/UI detach may stop observation temporarily but must not fabricate AgentRun termination when the underlying execution remains live;
- reconnect rebinds projections/adapters to existing durable identities.

The precise worker/process topology remains a later ADR decision, but GUI lifetime may not become agent lifetime authority.

## 3. Universal external Agent Sessions

External agents remain ordinary workloads. Seyal gains richer semantics only through negotiated evidence.

### 3.1 Detection/integration tiers

```text
Tier 0: process/PTY lifecycle only
Tier 1: process/shell signals + bounded heuristics
Tier 2: official hooks/events exposed by the external harness
Tier 3: structured AgentAdapter protocol
```

Every state field carries provenance/confidence where the source is not authoritative structured data.

Raw terminal text is never trusted for approval, security, billing, audit or final outcome authority.

### 3.2 Capability model

Adapters negotiate explicit optional capabilities rather than forcing a lowest-common-denominator interface.

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

Unsupported capability is an explicit state, not an error and not guessed from terminal text.

### 3.3 External harness binding

An adapter binding records at least:

```text
adapter identity/version
external harness kind/version
external session/conversation reference?
observed vs Seyal-started ownership mode
capability snapshot + generation
binding start/end/reconnect metadata
```

External session/conversation references are resumability hints only. They never replace `AgentRunId`, and losing them must not corrupt Seyal's durable work/evidence model.

### 3.4 Failure containment

- adapter crash/restart must not kill a live TerminalExecution;
- stale/out-of-order adapter events are rejected or reconciled through AgentRun event ordering rules;
- third-party adapters stay out of the authoritative Runtime process unless a future ADR explicitly proves another design safer;
- no adapter may own PTY, VT, TerminalState, terminal grid or renderer state.

## 4. First-party Seyal AI Agent

The first-party agent is a Seyal-owned harness, not a privileged alternate runtime.

```text
WorkItem / Attempt / AgentRun
            |
            v
     SeyalAgentHarness
       |     |      |
       |     |      +--> Tool/Capability plane
       |     +---------> Context Engine
       +---------------> Model Provider Adapter
```

The harness uses the same AgentRun events, Artifact, Attention, Evaluation and permission semantics that external adapters project into.

### 4.1 First implementation provider scope

The **initial first-party model implementation may be OpenAI-only** to keep the first production vertical narrow.

That is an implementation constraint, not a core-domain constraint.

Core interfaces must remain provider-neutral:

```text
ModelProvider
  describe_capabilities()
  create_response(request)
  stream_events(...)
  usage_metadata(...)
  cancel(...)
```

No core `WorkItem`, `AgentRun`, `ContextBundle`, `MemoryRecord`, tool, permission or evaluation schema may contain OpenAI-specific semantics that another provider would require migrating later.

Deterministic context retrieval, memory correctness, privacy filtering and evaluation truth must work with **no model provider at all**.

This preserves the option to add other providers later as adapters without committing Seyal to supporting them in the first release.

### 4.2 Provider continuation is an optimization, not authority

Some providers support server-side conversation/response continuation identifiers. Seyal may use them for efficiency, but stores them only as external references.

Seyal must retain enough local AgentRun/run-working evidence to:

- explain what was sent and received;
- rebuild the next required context when provider continuation is unavailable;
- avoid making provider-side hidden history the only copy of critical work state;
- migrate/resume through another provider adapter in the future without migrating durable Seyal identities.

A provider continuation reference may improve cache/resume behavior; it never becomes `AgentRun` or memory authority.

## 5. Data planes and context horizons

These are intentionally separate.

| Plane | Durable? | Authority | Purpose |
|---|---:|---|---|
| project/source truth | external/durable | highest according to source policy | code, ADRs, specs, instructions, git state |
| AgentRun event evidence | durable | factual execution evidence | lifecycle, artifacts, tool/evaluation provenance |
| RunWorkingSet | reconstructable/durability-policy dependent | current-run working evidence only | short-term conversation/tool state and compaction |
| MemoryStore | durable | scoped semantic knowledge, never automatic source truth | reusable decisions/facts/patterns/preferences |
| ContextBundle | immutable per build, usually ephemeral | request-specific assembled input | what one model/harness turn receives |
| derived caches/indexes | disposable | none | performance/retrieval acceleration |
| raw provider transcript | retention-policy dependent | conversation evidence only | display/debug/resume where supported |

A transcript is **not memory**. A cache is **not memory**. A summary is **not source truth**. A ContextBundle is **not durable knowledge authority**.

### 5.1 Three context horizons

Seyal distinguishes:

```text
Turn context
  = immutable ContextBundle for one request/turn

Run working context
  = bounded recent AgentRun messages/tool results/artifact refs + derived compaction
    needed to continue the current AgentRun

Long-term memory
  = accepted reusable MemoryRecords that may be retrieved across eligible runs/scopes
```

This avoids two common failure modes: treating the entire transcript as permanent memory, or relying on provider-side conversation state as the only short-term working context.

### 5.2 RunWorkingSet

`RunWorkingSet` is a derived/reconstructable view over AgentRun evidence and eligible transcript retention, not a new durable authority.

It may contain:

```text
recent user/agent messages
recent tool requests/results
current plan/task-list/checkpoint metadata
selected artifact references
provider continuation reference?
derived compacted summaries with source event ranges
working-set fingerprint/version
```

Compaction summaries always retain source event/message ranges and model/config fingerprints. They can be discarded/rebuilt without losing authoritative AgentRun events/artifacts.

## 6. MemoryStore

### 6.1 MemoryRecord

A durable memory record is structured and inspectable:

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
  confidence?              # only when meaningful
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

Recommended kinds include:

```text
Decision
EngineeringFact
FailurePattern
Procedure
EnvironmentFact
UserPreference
Heuristic
```

A memory that merely mirrors an ADR does not gain ADR authority; the ADR remains the authority and the memory points to it.

### 6.2 Scope

Initial scopes:

```text
user-local
project/repository
workspace
worktree
WorkItem/Attempt
```

Cross-scope reads require explicit policy. Worktree-local facts must never silently leak into sibling worktrees merely because file paths match.

User-local records are not implicitly injected into every project; project eligibility is policy-scoped and inspectable.

Team/organization synchronized memory is a later consumer of the OSS seam and must not be required for local correctness.

### 6.3 Lifecycle

```text
Proposed
   |
   +--> Accepted
          |  \
          |   +--> Superseded
          +-----> Revoked
          +-----> Expired
```

- `Proposed`: candidate extracted or explicitly suggested; not eligible as accepted knowledge unless policy allows proposed-memory retrieval for inspection.
- `Accepted`: eligible for retrieval within scope/policy.
- `Superseded`: retained for provenance but not selected as current knowledge by default.
- `Revoked`: explicitly withdrawn from normal retrieval.
- `Expired`: validity window elapsed; requires revalidation before normal use.

Conflicts do not silently overwrite. Multiple records can coexist with explicit conflict/supersession edges until authority/evidence resolves them.

Concurrent updates use transactional version checks so two agents cannot silently overwrite each other's accepted memory state.

### 6.4 Memory modes

Memory behavior is independently configurable from context retrieval:

```text
Disabled
  no memory read and no memory proposal/write

ReadOnly
  accepted eligible memory may be retrieved; no new memories created

Curated
  explicit user/typed product actions may create/update memory

Assisted
  eligible run evidence may produce proposed memories asynchronously;
  policy decides which kinds require confirmation and which low-risk kinds may auto-accept
```

No mode permits opaque provider-side hidden memory to become Seyal authority.

Normative project decisions, security/policy rules, permission expansions, destructive procedures and other high-impact knowledge may **not** be auto-accepted solely from model output. Their authority must come from an accepted source or an explicit typed human/product action.

### 6.5 Memory write pipeline

```text
eligible evidence/event/artifact
      -> candidate extraction
      -> provenance binding
      -> sensitivity/secret policy
      -> conflict/duplicate detection
      -> validation/revalidation policy
      -> Proposed MemoryRecord
      -> optional human/policy acceptance
      -> Accepted MemoryRecord
```

Model-assisted extraction is optional and must produce a **proposal**, not an untraceable authoritative write.

A model cannot write accepted memory solely by emitting natural language. Any automatic acceptance is a Seyal policy decision with an inspectable rule and evidence requirements.

### 6.6 Deletion and forgetting semantics

Deleting/revoking a MemoryRecord must:

1. make it immediately ineligible for new ContextBundles;
2. invalidate its lexical/vector indexes, summaries and selection caches;
3. prevent stale prompt-bundle cache reuse;
4. remove/redact local stored payloads according to retention policy;
5. preserve only the minimum tombstone/provenance needed for consistency where required.

Deleting memory does not pretend to erase an independent source artifact, git commit or separately retained AgentRun evidence. Those have their own retention authority.

Seyal also cannot claim that local deletion retroactively removes content already transmitted to an external model/tool provider. Every provider transmission must have inspectable provenance, and provider-side deletion/retention controls are applied only when the provider exposes a trustworthy supported mechanism.

## 7. Context Engine

The Local Context Engine builds request-specific context from current truth and eligible durable knowledge.

```text
Task + AgentRun + RunWorkingSet + capability/policy + context budget
        |
        v
Sources
  -> normalize + provenance
  -> permission/sensitivity filter
  -> freshness validation
  -> deterministic candidate retrieval
  -> authority + relevance ranking
  -> MemoryStore retrieval
  -> conflict + duplicate handling
  -> optional semantic rerank / compaction
  -> budget partitions
  -> immutable ContextBundle + SelectionTrace
```

### 7.1 Context sources

- repository/source files and structure;
- authoritative project instructions, ADRs and specs;
- git/worktree/branch/diff state;
- local symbol/LSP/index metadata where enabled;
- current RunWorkingSet;
- retained AgentRun events/artifacts explicitly eligible for context;
- accepted MemoryRecords;
- user-pinned context;
- typed external/plugin sources through the provenance contract.

### 7.2 Immutable bundle semantics

Each `ContextBundle` is immutable and fingerprinted.

```text
ContextBundle {
  ContextBundleId
  WorkItemId / AttemptId / AgentRunId
  ordered_items[]
  source_fingerprints[]
  working_set_fingerprint
  memory_ids[]
  policy_version
  builder_version
  budget
  token_estimate
  created_at
}
```

The next model turn may build a new bundle or a cache-friendly delta, but the previous bundle is never silently mutated.

If a dependency changes, the existing bundle becomes stale evidence. The builder either rebuilds affected context or explicitly records that stale material was intentionally retained.

### 7.3 Authority and ranking

Authority and relevance are independent dimensions.

A semantically similar memory or source file cannot outrank a current normative project instruction merely because similarity is higher.

Recommended ordering logic:

1. security/capability policy;
2. current normative instruction authority;
3. current source/worktree truth;
4. exact task/path/symbol references;
5. accepted scoped memory with valid evidence;
6. current RunWorkingSet and recent typed run evidence;
7. lexical/task/worktree relevance;
8. optional semantic rerank;
9. diversity/deduplication;
10. token-budget selection.

Memory that conflicts with current source truth is marked conflicted/stale and excluded or surfaced with explicit warning according to policy.

### 7.4 SelectionTrace

`SelectionTrace` explains included/excluded classes and reasons without becoming a secret store.

It records enough to answer:

- why was this item selected?
- why was another excluded?
- which policy/freshness rule applied?
- which memory record contributed?
- which working-set summary/event range contributed?
- what was dropped for budget?

Secret-denied candidates retain only policy-safe identifiers/reasons.

## 8. Context budget and stable-prefix strategy

The context builder divides budget intentionally rather than concatenating arbitrary history:

```text
system/security policy
project instructions
current task + user request
current execution/worktree state
retrieved source context
eligible memory
bounded RunWorkingSet / recent run-tool evidence
reserved tool/model response headroom
```

Stable prefixes may be fingerprinted for provider prompt caching where supported, but provider prompt-cache metadata remains an adapter concern.

Provider-side prompt caching and Seyal local caches are different mechanisms.

## 9. Storage recommendation

### 9.1 Durable local metadata

Use the M004 durable local storage foundation. SQLite remains the leading local metadata/index candidate unless M004 chooses another embedded transactional authority.

Logical tables/namespaces should remain separate for:

```text
agent_events
agent_transcript_metadata?   # only under explicit retention policy
memory_records
memory_edges
context_source_metadata
working_set_compactions
selection_traces
provider_transmission_log
index_metadata
cache_metadata
```

Repository bytes remain repository truth; do not create an authoritative duplicate source database.

Sensitive storage uses the M004 security/key-storage contract; secrets are excluded from persistent memory/derived storage by default unless an explicit accepted policy says otherwise.

### 9.2 Derived storage

Content-addressed blobs/indexes may hold:

- chunks;
- lexical indexes;
- symbol metadata;
- optional embeddings;
- derived summaries/working-set compactions;
- context-selection caches;
- prompt-bundle cache entries.

All derived data carries dependency fingerprints + builder/model/config/policy versions and is rebuildable.

A dedicated vector database is not required for the baseline architecture. Semantic indexes remain optional providers justified by evaluation evidence.

## 10. LSP/code intelligence relationship

LSP and language-aware indexes are **context sources**, not memory authority.

They may contribute:

```text
diagnostics
symbols
definitions/references
completion metadata
format/lint information
```

They stay optional/lazy/bounded and cannot become always-resident terminal dependencies.

## 11. Tool and permission plane

The first-party harness needs typed tools without creating a second control model.

The durable design is:

```text
SeyalAgentHarness
   -> typed capability request
   -> local capability/permission service
   -> existing authoritative resource/runtime/file/git operation
   -> typed result/event
```

Later CLI/SDK/MCP projections should reuse the same capability semantics rather than invent another permission model.

Permission policy evaluates at least:

```text
actor / AgentRun
resource scope
operation capability
effect class
workspace/project/user policy
sensitivity
approval requirement
expiry/replay identity
```

Human approval produces a typed approval bound to the exact action/capability/resource/run. Terminal text never counts as approval.

## 12. Skills and MCP

Rules/skills and MCP are harness inputs/extensions, not terminal dependencies.

- skills remain repository/user-discoverable through stable interoperable formats where possible;
- repository content cannot silently install executable adapters/tools;
- MCP/tool servers have explicit trust/install/enable and capability scope;
- tool outputs become typed AgentRun evidence/artifacts and may be eligible context only through normal provenance/privacy rules.

## 13. Multi-agent preparation

M005 establishes single-run identities, context/memory, permissions and evaluation. M006 adds orchestration.

The same model must support:

```text
WorkItem
  -> Attempt A -> AgentRun planner
  -> Attempt B -> AgentRun implementer -> isolated worktree
  -> Attempt C -> AgentRun validator   -> independent evidence
```

or cooperating child AgentRuns where explicitly configured.

Independent writers default to isolated worktrees. Shared-writer mode is deliberate, observable and conflict-detected. Memory/context scope never silently crosses worktrees.

Concurrent memory proposals/updates are transactionally versioned and retain conflict/provenance edges; one agent cannot silently rewrite another agent's accepted memory.

A code/workflow checkpoint never claims to rewind a live PTY/process or external side effect.

## 14. Security and failure model

Required threats/failures include:

- malicious/compromised external adapter;
- forged/out-of-order agent events;
- prompt injection in repository/context/memory content;
- model-generated memory poisoning;
- secret/PII persistence or provider leakage;
- cross-workspace/worktree memory leakage;
- stale memory overriding current truth;
- memory deletion leaving retrievable derived indexes;
- previously transmitted provider content being mistaken for locally erasable state;
- poisoned embeddings/summaries/selection caches;
- capability escalation through tools/MCP/skills;
- approval replay;
- provider outage/cancel/reconnect;
- provider continuation loss;
- context build/index overload;
- adapter/harness crash while the underlying execution remains live;
- GUI close being mistaken for AgentRun/execution termination.

Security policy and provenance filters run before provider transmission. Context/memory content never receives execution authority merely because it appears in a prompt.

Provider transmission records must be privacy-aware: store references/fingerprints and required audit metadata without creating a second secret-bearing prompt archive.

## 15. Performance isolation

Agent features are cold/background relative to terminal I/O.

```text
model / context / memory / index / cache / LSP / persistence / network
                         X
                         | never synchronously gates
                         v
PTY -> VT -> TerminalState -> projection -> Metal
```

Required budgets/measurements include:

- zero-agent idle CPU/RSS;
- many detected external-agent sessions idle overhead;
- event ingest/replay cost;
- RunWorkingSet rebuild/compaction cost;
- memory lookup/write/invalidation latency;
- large-project context retrieval/build latency;
- cache/index disk and RSS bounds;
- provider-stream processing overhead;
- proof of no terminal latency/throughput regression with agent subsystem disabled and enabled-idle.

Background indexing/extraction/compaction is bounded, cancellable, priority-aware and visibility-aware.

## 16. Tests and evaluation required before production

### Functional/state tests
- AgentRun/session projection lifecycle;
- external adapter binding/ownership/reconnect semantics;
- adapter capability negotiation and unsupported states;
- external adapter crash/reconnect/stale events;
- first-party worker survives GUI detach according to accepted execution-lifetime contract;
- provider continuation loss falls back to Seyal working evidence without identity loss;
- RunWorkingSet reconstruction/compaction lineage;
- MemoryRecord lifecycle/supersession/conflict/revalidation/version races;
- memory Disabled/ReadOnly/Curated/Assisted modes;
- high-impact memory cannot auto-accept from model output;
- deletion/derived-cache invalidation;
- worktree/workspace isolation;
- deterministic ContextBundle fingerprints;
- stale source/memory/working-context detection;
- authority precedence;
- provider adapter swap conformance at core boundaries;
- OpenAI adapter failure/cancel/usage handling;
- permissions/approval replay/expiry.

### Security tests
- secret exclusion before memory persistence and model transmission;
- prompt/memory poisoning fixtures;
- path/symlink/submodule boundaries;
- malicious adapter/MCP frames;
- cross-scope leakage;
- forged evidence/memory proposals;
- provider-transmission metadata cannot reconstruct excluded secrets;
- local delete semantics never falsely claim provider-side erasure.

### Evaluation corpus
Measure context retrieval precision/recall, authoritative-source inclusion, stale-item rate, memory retrieval usefulness/false-positive rate, compaction information retention, task success, retries, latency, tokens and cost. Model-assisted enhancement must beat the deterministic baseline on the same corpus/budget before becoming required.

## 17. Roadmap boundary

While M002 is active:

- M002 production continues terminal compatibility work only.
- Agent Sessions and Seyal AI Agent work may proceed in parallel as R&D/specification/conformance-fixture work under #838 and existing R&D owners.
- Do not introduce M005 production code into M002 branches.

Production order remains:

```text
M002 terminal compatibility
  -> M003 workspace
  -> M004 durable local storage/product foundation
  -> M005 agent-native local substrate
       #678 identity/event authority first
       then #679 / #680 / #681 can proceed in parallel when exact dependencies are satisfied
       plus the first-party Seyal AI Agent harness package (#839) after #838 architecture acceptance
  -> M006 workflows + multi-agent + coding/DevOps surfaces
```

The first-party harness work package is a previously missing roadmap owner and must not be hidden inside #679 external adapters or #681 context/evaluation.

## 18. Existing provider-scope conflict that must be resolved explicitly

Current agent-platform roadmap/R&D language includes no-account/BYOK/local-provider operation. The product decision to ship the **first-party Seyal AI Agent initially with OpenAI only** narrows that earlier expectation.

This refinement resolves the architectural risk by keeping core provider contracts neutral, but it does **not** silently rewrite the existing product requirement.

Before M005 readiness, product/ADR review must choose one of these explicit dispositions:

1. OpenAI is the first shipped provider for the first-party agent, while local/BYOK provider support remains a later accepted roadmap requirement; or
2. M005 exit criteria continue to require at least one account-free/local model path for the first-party agent.

External Agent Sessions and deterministic local context/memory remain usable independently of that choice.

## 19. Promotion gate

This document is deliberately **not** final architecture authority.

Before implementation:

1. independent high-capability architecture review of #838 and this document;
2. reconcile all blocking findings;
3. explicitly resolve the provider-scope conflict in §18;
4. promote stable decisions into accepted ADR(s) covering ownership/lifecycle/storage/provider/permission boundaries;
5. create behavior specs for AgentAdapter/AgentRun projection, RunWorkingSet, MemoryRecord and ContextBundle/SelectionTrace;
6. refine #678–#683 and #839 against those accepted specs;
7. only then mark implementation slices Ready according to roadmap dependencies.

The independent reviewer must actively try to find duplicate authority, hidden provider lock-in, memory/context conflation, privacy/deletion gaps, replay/side-effect mistakes, multi-agent isolation gaps and terminal hot-path coupling rather than merely checking prose consistency.
