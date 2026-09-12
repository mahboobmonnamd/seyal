# SPEC-015 — M005 privacy revocation, continuation fencing and forgetting completion

- **Status:** Accepted on merge; specification promotion for #870
- **Architecture:** ADR-013, ADR-014
- **Parent refinement:** #838
- **Implementation consumer:** #681
- **Related specifications:** SPEC-012 / #853, SPEC-013 / #856, SPEC-014 / #863

## 1. Purpose

This specification defines observable behavior for privacy/security revocation after context or working state has already been constructed but before it is reused or externally dispatched.

It covers:

- privacy/policy generation binding;
- queued `ContextBundle` invalidation;
- `RunWorkingSet`/summary/compaction/index/cache invalidation;
- provider-continuation eligibility and abandonment;
- local forgetting completion semantics;
- same-evidence anti-resurrection;
- races with memory acceptance/revalidation and Action dispatch;
- failure/resource behavior and terminal isolation.

This specification does not create another context, memory, Action, provider-session or retention authority.

## 2. Authority boundaries

The authoritative planes remain:

```text
source / repository / policy authority
MemoryStore / MemoryRecord authority
AgentRun durable evidence
RunWorkingSet derived run context
ContextBundle + SelectionTrace derived dispatch input
provider continuation/session external optimization state
Action authority / dispatch boundary
```

Rules:

1. `MemoryStore` remains the only durable semantic-memory authority.
2. `ContextBundle`, `SelectionTrace`, summaries, embeddings, indexes, caches and provider continuation metadata remain derived/non-authoritative.
3. `Action` remains the sole Seyal-controlled effect/dispatch authority under ADR-014.
4. Provider continuation/session identity never replaces `WorkItemId`, `AttemptId`, `AgentRunId` or `ActionId`.
5. Revocation logic must not synchronously gate `PTY -> VT -> TerminalState -> damage/projection -> Metal`.

## 3. Terms

### 3.1 Revocation generation

A monotonic generation associated with a policy/scope domain that changes whenever eligibility of previously selectable/transmittable material is revoked, deleted or materially narrowed.

A generation proves ordering only. It does not prove that every external provider has deleted previously transmitted content.

### 3.2 Dependency set

The complete policy-safe identities/generations needed to decide whether a derived record may still be reused or dispatched.

A dependency set may include:

```text
scope identity + generation
policy generation
privacy/revocation generation
MemoryId + version/state generation
source identity + source version/fingerprint
RunWorkingSet generation
ContextBundle dependencies
provider-continuation eligibility generation
```

### 3.3 Local forgetting completion

Local forgetting is complete only when all required local retained payload and derived-state obligations for the revoked semantic content are satisfied according to policy.

It does **not** imply data already transmitted to an external provider has been retroactively erased.

### 3.4 Irreversible external handoff

The point after which Seyal can no longer guarantee that revoked bytes were not externally transmitted or an external effect was not initiated.

For a model/provider request, this is the accepted provider transport handoff defined by the provider adapter.

For a Seyal-controlled effectful `Action`, ADR-014 `Dispatching` remains the durable conservative boundary for effect ambiguity. Privacy eligibility must be revalidated immediately before that safety-critical dispatch transaction.

## 4. Canonical revocation event

A revocation event records at least:

```text
RevocationEventId
scope identity
subject identity / semantic suppression identity
prior generation
new generation
reason class
policy generation
requested_at
committed_at
source/provenance reference when safe
local payload disposition requirement
provider-continuation disposition requirement
```

The revocation event itself must not copy forbidden payload merely to explain the deletion.

## 5. Revocation precedence

When revocation conflicts with concurrent memory/context work, the more restrictive decision wins.

Normative rules:

1. A committed revocation wins over concurrent acceptance/revalidation/supersession that was based on an older revocation generation.
2. A worker operating on generation `N` cannot publish reusable state after generation `N+1` is committed unless it revalidates against `N+1`.
3. A stale index/cache/summary result cannot make revoked content eligible again.
4. A failed or delayed background invalidation task does not make old content dispatchable.
5. Unknown generation/freshness at use time fails closed for the affected content/dispatch.

Version-aware/CAS-style mutation semantics are required where concurrent durable updates can conflict. Concrete database technology is not specified.

## 6. Revocation propagation contract

After revocation commits, the system must conceptually perform:

