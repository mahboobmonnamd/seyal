# SPEC-014 — M005 RunWorkingSet retention and behavioral resumability

- **Status:** Accepted on merge; specification promotion for #862
- **Issue:** #862
- **Architecture:** `docs/architecture/ADR-012-AGENT-RUN-IDENTITY-LIFECYCLE.md`, `docs/architecture/ADR-013-CONTEXT-DURABLE-MEMORY.md`
- **Parent refinement:** #838
- **Implementation consumer:** #681
- **Sibling contracts:** SPEC-012 / #847 and SPEC-013 / #854

## 1. Purpose and scope

Define the observable M005 contract for `RunWorkingSet`: the bounded, derived run-context state that Seyal may retain for one AgentRun/Attempt, compact over time, and consult when deciding whether the same AgentRun can safely continue after GUI reconnect, Runtime restart, worker/provider loss, or provider-continuation loss.

This specification is subordinate to ADR-012 and ADR-013. It does not create another source authority, MemoryStore, AgentRun authority, transcript authority, provider identity, or Action/effect state machine.

The contract makes three facts explicitly separate:

```text
AgentRun identity survives?
Historical evidence remains explainable?
Behavioral resume is safely available?
```

A yes for either of the first two does not imply the third.

## 2. Authority and ownership

The authorities remain distinct:

```text
source truth / current context sources
  -> owned by their source contracts

MemoryStore / MemoryRecord
  -> durable semantic-memory authority

WorkItem / Attempt / AgentRun + durable events
  -> durable work/run identity and evidence authority

RunWorkingSet
  -> derived, bounded run-context view

provider continuation/session metadata
  -> external optimization/resume reference only

summaries / compactions / indexes / caches
  -> derived and rebuildable where retained prerequisites permit
```

Absolute requirements:

1. `RunWorkingSet` is never source truth.
2. `RunWorkingSet` is never durable semantic memory and cannot replace `MemoryStore`.
3. `RunWorkingSet` never owns WorkItem, Attempt, AgentRun, TerminalExecution, PTY, VT, terminal grid, Action, Approval, or provider identity.
4. Provider continuation/session IDs are references to upstream state and never Seyal durable identity.
5. A summary or compaction cannot become authoritative merely because the original retained payload was later removed.
6. RunWorkingSet work is control/background-plane work and must never synchronously gate terminal progress.

## 3. Identity and scope

A RunWorkingSet is owned by exactly one `AgentRunId` and therefore one owning Attempt under ADR-012.

Conceptually it carries at least:

```text
RunWorkingSetId
WorkItemId
AttemptId
AgentRunId
working_set_generation
scope identity
retention availability
ordered retained/derived entries
dependency set
policy/privacy generation
builder/compactor version
created_at
last_compacted_at?
provider_continuation_ref?
```

The exact serialized schema is an implementation detail, but these semantics are mandatory.

Requirements:

- `RunWorkingSetId` is not reused for materially different owning run identity;
- a new fresh retry from scratch under ADR-012 creates a new Attempt + AgentRun and therefore cannot silently inherit the old RunWorkingSet as mutable current state;
- a replacement worker generation for the same safely resumable AgentRun may consume the same logical working state only after all resume prerequisites are revalidated;
- cross-AgentRun, cross-Attempt, sibling-worktree, unrelated-workspace or unrelated-repository reuse is denied by default;
- provider/model/account identity never merges working-set scopes.

## 4. Working-set entry classes

A working set may reference or retain policy-eligible run-context classes such as:

```text
UserInstructionOrCorrection
ModelVisibleAssistantMessage
ContextBundleDependencyRef
MemoryRecordDependencyRef
AgentRunEventOrEvidenceRef
PendingActionOrResultRef
ArtifactRef
PlanOrTaskState
ToolOrCapabilityObservation
ProviderContinuationRef
DerivedSummaryOrCompaction
```

Each entry must identify:

```text
entry identity
entry class
origin/provenance refs
owning scope
source/dependency generation(s)
authority class where applicable
sensitivity class
retention class
availability state
payload retained? / reconstructable? / reference-only?
created/updated generation
```

