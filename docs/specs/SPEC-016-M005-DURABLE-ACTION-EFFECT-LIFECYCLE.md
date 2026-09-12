# SPEC-016 — M005 durable Action lifecycle, approval consumption and effect reconciliation

- **Status:** Accepted on merge; specification promotion for #871
- **Architecture:** ADR-012, ADR-013, ADR-014
- **Parent refinement:** #838
- **Implementation consumers:** #680, #841, #839, #683
- **Privacy sibling:** #870 / SPEC-015 is reviewed in parallel but is not normative authority until accepted; this specification must merge only after the accepted privacy contract is available and reconciled.

## 1. Purpose

This specification defines observable behavior for every **Seyal-controlled effectful operation** that requires durable identity, authorization, dispatch fencing or effect recovery.

It freezes:

- immutable `ActionId` / `ActionIntent` identity;
- exact authorization and single-use approval consumption;
- the atomic durable transition to `Dispatching`;
- AgentRun-binding and Action-dispatch fencing;
- resource-version enforcement at the actual effect boundary;
- crash/restart recovery;
- executor idempotency/reconciliation capability evidence;
- `EffectUnknown` handling;
- cancellation linearization and compensation;
- typed result/reconciliation provenance;
- bounded failure/resource behavior and terminal isolation.

It does not create a second AgentRun, PTY, resource, approval UI or workflow authority.

## 2. Authority boundaries

```text
WorkItem -> Attempt -> AgentRun       ADR-012 Runtime/domain authority
Attention / human Approval            #680 human-decision authority
Context/privacy eligibility            ADR-013 (+ accepted privacy spec when merged)
ActionId / ActionIntent / effect state ADR-014 Action authority
resource/executor                      owns the actual resource operation
```

Rules:

1. Runtime/domain layer is the sole durable Action transition writer.
2. Resource executors perform effects and return typed evidence; they do not own Action lifecycle.
3. Harnesses, UI, MCP, CLI/SDK and workflows submit typed intents/requests but do not create competing state machines.
4. External CLI-agent effects that bypass Seyal's dispatch boundary are never labeled `SeyalEnforced`.
5. No Action persistence/executor/model work synchronously gates terminal I/O/rendering.

## 3. Action identity and immutable intent

Every operation has a stable `ActionId` and immutable `ActionIntent` containing at least:

```text
ActionId
AgentRunId
AgentRun binding generation at preparation/authorization where relevant
capability
resource identity
resource version / freshness precondition
normalized arguments or argument fingerprint
effect class
policy generation
privacy/revocation generation dependency
request provenance
required authorization class
created_at
intent expiry, when applicable
executor capability identity/version, when selected
```

### 3.1 Material change creates a new Action

After preparation, any material change to capability, target, target version, arguments, effect class, policy assumption or protected payload creates a **new `ActionId` and new immutable `ActionIntent`**.

The old Action is not edited, reused or widened.

A client retry carrying the unchanged existing `ActionId` is a duplicate reference to the same Action, not a request to mutate it.

## 4. Canonical lifecycle

```text
Prepared
   +--> Authorized
   |      +--> Prepared             authorization invalidated before dispatch
   |      +--> Dispatching
   |             +--> Prepared      authoritative known-not-dispatched reconciliation; fresh authorization required
   |             +--> Succeeded
   |             +--> FailedKnown
   |             +--> EffectUnknown
   |             |      +-- reconciliation --> Succeeded
   |             |      +-- reconciliation --> FailedKnown
   |             +--> CancelledAfterDispatch
   |                    +-- reconciliation --> Succeeded
   |                    +-- reconciliation --> FailedKnown
   |                    +-- reconciliation --> EffectUnknown
   +--> CancelledBeforeDispatch

Authorized -> CancelledBeforeDispatch
```

`Dispatching -> Prepared` is allowed **only** when the owning executor contract authoritatively proves `known-not-dispatched` for that exact Action/generation as defined by §10.1. It is not a general retry edge.

### 4.1 State meanings