```text
commit revocation generation
  -> memory/source becomes ineligible immediately
  -> prevent new selection/reuse from old generation
  -> invalidate queued ContextBundles
  -> invalidate affected RunWorkingSet compactions/summaries
  -> invalidate prompt/selection/retrieval caches
  -> invalidate lexical/vector/semantic derived indexes as needed
  -> mark provider continuation unsafe when absence cannot be proven
  -> remove/redact local retained payload according to policy
  -> retain minimum policy-safe suppression/tombstone metadata
```

Physical cleanup may complete asynchronously, but **eligibility/dispatch denial is immediate once revocation commits**.

## 7. Queued ContextBundle behavior

A `ContextBundle` is immutable.

If any dependency was revoked or its relevant privacy/policy generation changed after bundle construction:

- the bundle becomes `UndispatchableStale`;
- it cannot be repaired by deleting one field in place;
- the consumer must rebuild/revalidate from current eligible sources;
- old payload retained for debugging/history follows retention/redaction policy and cannot remain a secret archive;
- a stable bundle hash cannot override current eligibility.

### 7.1 Final use-time check

Immediately before model/provider/tool dispatch that consumes a bundle, Seyal verifies:

```text
bundle generation is current
all dependency generations remain eligible
scope identity remains current
policy generation remains current
privacy/revocation generation remains current
required source/memory versions are still eligible
provider continuation, if used, is still eligible
```

Any failure stops that dispatch and produces a typed stale/revoked result; it does not silently fall back to old content.

## 8. RunWorkingSet / compaction behavior

A `RunWorkingSet`, compaction or summary that depends on revoked content cannot be reused merely because the derived text does not contain an obvious verbatim match.

Rules:

1. Every reusable compaction/summary retains a dependency set sufficient to invalidate it.
2. Revoked dependency => affected derived payload is unavailable for future use until rebuilt from still-eligible sources.
3. Compaction cannot launder authority or sensitivity.
4. Hashes/source ranges do not reconstruct erased payload.
5. If required retained input no longer exists, SPEC-014 behavioral resumability becomes `ResumeUnavailable` or `ReconciliationRequired` as applicable.
6. A provider conversation that contains the old compaction is governed by provider-continuation fencing in §10.

## 9. Index/cache/embedding behavior

Derived retrieval structures are optimization state only.

After revocation:

- old entries are logically ineligible immediately, even before physical deletion completes;
- query-time filtering/revocation generation checks must prevent stale hits from becoming selected context;
- background deletion/rebuild is bounded and retry-limited;
- persisted vectors/embeddings/summaries obey the same sensitivity/retention policy as the source dependency;
- traces must not keep snippets/locators that reconstruct deleted secret content;
- unknown/newer schema generations are quarantined/ineligible.

A stale cache hit can never override authoritative `MemoryRecord`/source eligibility.

## 10. Provider continuation fencing

Provider continuation/session state is external optimization state and may contain content that Seyal cannot inspect or selectively erase.

A provider continuation is eligible only when Seyal can establish that:

```text
provider continuation belongs to the same permitted AgentRun scope
provider continuation reference is still current
all Seyal-known content dependencies remain eligible
no relevant revocation generation advanced since the continuation checkpoint
provider capability/policy permits reuse
```

If revoked content **may** remain and absence cannot be proven under the provider contract, Seyal must abandon the continuation.

Abandon means:

- do not send the provider continuation/session identifier for the next turn;
- rebuild the next request from currently eligible local sources/evidence;
- if required local state is unavailable, stop/reconcile or create a fresh retry under ADR-012 rather than pretending behavioral continuity.

Provider cache warmth or cost benefit never overrides this rule.

## 11. External-provider deletion truthfulness

Seyal distinguishes:

```text
LocalForgotten
ProviderDeleteRequested
ProviderDeleteConfirmed
ProviderDeleteUnsupported
ProviderDeleteUnknown
```

These are evidence states, not guarantees beyond provider capability.

Rules:

1. `LocalForgotten` never implies provider deletion.
2. A provider delete API may be invoked only through the provider's declared capability/security contract.
3. Provider acknowledgement is recorded as evidence of that provider operation, not proof that all downstream backups/training/log retention ceased unless the provider contract explicitly guarantees it.
4. Unsupported/unknown provider deletion is surfaced honestly.
5. Seyal must not keep local forbidden payload merely to retry provider deletion.

## 12. Same-evidence anti-resurrection