Retaining a payload does not increase its authority. Reference-only entries do not prove that the referenced payload remains available.

## 5. Retention classes and availability

Retention policy must distinguish whether information required for future continuation is actually available.

At minimum, each prerequisite is classified as one of:

```text
RetainedPayload
ReconstructableFromCurrentAuthority
ReferenceOnly
Unavailable
RevokedOrForbidden
Stale
```

Meanings:

### 5.1 `RetainedPayload`

The required payload is locally retained, policy-eligible and bound to current provenance/generation.

### 5.2 `ReconstructableFromCurrentAuthority`

The original derived payload need not be retained because an equivalent required input can be deterministically reconstructed from still-authorized current source authority without relying on hidden provider/model state or erased data.

Reconstructability must be proven by the owning source contract. Hashes, filenames, source ranges, embeddings, provider IDs, or summaries do not by themselves prove reconstructability.

### 5.3 `ReferenceOnly`

Only an identifier/reference remains. This may support explainability but does not satisfy a resume prerequisite that requires the original payload.

### 5.4 `Unavailable`

The required retained payload is absent and cannot currently be reconstructed safely.

### 5.5 `RevokedOrForbidden`

Current privacy/security/policy makes the prerequisite ineligible for reuse even if bytes remain physically present.

### 5.6 `Stale`

The retained/reconstructable prerequisite is bound to an old source, context, memory, worktree, policy or builder generation and has not been validly refreshed.

Availability is evaluated from current authority at use/recovery time, not inferred from a previous successful build or provider session.

## 6. Retention availability summary

A working set exposes a deterministic retention-availability summary covering every prerequisite class needed for the next safe continuation step. Runtime supplies a typed `ContinuationPlan` bound to the exact WorkItem/Attempt/AgentRun and binding generation; it enumerates each next-step dependency as `required` or `optional`, its typed source and payload requirement, and the current plan generation. Unknown dependency classes, unknown requiredness, or a missing/stale plan are treated as required-but-unverified and yield `ReconciliationRequired`, never omission by default.

At minimum it accounts for relevant classes including:

- user instructions/corrections required to preserve intent;
- model-visible assistant/context messages required to preserve semantic continuity;
- pending action/result references required to avoid duplicate or contradictory work;
- artifacts and plan/task state;
- selected ContextBundle dependencies;
- selected MemoryRecord dependencies;
- provider continuation reference and known eligibility generation;
- current policy/privacy generation.

The summary is evidence about availability, not a replacement for the payload or source.

A stale availability summary is not proof of current availability.

## 7. Dependency completeness and generation fencing

Every retained payload, working-set compaction, summary, provider-continuation dependency and resume decision must carry the dependencies needed to invalidate it when eligibility changes.

Dependencies include, as applicable:

```text
AgentRun binding generation
worktree/repository/workspace identity
ContextBundleId + relevant source/dependency generations
MemoryId + MemoryRecord version/state/revocation generation
artifact/action/result generation
policy/privacy generation
provider continuation generation/reference
builder/compactor version
```

Requirements:

- stale ContextBundle or MemoryRecord state cannot remain current merely because it was copied into a working set;
- revocation/supersession/expiry/deletion of a dependency invalidates or narrows the affected working-state eligibility before reuse;
- a working-set generation never widens a dependency generation silently;
- a late compaction built from an older working-set generation cannot overwrite a newer invalidation/revocation result;
- implementations use version-aware/CAS-equivalent mutation so competing compaction/invalidation writers cannot both claim current authority.

## 8. Compaction and summaries

Compaction exists to bound retained run context; it is not a truth-making operation.

A compaction may replace eligible retained run-context material with a smaller derived representation only when the consumer contract explicitly permits derived representation for that information.

A compaction must preserve:

- provenance/dependency references required to judge current eligibility;
- owning AgentRun/Attempt/scope;
- authority no stronger than the least-authoritative required input;
- sensitivity no lower than the most-sensitive required input;
- unresolved conflicts/material uncertainty needed for correct continuation;
- information required to avoid action/result replay hazards where the action/effect contract requires it;
- policy/privacy generation needed for later revalidation.

A compaction must not:

- convert model narration into source truth;
- turn a provider transcript into durable semantic memory;
- erase the fact that required payload is no longer retained;
- claim that a summary reconstructs deleted/revoked source payload;
- drop a required unresolved conflict and present consensus;
- remove dependency identities merely to reduce metadata;
- retain forbidden secret/plaintext payload through a summary, embedding, trace or cache.

If safe compaction cannot fit the configured bound without losing a required resume prerequisite, the implementation records reduced retention availability. It does not silently claim full resumability.

## 9. Behavioral resume classification

Behavioral resume is an explicit advisory classification, separate from AgentRun identity and execution liveness. It does not commit an AgentRun recovery-state transition; only the Runtime/domain transition authority under ADR-012 §3 may commit durable AgentRun recovery state.

At minimum the recovery decision is one of:

```text
BehavioralResumeAvailable
ReconciliationRequired
ResumeUnavailable
```

### 9.1 `BehavioralResumeAvailable`

Allowed only when every prerequisite required for the next continuation step is:

1. current and policy-eligible; and
2. either `RetainedPayload` or `ReconstructableFromCurrentAuthority`; and
3. consistent with the current AgentRun binding/recovery state under ADR-012; and
4. free of unresolved ambiguous external effects that require reconciliation.

The continuation may rebuild locally from eligible retained/current authority. An opaque provider-continuation/session reference never satisfies a required payload prerequisite and never by itself proves hidden state is intact, authorized, current, unexpired, or sufficient for the next step. A provider continuation may be used only as an optional optimization under a typed provider contract that attests the current exact AgentRun binding, checkpoint/content generation, privacy/policy generation, expiry, and required capabilities; otherwise abandon it and rebuild locally or return `ReconciliationRequired`. Provider continuation is never a requirement when local prerequisites suffice.

### 9.2 `ReconciliationRequired`

Used when the same AgentRun identity can remain meaningful but a safe next step requires explicit reconciliation before work continues, including cases such as:

- ambiguous action/effect outcome;
- conflicting retained/current evidence that cannot be deterministically resolved;
- provider continuation may contain now-ineligible content and must be abandoned while local state needs reconciliation;
- persisted run metadata and actual worker/execution state disagree after restart;
- adapter/observation loss leaves prior execution liveness unknown;
- a required prerequisite may exist but current validity/availability cannot yet be established safely.

Reconciliation cannot fabricate missing payload.

### 9.3 `ResumeUnavailable`

Used when any required prerequisite is `Unavailable`, `RevokedOrForbidden`, or irrecoverably `Stale`, and safe reconstruction from current authority is not possible. `ResumeUnavailable` does not establish that the prior execution has ended. A fresh retry may begin only after Runtime-authoritative proof that the previous execution terminated or was explicitly cancelled. If execution liveness is unknown, classify `ReconciliationRequired`; if it is known live, preserve the current Attempt/AgentRun and recover or reconcile it rather than starting a fresh retry.

The product must not present this as a successful resume merely because:

- AgentRun metadata still exists;
- a provider continuation/session ID exists;
- hashes/source ranges remain;
- a summary remains;
- an old ContextBundle/working-set snapshot exists;
- the previous worker once reported success.

Starting work again from scratch follows ADR-012 fresh-retry semantics: new Attempt + new AgentRun. The prior run retains its disposition/evidence and is not rewritten into the new try.

## 10. Reconnect, restart and replacement-worker behavior

The following events are distinct:

### 10.1 GUI/client reconnect

A GUI reconnect that does not interrupt the authoritative Runtime/worker does not itself require behavioral reconstruction. Existing live state remains authoritative. Reconnect metadata never creates a new AgentRun or Attempt by itself.

### 10.2 Runtime restart