- `Prepared`: intent durably exists; no consumable dispatch authorization is current. This also includes an Action returned from `Dispatching` only by authoritative known-not-dispatched reconciliation, in which case fresh authorization is mandatory.
- `Authorized`: exact authorization is bound and eligible, but any single-use approval has not yet been consumed for dispatch.
- `Dispatching`: the durable conservative boundary after which missing local success cannot prove no external effect.
- `Succeeded`: authoritative evidence proves successful completion for this Action.
- `FailedKnown`: authoritative evidence proves a known non-success outcome with sufficiently known effect semantics.
- `EffectUnknown`: occurrence/partial occurrence/completion cannot safely be established.
- `CancelledBeforeDispatch`: cancellation linearized before `Dispatching`; no dispatch is permitted through this authority.
- `CancelledAfterDispatch`: cancellation linearized after `Dispatching`; rollback/no-effect is not implied.

Audit history retains prior ambiguity/cancellation facts after reconciliation.

## 5. Authorization and exact approval binding

An authorization/approval that permits dispatch binds at least:

```text
ActionId
AgentRunId
capability
resource identity + required version/fingerprint
normalized argument fingerprint
effect class
policy generation
privacy/revocation generation where payload eligibility matters
expiry
consumption state
```

It is not a bearer token and cannot authorize another Action/resource/version/arguments/AgentRun.

Duplicate UI events, reconnects or stale workers cannot consume it twice.

## 6. Atomic dispatch transaction

Immediately before external effect invocation, one local safety-critical transaction must atomically:

1. verify Action is current, non-terminal and not cancelled;
2. verify **intent expiry has not passed**;
3. verify current AgentRun control/binding generation under ADR-012;
4. verify capability remains allowed;
5. verify current policy generation;
6. verify current privacy/security eligibility under ADR-013 and the accepted privacy contract;
7. verify target resource identity and required freshness/version precondition;
8. verify exact approval/authorization is current, unexpired and unconsumed;
9. acquire a new current Action dispatch generation/ownership fence;
10. consume the exact single-use approval where required;
11. durably transition to `Dispatching`.

No intermediate committed state may expose approval consumed without current dispatch ownership/`Dispatching`, or vice versa.

Failure of any precondition leaves the Action undispatched and cannot silently widen authorization.

## 7. Between transaction commit and executor invocation

The executor invocation is bound to **both**:

```text
exact AgentRun binding generation
exact Action dispatch generation
```

A worker/rebinding event that makes the AgentRun binding stale also invalidates that worker's right to invoke the Action even if it still possesses an Action generation token.

The Seyal-controlled executor boundary rechecks/fences the current binding immediately before effect invocation. Where an executor boundary cannot enforce the required fence, that executor cannot be treated as safely `SeyalEnforced` for operations requiring this guarantee.

A stale worker/dispatcher may submit observational evidence, but cannot cross the effect boundary or commit current Action state.

## 8. Resource-version enforcement at the actual effect boundary

The local transaction's version check alone is insufficient if the resource can change before invocation.

For operations whose authorization depends on a resource version/fingerprint, the executor contract must provide one of:

- compare-and-set / conditional mutation on the bound version;
- an executor-owned lock/fence spanning final version validation and effect;
- another authoritative atomic freshness primitive with equivalent semantics.

If the actual effect boundary cannot enforce the bound version, the operation fails closed before effect where possible. If uncertainty arises after `Dispatching`, recovery follows `EffectUnknown`; Seyal must not pretend the approved version was mutated.

Tests must cover a resource change between local transaction commit and executor invocation.

## 9. Crash before `Dispatching`

### Prepared

Crash/restart may reconsider the same immutable Action. No dispatch occurred through this authority.

### Authorized but atomic transaction did not commit

Recovery must:

```text
invalidate old consumable authorization
transition Authorized -> Prepared
record authorization invalidation reason/provenance
require fresh authorization before any future dispatch
```

It must not leave an externally `Authorized` Action that appears ready to dispatch.

## 10. Crash after durable `Dispatching`

Absence of a local result never proves no effect.

Recovery classifies using authoritative executor evidence:

### 10.1 Known not dispatched

Only an executor contract may prove that invocation/effect did not occur.

When proven:

- invalidate the old dispatch generation;
- transition `Dispatching -> Prepared` with durable `known-not-dispatched` reconciliation evidence;
- require fresh authorization before a future dispatch.

### 10.2 Replay-safe continuation of the same Action

Allowed only under a validated executor idempotency/reconciliation contract.