After a semantic memory is revoked/forgotten, pre-revocation retained evidence must not automatically recreate the same semantic memory.

### 12.1 Suppression identity

Suppression uses a policy-safe, non-reversible semantic identity that does not retain the deleted secret/plaintext claim.

It binds at least:

```text
scope identity
record kind / semantic category
policy-safe normalized semantic identity
source/evidence lineage identity or fingerprint
revocation generation
```

A plain hash of raw secret text is not automatically safe; the semantic-key design must not enable offline reconstruction or cross-scope correlation.

### 12.2 New evidence

New independent post-revocation evidence may propose a new record only through the normal SPEC-012 creation/acceptance pipeline.

The system must distinguish:

- same old evidence reappearing;
- a restatement derived from the same old evidence;
- genuinely independent new evidence.

Model paraphrase or reformatting does not create independent evidence.

## 13. Concurrent proposal/acceptance/revalidation races

Required ordering:

- if proposal extraction started before revocation and completes after revocation, it is stale and cannot publish an eligible proposal without current revalidation;
- if acceptance started before revocation and the revocation commits first, acceptance fails/retries against the new generation;
- if acceptance commits first and then revocation commits, revocation wins for future eligibility;
- revalidation never resurrects `Revoked` without a new MemoryId/new normal proposal path allowed by policy;
- revocation must be idempotent for duplicate user/policy requests.

## 14. Interaction with ADR-014 Action dispatch

This specification does not create a second Action state machine.

If an Action payload/context depends on revocable material, ADR-014's atomic pre-dispatch transaction must include current privacy/revocation eligibility.

### 14.1 Revocation before `Dispatching`

If revocation commits before the ADR-014 pre-dispatch transaction commits:

- the precondition fails;
- the old authorization cannot be silently refreshed;
- any prior consumable authorization is invalidated as required by ADR-014;
- the Action must be re-prepared/re-authorized if materially changed current payload/context is still desired.

### 14.2 Revocation after durable `Dispatching`

After `Dispatching`, Seyal must not claim the external effect/bytes were prevented merely because a later revocation occurred.

The Action follows ADR-014 reconciliation/effect rules. Local retained payload still follows deletion policy, but missing payload may make safe reconciliation unavailable; that unavailability is reported honestly.

## 15. Model/provider dispatch race matrix

| Race point | Required behavior |
|---|---|
| revoke before bundle build | revoked content is not eligible for selection |
| revoke during bundle build | build cannot publish dispatchable bundle unless it validates current generation at commit/use |
| revoke after bundle build, before send | final use-time check rejects old bundle; rebuild required |
| revoke after transport preparation, before irreversible provider handoff | cancel/prevent send where adapter can still guarantee no handoff; otherwise treat handoff truthfully |
| revoke after irreversible provider handoff | cannot unsend; mark continuation unsafe and enforce local forgetting/provider evidence semantics |
| response arrives from now-abandoned continuation | may be retained only under current policy as external evidence; cannot silently reactivate revoked context or continuation |

## 16. Local deletion completion

Local deletion completion requires all applicable obligations to reach a policy-defined terminal disposition.

At minimum:

- authoritative MemoryRecord/source eligibility is revoked;
- locally retained semantic payload slated for deletion is removed/redacted;
- affected ContextBundle/SelectionTrace payload is removed/redacted or expired according to policy;
- affected RunWorkingSet/compaction payload is removed/redacted or made unavailable;
- prompt/selection/retrieval caches are logically invalidated;
- derived index/embedding entries are logically invalidated and scheduled for bounded physical cleanup;
- provider continuation is fenced/abandoned when necessary;
- minimum tombstone/suppression metadata is policy-safe.

Physical derived-index cleanup may lag while logical eligibility remains denied. User-facing completion claims must distinguish logical ineligibility from completed physical erasure if the product exposes that detail.

## 17. Failure behavior

### Persistence failure before revocation commit

- do not claim forgetting completed;
- retry with bounded backoff or surface degraded state;
- no new affected dispatch may rely on an uncommitted revocation claim.

### Persistence failure after generation commit

- revoked content remains ineligible;
- cleanup may be pending/degraded;
- retry cleanup with bounded backoff;
- do not re-enable dispatch merely because cleanup storage is unavailable.

### Index/cache deletion failure

- query/use-time generation checks keep old entry ineligible;
- physical cleanup retries are bounded;
- resource growth is monitored/capped.

### Provider/network failure

