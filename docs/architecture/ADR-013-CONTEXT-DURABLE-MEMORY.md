# ADR-013 — Local context, durable memory and revocation authority

- **Status:** Accepted on merge
- **Date:** 2026-09-09
- **Issue:** #838
- **Scope:** OSS local context/memory ownership, provenance, retention/resumability, privacy revocation, invalidation and isolation semantics
- **Research:** [`agent-rd/SEYAL-LOCAL-CONTEXT-ENGINE-RD-001.md`](agent-rd/SEYAL-LOCAL-CONTEXT-ENGINE-RD-001.md), [`agent-rd/SEYAL-AGENT-MEMORY-CONTEXT-ARCHITECTURE-RD-001.md`](agent-rd/SEYAL-AGENT-MEMORY-CONTEXT-ARCHITECTURE-RD-001.md)
- **Depends on:** ADR-007 and ADR-012

## Context

ADR-007 established Workspace as a durable domain/security/context boundary and rejected provider transcript/session identity as durable work authority. ADR-012 now accepts the `Workspace -> WorkItem -> Attempt -> AgentRun` identity/lifecycle authority used by both external Agent Sessions and the first-party Seyal AI Agent.

The remaining M005 context/memory boundary cannot safely be left to implementation convention. Without one accepted authority, separate implementations could make any of the following mistakes:

- treat a raw provider transcript, summary, cache or vector index as durable memory;
- let stale semantic memory override current repository/ADR/spec truth;
- leak worktree-local or workspace-local facts into another scope;
- construct a prompt before a privacy revocation and dispatch it after the revocation;
- reuse a provider continuation that still contains forgotten/private material;
- claim behavioral resume from hashes or metadata after required payload was deleted;
- allow model-generated claims to become trusted durable truth without provenance;
- retain secret-bearing explainability/index/cache data after the source was excluded;
- synchronously block terminal progress on indexing, retrieval, compaction, persistence or model work.

The reviewed #838/#840 R&D converged on a provenance-first Local Context Engine plus a separate inspectable `MemoryStore`. This ADR accepts those ownership, lifecycle, revocation and isolation decisions. It does not choose storage tables, vector technology, ranking weights, provider SDKs or commercial routing services.

## Decision

### 1. Context and memory are separate authorities

Seyal keeps the following planes distinct:

| Plane | Authority / durability | Purpose |
|---|---|---|
| project/source truth | external/durable source authority | code, ADRs, specs, instructions, git/worktree state |
| AgentRun event evidence | durable factual/provenance evidence | lifecycle, artifacts, actions, evaluation observations |
| `RunWorkingSet` | derived current-run working evidence | bounded recent messages/tools/plans/artifacts/compaction |
| `MemoryStore` | durable scoped reusable knowledge eligibility | inspectable claims/decisions/facts/procedures/preferences |
| `ContextBundle` | immutable per-build input snapshot | exact selected material for one consumer/dispatch |
| derived indexes/caches/summaries/embeddings | disposable/rebuildable | retrieval/performance enhancement |
| raw provider transcript/continuation | retention-policy dependent external evidence | display/debug/resume optimization where permitted |

A transcript is not memory. A summary is not source truth. A cache/index is not memory. A `ContextBundle` is not durable knowledge authority.

### 2. `MemoryStore` is the sole durable semantic-memory authority

Durable reusable semantic memory is represented by versioned, inspectable `MemoryRecord`s owned by one local `MemoryStore` authority.

A record carries at least the semantic concepts required to express:

```text
MemoryId
scope
kind
statement / structured payload
applicability
evidence/provenance references
authority class
sensitivity
state
created/validated/revalidation/expiry metadata
supersession/conflict edges
source fingerprints
record/schema version
```

Recommended record kinds include `Decision`, `EngineeringFact`, `FailurePattern`, `Procedure`, `EnvironmentFact`, `UserPreference` and `Heuristic`.

No provider-specific hidden memory, transcript store, workflow cache or M006 subsystem may become a competing durable semantic-memory authority.

### 3. Memory lifecycle is explicit and does not imply truth

The minimum lifecycle is:

```text
Proposed
   |
   +--> Accepted
          |  \
          |   +--> Superseded
          +-----> Revoked
          +-----> Expired
```

`Accepted` means **eligible for retrieval under current policy**. It does not mean "proven factual truth".

An AgentRun event can prove that a source reported a claim; it cannot make that claim true merely by being persisted. A memory mirroring an ADR/spec never gains the ADR/spec's authority; it retains a provenance reference to that source.

Conflicting records are preserved with explicit conflict/supersession relationships until current authority/evidence resolves them. Similarity, recency or repeated model restatement cannot silently overwrite stronger authority.