Persisted metadata does not prove a PTY, worker, provider stream or external process remains live.

After Runtime restart, Seyal reconciles:

```text
persisted AgentRun identity/evidence
actual worker/execution liveness
current binding generation
working-set retention availability
provider continuation eligibility
pending/ambiguous action state
```

A replacement TerminalExecution gets a new ExecutionId where ADR-012/runtime authority requires it. RunWorkingSet metadata cannot resurrect an old PTY/process.

### 10.3 Replacement worker for same AgentRun

A replacement worker generation may resume the same AgentRun only when ADR-012 permits same-run recovery and this specification yields `BehavioralResumeAvailable` after current revalidation.

A stale worker/binding generation cannot mutate the current working set after replacement.

### 10.4 Fresh retry

If recovery requires restarting the bounded try from scratch, create a new Attempt + AgentRun. Reusing the old RunWorkingSet as writable current state is forbidden. Policy-safe evidence/source references may be consumed as normal current inputs through their owning contracts.

## 11. Provider continuation semantics

Provider continuation/session state is external optimization metadata only.

Requirements:

- provider conversation/session/thread IDs never replace WorkItem/Attempt/AgentRun identity;
- existence of a continuation ID does not prove hidden provider state is intact, current, authorized or sufficient;
- provider continuation loss does not destroy Seyal identity/evidence;
- if required local prerequisites remain, Seyal may rebuild context and continue the same AgentRun when ADR-012 and §9 permit it;
- if required local retained payload is unavailable, provider continuation loss makes behavioral resume unavailable rather than triggering fabricated reconstruction;
- if a provider continuation may contain a revoked/forbidden dependency, that continuation becomes ineligible and is abandoned for future use; rebuild only from still-eligible local sources where sufficient;
- provider-specific continuation metadata must remain outside provider-neutral durable core semantics.

This specification does not claim retroactive deletion of content already transmitted to a provider.

## 12. Privacy, revocation and deletion interaction

Every queued bundle, working-set compaction, provider-continuation dependency and derived run-context cache records the relevant dependency set and privacy/policy revocation generation.

Before working state is reused for continuation, current dependency eligibility is rechecked.

Required behavior:

- a revoked MemoryRecord immediately becomes unusable through working-set copies/summaries/references according to the owning privacy policy;
- stale compactions that contain revoked/ineligible content cannot be selected merely because they predate revocation;
- provider continuation that may embed revoked content is abandoned for future continuation when the current policy requires that content not be reused;
- locally retained working-set payload follows the same effective sensitivity/retention constraints as its inputs;
- derived summary/cache/index deletion/minimization cannot be bypassed by another working-set representation;
- if permitted suppression/audit metadata remains, it must be policy-safe and non-reconstructive.

The dedicated privacy/revocation specification owns dispatch-time irrevocable-handoff ordering, physical deletion completion, and user-visible forgetting-completion claims.

## 13. Scope and isolation

RunWorkingSet scope must prevent accidental context transfer.

At minimum:

- sibling worktrees do not share mutable working state;
- unrelated workspaces/repositories do not share working state by path coincidence;
- a new Attempt/AgentRun does not inherit prior mutable state merely because task text is the same;
- external provider account/session identity does not create shared scope;
- submodule/nested-repository identity remains distinct when retained dependencies reference it;
- user-level eligible source/memory may be consumed only through its normal policy, not because another run previously copied it.

A product may explicitly create a typed handoff/artifact/context reference between runs. That creates a new scoped input with provenance; it is not hidden RunWorkingSet sharing.

## 14. Persistence truthfulness

Persisting a RunWorkingSet is optional product behavior, but if persisted:

- persistence stores derived state, not another authority;
- durable commit success/failure is explicit;
- an acknowledged durable generation is never reported unless committed;
- corrupted/unknown-version entries are quarantined/ineligible;
- retention cleanup/expiry is bounded;
- disk-full or repeated persistence failure degrades behavioral resume availability honestly rather than blocking terminal execution or pretending state was saved;
- crash recovery revalidates dependencies instead of trusting persisted `current=true` flags.