The Action remains an unresolved dispatched Action while reconciliation establishes a safe continuation. A new dispatch generation may be acquired only through the recovery/reconciliation authority after old generation fencing and all current policy/resource/privacy preconditions are revalidated.

This is continuation/reconciliation of the **same ActionId**, not preparation of changed arguments.

### 10.3 Otherwise

Transition/recover to `EffectUnknown`. No blind retry.

## 11. Executor capability trust

Replay/idempotency/reconciliation capability metadata must be:

- supplied/validated by the owning executor authority, not model narration or an untrusted adapter;
- authenticated according to the executor integration trust model;
- bound to executor identity/version and operation/effect class;
- versioned and current;
- treated as absent when stale, conflicting, unverifiable or outside its validity window.

A UUID generated by Seyal is not proof of external idempotency.

## 12. Idempotency/reconciliation contract

An executor claiming replay-safe semantics specifies at least:

```text
stable operation/idempotency identity
duplicate-request guarantee
validity duration/window
partial-effect semantics
authoritative status/reconciliation query
restart/failover semantics
resource-version/CAS behavior
causal evidence available for reconciliation
```

If any required guarantee is absent/expired/unverifiable, fallback is conservative `EffectUnknown`/manual reconciliation.

## 13. Result evidence

Authoritative result evidence binds:

```text
ActionId
Action dispatch generation
AgentRun binding generation or accepted recovery authority
executor identity/version
operation/result identity
resource/version evidence where applicable
outcome
observed_at / committed_at
```

Terminal text, model narration, display strings and stale adapter events are non-authoritative.

## 14. Causal reconciliation

Post-hoc state inspection may resolve `EffectUnknown` only when the executor contract can causally bind the observed state to this Action, for example through:

- operation/request ID;
- idempotency record;
- version/CAS witness;
- resource transaction ID;
- another executor-defined authoritative causal marker.

Seeing the desired state alone is insufficient because another actor may have produced it.

Without causal correlation, remain `EffectUnknown` even if current state looks correct.

## 15. Reconciliation exits

`EffectUnknown` may transition to:

- `Succeeded` with authoritative causal success evidence;
- `FailedKnown` with authoritative known-failure/no-success evidence.

`CancelledAfterDispatch` may transition to:

- `Succeeded`;
- `FailedKnown`;
- `EffectUnknown`;

according to authoritative effect evidence. Cancellation history remains in audit evidence.

## 16. Cancellation linearization

Cancellation is serialized against the durable `Dispatching` transition.

### 16.1 Cancellation wins before `Dispatching`

If cancellation commits first:

```text
Prepared/Authorized -> CancelledBeforeDispatch
```

The atomic dispatch transaction must then fail and no executor invocation may begin.

### 16.2 Dispatch wins first

If `Dispatching` commits first:

```text
Dispatching -> CancelledAfterDispatch
```

Cancellation may request executor stop if supported, but cannot claim rollback/no-effect.

### 16.3 Completion racing cancellation

A current-generation executor completion/reconciliation result remains admissible even if cancellation was requested after dispatch. Cancellation must not suppress authoritative completion evidence.

### 16.4 No reissue after post-dispatch cancellation

`CancelledAfterDispatch` is reconciliation-only for the original Action. It must not be automatically reissued even if an idempotency key exists.

Any later attempt to perform the operation again is a **new ActionId** with fresh authorization.

## 17. Compensation / undo

Compensation is a new explicit Action with its own immutable intent, policy, authorization, dispatch and evidence.

It is not an implicit rollback state of the original Action.

## 18. Privacy and payload retention

Until SPEC-015 is accepted, ADR-013 is the normative privacy authority. This PR must not merge ahead of the accepted privacy contract; after that merge the final rebase must use its canonical dispatch hook rather than duplicating one.

Current invariant:

- privacy/security eligibility is a precondition of the atomic dispatch transaction;
- revocation before `Dispatching` prevents the old protected payload from dispatching;
- revocation after `Dispatching` cannot be described as unsent/rolled back;
- Action payload retention/redaction follows ADR-013 policy;
- hashes/fingerprints do not reconstruct erased payload.

If required payload is deleted before safe reconciliation, that prerequisite is reported unavailable; evidence is not fabricated.