### 4. Memory creation is provenance-bound and policy-controlled

Memory modes are:

```text
Disabled  -> no memory read or proposal/write
ReadOnly  -> eligible memory may be read; no creation/update
Curated   -> explicit typed human/product actions create/update memory
Assisted  -> eligible evidence may asynchronously propose memory;
             deterministic policy may auto-accept only explicitly allowed low-risk kinds
```

The write path is conceptually:

```text
eligible evidence
  -> candidate extraction
  -> claim-vs-observation provenance binding
  -> sensitivity/secret policy
  -> duplicate/conflict detection
  -> deterministic eligibility/revalidation policy
  -> Proposed MemoryRecord
  -> human/policy acceptance when required
  -> Accepted MemoryRecord
```

Model-assisted extraction produces a proposal, not untraceable authority. Normative architecture/security/policy rules, permission expansions, destructive procedures and other high-impact knowledge cannot auto-accept solely from model output.

Repeated proposals, self-reference or a model citing its own prior memory must not launder confidence or authority.

### 5. Scope and isolation are explicit

Initial memory/context scopes include:

```text
user-local
project/repository
workspace
worktree
WorkItem / Attempt
```

Cross-scope reads require explicit policy. In particular:

- user-local memory is not implicitly injected into every project;
- worktree-local dirty/untracked facts do not leak into sibling worktrees;
- sharing a provider/model does not merge workspace truth;
- cross-repository/submodule items retain their own repository/revision provenance;
- shared immutable derivatives may be reused only when content identity **and** policy/scope eligibility allow it.

Worktree isolation alone is not sufficient. `RunWorkingSet`, memory scope, temporary/derived state and evaluator/context inputs also preserve explicit scope/provenance.

### 6. The Local Context Engine is provenance-first

The canonical assembly pipeline is:

```text
Task + AgentRun + RunWorkingSet + policy/capabilities + budget
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

Eligible sources may include repository files/structure, authoritative instructions/ADRs/specs, git/worktree state, versioned LSP/symbol/index results, current `RunWorkingSet`, eligible AgentRun evidence/artifacts, accepted `MemoryRecord`s, user-pinned context and typed external/plugin sources.

Source discovery does not execute content.

### 7. Authority and relevance are different dimensions

Context selection must preserve project authority ordering rather than using one opaque similarity score.

At minimum:

1. security/capability policy;
2. current normative instruction authority;
3. current source/worktree truth;
4. exact task/path/symbol references;
5. eligible scoped memory with valid provenance;
6. current RunWorkingSet/run evidence;
7. lexical/task/worktree relevance;
8. optional semantic rerank;
9. diversity/deduplication;
10. token-budget selection.

A semantically similar stale memory or summary cannot outrank a current ADR/spec/source fact merely because it scores higher.

### 8. `ContextBundle` is immutable and dependency-tracked

A `ContextBundle` records the exact ordered input selection plus sufficient identity to prove its dependencies and eligibility, including conceptually:

```text
ContextBundleId
WorkItemId / AttemptId / AgentRunId
ordered items
source fingerprints
working-set fingerprint
memory ids
full dependency set
policy version
privacy/revocation generation
builder version
budget/token estimate
created_at
```

Once built, the bundle is immutable. If a dependency changes, the bundle may become stale; it is rebuilt/revalidated rather than mutated invisibly.

Ordinary source freshness may follow an explicit bounded policy. **Privacy, security and permission eligibility must always be revalidated at use time before dispatch.**

### 9. `SelectionTrace` explains selection without becoming a secret store

`SelectionTrace` records policy-safe inclusion/exclusion reasons, provenance, important ranking components, memory/working-set contribution and budget drops.

For excluded secret/policy-denied material, the persisted trace stores only minimum policy-safe metadata. It must not preserve source bytes, snippets, embeddings, summaries or locators when those would reconstruct or reveal excluded content.

Explainability cannot become a second retention path around privacy policy.

### 10. `RunWorkingSet` is derived run context, not durable memory

`RunWorkingSet` is a bounded derived view over retained AgentRun/transcript/action/artifact evidence. It may include recent messages/tool results, current plan/task state, selected artifact references, provider-continuation reference and derived compactions with source ranges/dependencies.

It is not a competing durable knowledge store.

A compaction is rebuildable only while its required source payload/reference classes remain retained. If policy removes those inputs, Seyal must not claim the compaction can be reconstructed from hashes or metadata alone.

### 11. Identity survival, explainability and behavioral resumability are separate

For an AgentRun, Seyal distinguishes:

```text
identity survival
  durable WorkItem/Attempt/AgentRun identity still exists