No persistence mechanism may claim to restore a live PTY/process/provider stream solely from journaled metadata.

## 15. Failure and degraded behavior

Required behavior:

- missing optional retained entry -> omit/degrade if continuation correctness permits;
- missing required retained entry -> `ResumeUnavailable` unless safely reconstructable;
- stale dependency -> revalidate/rebuild or become reconciliation/unavailable;
- invalid/revoked dependency -> remove from eligibility and rebuild/reconcile as allowed;
- provider continuation unavailable -> local rebuild if sufficient, otherwise unavailable;
- adapter/observation loss while execution is live does not change retention availability or permit `ResumeUnavailable`/fresh retry; unknown liveness is `ReconciliationRequired` until Runtime establishes termination or explicit cancellation;
- compaction failure -> keep prior current valid generation if policy/limits permit, otherwise reduce availability explicitly;
- persistence corruption -> quarantine affected derived generation, preserve independent source/memory/run authorities;
- repeated failure -> bounded retry/backoff; no hot loop;
- cancellation -> stop background compaction/rebuild without corrupting prior authoritative run evidence;
- disk/resource pressure -> bounded eviction following retention priority, with resulting resume availability recalculated honestly.

No degraded path may silently create a new MemoryRecord, claim source truth, retry an ambiguous effect, or gate unrelated terminal progress.

## 16. Security requirements

The implementation must defend against at least:

- untrusted terminal/provider/model text poisoning retained working state;
- provider/session ID spoofing or cross-run attachment;
- cross-workspace/worktree/AgentRun leakage;
- stale working-set generation overwriting a newer revocation/invalidation;
- summary/compaction authority laundering;
- sensitivity downgrading through summarization;
- revoked secret retained through compaction/cache/index/provider continuation reuse;
- path/source identity aliasing in retained dependencies;
- replay of stale pending-action/result references;
- corrupt persisted state being treated as current;
- resource-exhaustion through unbounded transcript/context retention or compaction churn.

Raw shell/terminal output and model/provider narration remain untrusted evidence unless an owning typed integration grants a stronger evidence class; RunWorkingSet retention cannot upgrade that class.

## 17. Terminal hot-path isolation

None of the following may synchronously gate:

```text
PTY -> byte stream -> VT/parser -> TerminalState -> damage/projection -> Metal
```

- RunWorkingSet persistence;
- retention accounting;
- compaction/summarization;
- dependency revalidation;
- provider continuation calls;
- context/memory reconstruction;
- resume classification;
- cleanup/expiry;
- indexing/cache writes;
- retry/backoff.

Under active compaction, persistence failure, provider failure or retention pressure, unrelated terminal I/O/rendering must continue within accepted terminal performance budgets.

## 18. Resource and performance requirements

Before #681 becomes Ready, implementation evidence must bound and measure at least:

- resident/durable bytes per working set and aggregate;
- retained-entry count and metadata overhead;
- compaction CPU/RSS/latency;
- persistence write/read/recovery cost;
- resume-classification/revalidation latency;
- cleanup/expiry/eviction behavior;
- concurrent independent AgentRun working sets;
- provider-continuation-loss fallback cost;
- repeated persistence/provider failure backoff;
- queue saturation/backpressure;
- terminal latency/throughput isolation during active and failing working-set work.

Calibrated production budgets and reproducible measurement procedure must be recorded in versioned repository evidence before #681 Ready/acceptance.

## 19. Required deterministic tests

At minimum:

1. RunWorkingSet is bound to exactly one AgentRun/Attempt and cannot alias a sibling run;
2. provider continuation ID does not replace AgentRun identity;
3. `ReferenceOnly` prerequisite cannot satisfy a payload-required resume;
4. a current retained payload can satisfy a resume prerequisite;
5. deterministic reconstruction from current authorized source can satisfy a reconstructable prerequisite;
6. a hash/source range/provider ID alone cannot make unavailable payload reconstructable;
7. stale ContextBundle dependency invalidates affected working state;
8. revoked/superseded/expired MemoryRecord cannot remain eligible through working-set copy/cache;
9. revocation racing a compaction wins eligibility and stale compaction cannot overwrite it;
10. derived summary never increases authority or decreases sensitivity;
11. compaction preserves unresolved conflict/provenance required for continuation;
12. compaction unable to retain required resume information reduces availability instead of claiming full resume;
13. GUI reconnect with live Runtime/worker does not create new Attempt/AgentRun;
14. Runtime restart does not treat persisted metadata as proof of live PTY/worker/provider state;
15. replacement worker generation can resume same AgentRun only after safe revalidation;
16. stale worker generation cannot mutate current working state;
17. provider continuation loss + sufficient local prerequisites permits same-run continuation when ADR-012 allows;
18. live execution plus adapter/observation loss preserves retention availability and never starts a fresh retry;
19. unknown execution liveness after observation loss yields `ReconciliationRequired`; a fresh retry requires Runtime proof of termination or explicit cancellation;
20. provider continuation loss + missing required retained payload yields `ResumeUnavailable`;
21. ambiguous external effect yields `ReconciliationRequired`, never blind retry;
22. fresh retry from scratch creates new Attempt + AgentRun and does not reuse old working set as mutable current state;
23. provider continuation that may contain revoked content is abandoned for future use;
24. sibling worktree/workspace/run cannot read another run's working state by path/task/provider coincidence;
25. unknown/newer persisted schema generation is quarantined;
26. corruption of RunWorkingSet does not corrupt MemoryStore/source/AgentRun evidence authority;
27. disk-full/repeated persistence failure yields bounded retries and honest reduced resumability;
28. bounded eviction recalculates availability deterministically;
29. cancellation leaves prior current generation coherent;
30. restart reconciliation rejects stale `current` markers/generations;
31. malformed dependency/scope/provider identifiers are rejected/fuzzed;
32. sustained working-set/compaction failure does not synchronously stall terminal progress.

Property/fuzz tests are required for generation ordering, dependency-set composition, scope identity, retained-entry decoding and malformed provider/dependency identifiers.

## 20. Acceptance criteria

SPEC-014 is acceptable when independent review establishes that:

- RunWorkingSet is explicitly derived and cannot become a second MemoryStore/source/event authority;
- sibling run/worktree/workspace payload cannot be read or resumed through path, task, provider or identifier coincidence;
- AgentRun identity, historical explainability and behavioral resumability are separate facts;
- retention availability is explicit and deterministic;
- hashes/provider IDs/summaries cannot falsely reconstruct erased or unavailable payload;
- compaction preserves provenance/authority/sensitivity/conflict constraints;
- stale/revoked dependencies invalidate before reuse;
- provider continuation is an optimization and never durable Seyal identity;
- reconnect/restart/replacement-worker behavior is consistent with ADR-012;
- missing required retained state yields reconciliation/unavailable rather than a fabricated resume;
- fresh restart of the bounded try creates a new Attempt + AgentRun;
- persistence/resource/failure behavior is bounded and honest;
- provider/model neutrality is preserved;
- terminal hot-path isolation is explicit and testable;
- no Action/effect or dedicated privacy-race authority is redefined here.

## 21. Explicit non-goals / deferred behavior

This specification does not define:

- MemoryRecord lifecycle/memory modes/conflicts (SPEC-012 / #847);
- ContextBundle selection/ranking/filesystem/LSP invalidation (SPEC-013 / #854);
- dispatch-time privacy/revocation race and irrevocable external-handoff ordering;
- physical deletion completion or user-visible forgetting-completion claims;
- Action/Approval/effect dispatch/reconciliation semantics (ADR-014 and later specs);
- workflow DAG/multi-agent scheduling;
- provider-specific transcript/message schema;
- a database/object-store format;
- production implementation authorization.

If implementation requires behavior outside these boundaries that changes authority, identity, privacy, recovery, compatibility, security or terminal isolation, stop and refine the owning ADR/spec before coding.