## 19. External-agent enforcement truthfulness

Only operations crossing this Seyal-controlled Action boundary may be labeled `SeyalEnforced`.

An independent external CLI agent may perform shell/network/tool effects outside this boundary. Seyal may observe/request those according to capabilities, but cannot claim this contract prevented or authorized them.

## 20. Duplicate/replay behavior

Receiving the same `ActionId` again is treated as a duplicate only when the caller's canonical immutable intent identity/digest matches the stored `ActionIntent` exactly.

For an exact duplicate:

- never create a second Action;
- never consume approval twice;
- never dispatch twice merely because the caller retried;
- return current durable Action state or join the current reconciliation path.

Reusing an existing `ActionId` with different capability, target/version, normalized arguments, effect class, policy/privacy assumptions or canonical intent digest is an identity collision/tampering error. It is rejected explicitly and can neither mutate nor dispatch the stored Action. A materially changed request requires a new ActionId under §3.1.

## 21. Persistent failure and convergence

### 21.1 Persistence failure before `Dispatching`

If the atomic safety state cannot be durably committed, do not dispatch.

### 21.2 Result persistence failure after effect

The Action remains unresolved dispatched state and is reconciled conservatively. No blind retry.

### 21.3 Automatic reconciliation budget

Automatic reconciliation/retry has a finite policy-defined attempt and/or deadline budget.

On exhaustion:

```text
Action remains EffectUnknown (or current unresolved post-dispatch state)
automatic rescheduling stops
manual/Attention reconciliation may be surfaced
minimum recovery evidence is retained
unrelated terminal/execution work continues
```

Manual/operator reconciliation does not lower the evidence bar: it may transition to `Succeeded` or `FailedKnown` only with the authoritative causal evidence required by §§13–15. A human acknowledgement that evidence is unavailable may classify/accept the residual operational risk for workflow purposes, but must not fabricate a known effect outcome in Action authority; otherwise the Action remains truthfully unresolved.

The system does not invent a fake terminal lifecycle state merely to stop retry. The durable Action remains truthfully ambiguous until authoritative evidence or explicit operator risk handling outside effect truth is available.

No tight loops, unbounded queues or unbounded disk/RSS growth are allowed.

## 22. Recovery matrix

| Failure point | Required recovery |
|---|---|
| before durable ActionIntent | no durable Action; nothing may be claimed/replayed |
| after Prepared | no dispatch; may reconsider same intent |
| after Authorized, before atomic transaction commit | `Authorized -> Prepared`; authorization invalidated; fresh authorization required |
| after durable Dispatching, before executor invocation | conservative ambiguity unless executor proves known-not-dispatched; if proven, `Dispatching -> Prepared` only through §10.1 |
| during executor call / timeout / channel loss | reconcile; no blind retry |
| effect occurred, result persistence failed | unresolved/`EffectUnknown` unless executor evidence proves outcome |
| resource changed after local check | executor CAS/fence decides; otherwise fail closed or reconcile ambiguity |
| stale AgentRun/Action dispatcher returns | cannot invoke/commit current state |
| durable success/failure committed | never dispatch external operation again |
| cancellation races dispatch | §16 linearization decides before/after state |
| reconciliation budget exhausted | stop automatic retries; remain truthfully unresolved and surface manual path |

## 23. Security requirements

1. Approval replay/widening fails closed.
2. Stale AgentRun binding and stale Action dispatch generations cannot effect/commit.
3. Intent/action expiry is checked before dispatch.
4. Executor capability claims are trusted only under §11.
5. Resource-version assumptions are enforced at actual effect boundary.
6. Reconciliation cannot infer causality from desired state alone.
7. Raw terminal/OSC/model narration cannot fabricate Action/result authority.
8. Persistent local safety-state failure prevents new effects but not unrelated terminal progress.
9. Reused `ActionId` with mismatched immutable intent is rejected and never treated as an idempotent duplicate.

## 24. Resource/performance requirements

Action control/effect work stays outside terminal hot paths.

Required controls:

- bounded Action queues;
- bounded per-Action evidence/history according to retention policy;
- finite retry/reconciliation budgets;
- cancellable executor calls where supported;
- stale generation cleanup;
- CPU/RSS/disk accounting under executor/persistence failure;
- no synchronous gate from Action persistence/executor/network/approval to PTY/VT/render progress.