historical explainability
  retained local evidence is sufficient to explain supported past decisions/events

behavioral resumability
  retained eligible payload/reference classes are sufficient to construct the next valid turn safely
```

Retention availability records the policy-safe availability of relevant classes such as user instructions/corrections, model-visible assistant messages, pending action/result references, artifacts, plan/task state, context dependencies, provider continuation reference and policy generation.

Hidden model reasoning is never a resume prerequisite.

If required retained payload is unavailable, behavioral resume is **unavailable**. Hashes, source ranges or provider IDs cannot be claimed to reconstruct erased payload. The system requires explicit reconciliation or a fresh safe retry under ADR-012 semantics.

### 12. Privacy/forgetting revocation is enforced at use time

Every queued `ContextBundle`, `RunWorkingSet` compaction, derived cache/index and provider-continuation dependency records the relevant dependency set and privacy/policy revocation generation.

Revoking/deleting a memory or source eligibility must:

1. immediately make it ineligible for future selection;
2. advance the relevant revocation generation;
3. invalidate dependent lexical/vector indexes, summaries, working-set compactions and selection/prompt caches;
4. make already-built queued bundles from an older generation undispatchable until revalidated/rebuilt;
5. prevent reuse of a provider continuation when Seyal cannot prove the revoked dependency is absent;
6. remove/redact locally retained payload according to policy;
7. retain only minimum policy-safe tombstone/provenance needed for consistency and re-extraction suppression.

Immediately before model/provider/tool dispatch, Seyal rechecks current policy/revocation generation and dependency eligibility.

Security denial, privacy revocation and permission revocation never take an intentionally stale path.

If a provider continuation may contain revoked content, abandon that continuation and rebuild from still-eligible local sources. If local retention is insufficient, stop/reconcile rather than silently sending hidden stale state.

Already transmitted external-provider content cannot be retroactively unsent; Seyal must not claim otherwise.

### 13. Forgetting cannot be silently undone by the same evidence

After a semantic memory is forgotten/revoked, pre-revocation retained evidence must not automatically recreate the same forgotten memory.

A policy-safe tombstone/source fingerprint may suppress re-proposal from that same evidence without retaining the deleted semantic payload. New independent post-revocation evidence may propose a new record only through the normal policy pipeline.

Deleting semantic memory does not automatically delete independent source artifacts, git commits or separately retained AgentRun evidence governed by another retention authority.

### 14. Filesystem/repository provenance is explicit

Context discovery preserves real repository/worktree boundaries:

- non-ignored untracked files inside the authorized worktree may be eligible and are worktree-scoped/fingerprinted;
- ignored files are excluded from automatic discovery by default but `.gitignore` is not a security boundary;
- symlinks cannot silently expand authorized source roots; external roots require explicit source authorization;
- submodules/nested repositories retain separate repository/revision/dirty-state provenance;
- rename/delete/untracked-to-tracked, symlink-target, submodule revision and policy changes invalidate affected derivatives.

Derived data may be shared by immutable content hash only when repository/policy/sensitivity scope permits it.

### 15. LSP/symbol/index data is an optional versioned context source

LSP and language-index results are context enrichment, not source truth or terminal authority.

Every result is bound to workspace/worktree, path/document source version and index generation. Unsaved overlays are separate versioned context sources. Late diagnostics/symbol results for an old generation are stale and cannot silently become current authority.

No language server is required for context correctness.

### 16. Provider/model neutrality is an architecture requirement

The Local Context Engine, `MemoryStore`, `RunWorkingSet`, `ContextBundle`, `SelectionTrace`, privacy/revocation and deterministic retrieval correctness operate without a first-party model provider.

The initial first-party Seyal AI Agent may use OpenAI only, but no OpenAI-specific durable type enters these context/memory authorities.

Optional model-assisted summarization, semantic retrieval or memory extraction remains derived, provenance-bound and removable. Shipping a vector database or a second production provider is not required by this ADR.

### 17. Storage and indexing technology remain implementation decisions

This ADR does not choose the durable storage backend, table schema, vector implementation, ranking weights or cache data structure.

Requirements are semantic:

- transactional/version-aware updates where concurrent durable memory mutations require them;
- bounded/cancellable background work;
- dependency-addressed invalidation;
- rebuildable derived indexes/caches;
- source bytes remain source authority rather than being copied into a competing authoritative database;
- persistent derived storage obeys sensitivity/retention policy.

SQLite/FTS or other mechanisms may be selected later if they satisfy these contracts. A dedicated vector database is not an architectural requirement.

### 18. Terminal hot-path isolation remains absolute

None of the following may synchronously gate:

```text
PTY -> VT/parser -> TerminalState -> damage/projection -> Metal
```

- context discovery/indexing/retrieval;
- memory lookup/write/revalidation/revocation;
- RunWorkingSet compaction;
- persistence/cache/index writes;
- LSP/model/embedding work;
- provider continuation checks;
- evaluation/routing/cloud/licensing/telemetry.

Background work is bounded, cancellable, priority-aware and measured under active/failure load. Asynchronous CPU/RSS/disk contention is still a performance concern and must be benchmarked.

## Required downstream specifications

Before #681 or the first-party harness can become Ready, behavior specs must define at least:

1. `MemoryRecord` schema/lifecycle/scope/conflict/revalidation and memory modes;
2. `ContextItem` / `ContextBundle` / `SelectionTrace` selection, provenance, filesystem and invalidation behavior;
3. `RunWorkingSet` and retention-availability / behavioral-resume semantics;
4. privacy/revocation use-time race behavior, provider-continuation abandonment and same-evidence re-extraction suppression.

Those specs consume this ADR; they may not invent another context or memory authority.

## Explicitly out of scope

This ADR does not define:

- AgentRun identity/lifecycle (ADR-012 already owns it);
- durable `ActionIntent`, approval consumption, effect ambiguity or dispatch fencing (separate action/effect ADR);
- workflow DAG/multi-agent scheduling;
- evaluation verdict/outcome formulas or learned routing algorithms;
- provider SDK/model portfolio or model-specific harness policy;
- commercial learned routing, shared team/org memory, managed indexing/cloud execution or enterprise governance;
- a concrete storage backend/table schema/vector database;
- user-facing memory UX.

## Alternatives rejected

### Transcript or provider session as memory authority

Rejected because it couples durable knowledge to one harness/provider, increases privacy/retention cost and fails when provider continuation disappears.

### One global mutable context shared by all agents/worktrees

Rejected because it creates cross-worktree leakage, stale shared assumptions and untraceable multi-agent contamination.

### Vector database as the architectural foundation

Rejected because exact source/authority/provenance semantics do not require it and it adds an unnecessary storage/privacy/invalidation authority before measured benefit exists.

### LLM-only context selection or memory acceptance

Rejected because it is non-deterministic, difficult to inspect, can launder model claims into authority and creates circular dependence on a model for basic correctness.

### Delete only future memory lookup, leave queued prompts/continuations untouched

Rejected because a privacy revocation between bundle construction and dispatch would still transmit revoked data.

### Hashes are enough for behavioral resume after payload deletion

Rejected because hashes prove identity/integrity; they cannot reconstruct erased content.

## Consequences

Positive:

- external and first-party agents can share one inspectable local knowledge/context substrate without sharing hidden provider state;
- memory remains durable yet subordinate to current source/architecture authority;
- privacy/forgetting has an enforceable dispatch-time boundary rather than best-effort cache cleanup;
- worktree/workspace isolation is explicit;
- provider switching does not require a memory/context migration;
- context selection and resumability remain explainable;
- OSS correctness does not depend on cloud, commercial services or a vector database.

Costs:

- every derived bundle/compaction/cache/continuation needs dependency/policy-generation bookkeeping;
- revocation may force local rebuild or make behavioral continuation unavailable;
- memory conflict/provenance lifecycle is more complex than an opaque chat-memory blob;
- context indexing must be bounded and measured to prevent background resource contention.

## Security/failure cases required downstream

Specs/tests must cover at least:

- prompt/context/memory poisoning and confidence laundering;
- secret/PII exclusion and SelectionTrace leakage;
- cross-workspace/worktree/user leakage;
- revocation after bundle build but before dispatch;
- revocation while compaction/provider continuation contains the dependency;
- same-evidence re-extraction after forgetting;
- retention loss causing resume-unavailable;
- conflicting memories and stale source truth;
- symlink/submodule/untracked/ignored-file boundary cases;
- stale LSP/document/index generations;
- cache/index poisoning and invalidation;
- repeated persistence failure/disk pressure;
- active indexing/memory/revocation load without terminal starvation.

## Revisit conditions

Revisit this ADR only with evidence that:

- another durable semantic-memory authority is necessary and can avoid split-brain ownership;
- a different privacy-revocation boundary can prove no stale queued/continuation path exists;
- provider-side state can become durable portable authority without lock-in or retention ambiguity;
- observed scale requires a different context/memory partition while preserving provenance and isolation;
- a new shared/team memory architecture requires changes to the local OSS authority rather than composing above it.

Storage-engine selection, embedding model choice, ranking weights, UI presentation and commercial routing do not by themselves reopen this ADR.