- provider deletion may remain `Unknown`/`Requested` according to evidence;
- provider continuation remains fenced if revocation requires abandonment;
- terminal progress remains independent.

## 18. Security requirements

1. Revocation/tombstone metadata must not retain forbidden plaintext or reversible secret identity.
2. Cross-workspace/worktree/user suppression must not leak semantic existence unless policy explicitly permits it.
3. Logs/traces/errors contain only minimum policy-safe metadata.
4. A model/tool/provider cannot override revocation through self-report.
5. Raw terminal text cannot create/clear revocation authority.
6. Revocation generation tokens are not reusable authorization capabilities.
7. Stale workers cannot publish current derived context after replacement without generation revalidation.

## 19. Resource and performance requirements

Revocation may trigger wide invalidation but must remain bounded.

Required controls:

- bounded invalidation batches;
- cancellable/priority-aware cleanup;
- retry ceilings + backoff;
- disk/RSS accounting for pending cleanup;
- no unbounded full-repository rescans on every revocation unless separately justified/calibrated;
- terminal input/output/rendering never waits on invalidation, physical erasure, provider deletion or index rebuild.

Implementation readiness requires measured budgets under #681.

## 20. Required tests

### 20.1 Lifecycle / ordering

- revoke Accepted MemoryRecord -> immediately ineligible;
- revoke Superseded/Expired historical record -> retained payload minimized according to SPEC-012;
- duplicate revocation request -> idempotent;
- proposal acceptance racing revocation -> restrictive generation wins;
- revalidation racing revocation -> revoked cannot become Accepted again.

### 20.2 Bundle / dispatch

- bundle built at generation N; revoke to N+1 before send -> dispatch denied;
- bundle with mixed dependencies -> only rebuilt current eligible bundle can dispatch;
- stale bundle hash equality does not restore eligibility;
- revoke after bundle build but before Action dispatch transaction -> Action precondition fails.

### 20.3 Working state / continuation

- compaction depends on revoked memory -> compaction unavailable/rebuilt;
- provider continuation may contain revoked content -> continuation abandoned;
- provider continuation absence provable under declared provider contract -> only then permitted if all other checks pass;
- missing retained prerequisite after abandonment -> `ResumeUnavailable`/reconcile, not fabricated reconstruction.

### 20.4 Cache/index/trace

- stale lexical/vector hit cannot select revoked content;
- revoked dependency payload is not retrievable through retained ContextBundle/SelectionTrace;
- embedding/summary/index cleanup failure does not restore eligibility;
- logs/traces do not retain excluded secret-bearing snippets/locators.

### 20.5 Anti-resurrection

- same pre-revocation evidence cannot recreate same forgotten memory;
- paraphrase derived from same old evidence is still suppressed;
- different semantic key/evidence is not accidentally suppressed;
- genuinely independent post-revocation evidence can create a new proposal through normal policy.

### 20.6 Provider truthfulness

- local deletion complete + provider deletion unsupported -> states remain distinct;
- provider delete request timeout -> `ProviderDeleteUnknown/Requested` according to evidence, never `Confirmed`;
- already transmitted content is never described as unsent.

### 20.7 Failure/resource

- persistence unavailable before revocation commit -> no false completion claim;
- persistence unavailable after commit -> dispatch still denied; cleanup degraded;
- repeated cleanup failure bounded;
- queue saturation / large invalidation set does not block PTY/VT/render;
- fuzz malformed dependency/generation records; fail closed/quarantine.

## 21. Acceptance criteria

SPEC-015 is accepted only when independent review establishes that:

- revocation use-time races are deterministic;
- provider continuation cannot bypass forgetting;
- local deletion completion is truthful and testable;
- same-evidence resurrection is prevented without secret-bearing tombstones;
- ADR-014 dispatch ordering is consumed rather than duplicated;
- cross-scope privacy isolation is preserved;
- failure and cleanup behavior are bounded;
- terminal hot-path isolation is explicit;
- exact-head Foundation Quality is green.

## 22. Explicit non-goals

This specification does not define:

- provider-specific retention policy or legal guarantees;
- global cloud data-deletion architecture;
- MemoryRecord base lifecycle/modes/conflicts;
- context ranking/retrieval policy;
- RunWorkingSet compaction algorithm;
- Action lifecycle/idempotency/reconciliation beyond privacy precondition interaction;
- user-facing privacy settings UX;
- concrete persistence/index implementation;
- production implementation.