Concrete budgets are calibrated under #841/#680/#839 consumers before implementation readiness.

## 25. Required conformance tests

### Intent / authorization

- material argument/resource/policy change -> new ActionId, old intent unchanged;
- duplicate same ActionId + identical canonical intent -> no duplicate Action/approval/dispatch;
- same ActionId + mismatched immutable intent/digest -> explicit rejection, stored Action unchanged and undispatched by the mismatched request;
- expired ActionIntent while Authorized -> dispatch denied, authorization invalidated/prepared as policy requires;
- expired approval -> dispatch denied.

### Atomic dispatch

- crash before transaction commit -> old authorization invalidated, `Authorized -> Prepared`;
- crash after `Dispatching` before invocation -> no blind retry;
- executor authoritatively proves known-not-dispatched -> old dispatch fence invalidated, `Dispatching -> Prepared`, fresh authorization required;
- approval consumption and `Dispatching` cannot persist separately.

### Fencing

- AgentRun rebind between transaction and invocation -> old worker cannot invoke;
- stale Action dispatcher result -> cannot commit current state;
- both AgentRun binding generation and Action dispatch generation are validated.

### Resource TOCTOU

- resource changes between local check and invocation -> executor CAS/fence rejects or ambiguity reconciles; newer version is never silently mutated under old approval.

### Idempotency trust

- untrusted adapter claims idempotency -> treated as absent;
- stale/expired capability guarantee -> treated as absent;
- verified guarantee allows only contract-defined same-Action continuation.

### Reconciliation

- effect succeeds but result write fails -> no duplicate effect;
- state looks correct but lacks causal marker -> remains `EffectUnknown`;
- operation ID/CAS witness proves success -> may reconcile `Succeeded`;
- automatic reconciliation budget exhaustion stops rescheduling without fabricating known outcome;
- manual/operator handling without authoritative causal evidence cannot relabel an ambiguous Action `Succeeded` or `FailedKnown`.

### Cancellation

- cancel commits before `Dispatching` -> `CancelledBeforeDispatch`, no invocation;
- `Dispatching` commits before cancel -> `CancelledAfterDispatch`;
- completion races cancel -> authoritative completion remains admissible;
- post-dispatch cancelled Action is never reissued; repeat operation requires new ActionId.

### Privacy

- protected payload revoked before `Dispatching` -> dispatch fails;
- revocation after `Dispatching` -> no unsent/rollback claim.

### External-agent truthfulness

- direct external CLI effect bypassing Seyal Action -> never labeled `SeyalEnforced`.

### Failure/resource

- persistent Action-store failure -> fail closed for new effects;
- executor/reconciliation outage -> bounded resources/retries;
- active/failing Actions do not block PTY/VT/render.

## 26. Acceptance criteria

SPEC-016 is acceptable only when:

- material changes require a new ActionId and ActionId collisions with changed intent fail closed;
- lifecycle and recovery transitions are deterministic, including the narrowly authorized `Dispatching -> Prepared` known-not-dispatched edge;
- intent expiry and authorization expiry are enforced;
- dispatch transaction is atomic locally;
- exact AgentRun + Action generations fence invocation/results;
- resource freshness is enforced at the actual effect boundary;
- idempotency/reconciliation capability evidence is trusted/versioned and executor-owned;
- known-not-dispatched and replay-safe recovery have explicit durable behavior;
- cancellation is linearized and post-dispatch cancellation is reconciliation-only;
- post-hoc reconciliation requires causal evidence;
- automatic recovery has finite convergence behavior and manual handling cannot fabricate effect truth;
- external-agent enforcement claims remain truthful;
- privacy hooks compose with ADR-013 and the accepted SPEC-015 without duplicate authority;
- security/fault/property tests cover the complete race matrix;
- terminal hot-path isolation is absolute;
- Foundation Quality is green on the final exact reviewed head.

## 27. Explicit non-goals

This specification does not define:

- Attention/Approval presentation UX;
- ContextBundle/MemoryRecord base behavior;
- provider-specific prompting/routing;
- executor-specific Git/filesystem/process/cloud implementations;
- workflow DAG scheduling;
- concrete storage/transaction technology;
- production implementation.