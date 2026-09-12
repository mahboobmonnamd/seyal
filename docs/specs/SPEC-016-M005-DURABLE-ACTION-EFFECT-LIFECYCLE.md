# SPEC-016 — M005 durable Action lifecycle, approval consumption and effect reconciliation

- **Status:** Proposed under #871; accepted on merge only after independent review
- **Architecture:** ADR-012, ADR-013, ADR-014
- **Parent refinement:** #838
- **Implementation consumers:** #680, #841, #839, #683

## 1. Purpose

This specification defines the observable behavior required for Seyal-controlled effectful Actions.

It specifies:

- immutable `ActionIntent` identity;
- exact authorization/approval binding;
- single-use approval consumption;
- atomic transition to `Dispatching`;
- dispatch fencing/generations;
- crash/restart recovery;
- conservative `EffectUnknown` semantics;
- executor-specific idempotency/reconciliation;
- cancellation versus rollback;
- typed result provenance;
- retention/privacy interaction;
- bounded failure/resource behavior and terminal isolation.

This specification does not create another AgentRun, approval UX, executor, workflow engine or terminal authority.

## 2. Authority boundaries

```text
AgentRun authority       -> ADR-012
Context/privacy authority -> ADR-013 / SPEC-015
Action/effect authority  -> ADR-014 / this spec
Attention/Approval UX    -> #680
Executor/resource truth  -> owning typed resource authority
```

Rules:

1. One durable Action authority owns state transitions for Seyal-controlled effects.
2. Harness/UI/CLI/SDK/MCP/workflow components submit typed intents/observations; they do not own Action state.
3. Resource authorities execute their own operations and return typed evidence.
4. `TerminalExecution` remains sole PTY/process/TerminalState authority.
5. External-agent effects that bypass Seyal control are never relabeled `SeyalEnforced`.

## 3. Action identity and immutable intent

Every Seyal-controlled effect uses one stable `ActionId`.

Before authorization, Seyal durably records an immutable normalized `ActionIntent` containing at least:

```text
ActionId
AgentRunId
capability
resource identity
resource version / freshness precondition
normalized arguments or stable argument fingerprint
effect class
policy generation
privacy/revocation generation when relevant
request provenance
required authorization class
created_at
expiry when applicable
executor capability reference
```

Once `Prepared`, materially changing any of the following invalidates the intent as dispatch authority:

- capability;
- target resource;
- required resource version/fingerprint;
- normalized arguments;
- effect class;
- applicable policy generation;
- relevant privacy/security payload eligibility.

A materially changed operation is re-prepared and re-authorized; the old Action is not silently edited or widened.

## 4. Canonical lifecycle

Observable states:

```text
Prepared
  -> Authorized
      -> Dispatching
          -> Succeeded
          -> FailedKnown
          -> EffectUnknown
              -> Succeeded       # authoritative reconciliation
              -> FailedKnown     # authoritative reconciliation
          -> CancelledAfterDispatch
              -> Succeeded       # authoritative reconciliation
              -> FailedKnown     # authoritative reconciliation
              -> EffectUnknown   # authoritative reconciliation remains ambiguous
  -> CancelledBeforeDispatch

Authorized
  -> CancelledBeforeDispatch
```

No other externally meaningful transition is silently inferred.

### 4.1 State meanings

`Prepared`
: immutable ActionIntent exists; no consumable dispatch authorization is current.

`Authorized`
: exact authorization is durably bound to this intent; any single-use approval has not yet been consumed for dispatch.

`Dispatching`
: the atomic local dispatch boundary committed. Absence of a later local result cannot prove no external effect occurred.

`Succeeded`
: authoritative typed evidence establishes intended success.

`FailedKnown`
: authoritative typed evidence establishes a known non-success effect outcome.

`EffectUnknown`
: Seyal cannot safely establish whether the effect occurred, partially occurred, remains in progress, or did not occur.

`CancelledBeforeDispatch`
: cancellation prevented the dispatch boundary from being crossed.

`CancelledAfterDispatch`
: cancellation was requested after dispatch began; no rollback/no-effect claim is implied.

## 5. Exact authorization binding

An authorization consumed by an Action binds at least:

```text
ActionId
AgentRunId
capability
resource identity
resource version/fingerprint precondition
argument fingerprint
policy generation
privacy/revocation generation when relevant
authorization class
expiry
consumption state
```

Rules:

1. Authorization cannot be widened to another Action/resource/version/arguments/AgentRun.
2. Duplicate UI/reconnect events cannot consume the same approval twice.
3. Expired authorization is ineligible.
4. Authorization does not survive a material intent/precondition change.
5. Authorization is not a general bearer capability.
6. AgentRun reconnect alone does not invalidate exact authorization if all bound assumptions remain current; stale run bindings still cannot control it.

## 6. Atomic pre-dispatch transaction

Immediately before dispatch, one atomic local safety transaction performs all of:

```text
revalidate Action is current/non-terminal
revalidate current AgentRun control generation
revalidate target resource identity/version/freshness
revalidate capability/policy
revalidate privacy/revocation eligibility
revalidate exact approval presence/expiry/unconsumed state
acquire current Action dispatch generation/ownership
consume single-use approval when required
durably transition Action -> Dispatching
```

All succeed together or none commits.

Forbidden intermediate committed states include:

- approval consumed but Action not durably `Dispatching`;
- `Dispatching` recorded without current dispatch ownership/generation;
- dispatch ownership acquired while approval/preconditions remain unvalidated.

After the transaction commits, the executor invocation uses the stable `ActionId` and current dispatch-generation/fencing material.

## 7. Crash before `Dispatching`

If recovery finds:

```text
state == Authorized
and no committed Dispatching transaction
```

then Seyal knows no dispatch occurred **through this Action authority**.

However, prior consumable authorization is invalidated for recovery and **fresh authorization is required before the Action can dispatch**.

Rationale: recovery must not replay a pre-crash authorization across changed policy/resource/privacy/runtime assumptions.

No external effect is inferred merely from `Authorized`.

## 8. Crash after `Dispatching`

Once `Dispatching` is durable, recovery is conservative.

For non-replayable/insufficiently observable actions:

```text
missing result
worker death
transport timeout
provider timeout
process crash
UI disconnect
```

must not cause blind retry.

Recovery obtains authoritative executor/resource evidence and resolves to:

```text
Succeeded
FailedKnown
known-not-dispatched (only if executor contract can prove it)
replay-safe continuation (only under explicit idempotency contract)
EffectUnknown
```

If none can be proved, result is `EffectUnknown`.

## 9. Dispatch generation and fencing

Each dispatch attempt/control owner has a monotonically changing Action dispatch generation or equivalent fencing credential.

Only current dispatch generation may:

- invoke/reissue under an allowed executor contract;
- submit authoritative result evidence;
- complete/reconcile the current dispatch;
- initiate executor-specific status query/retry where allowed.

A stale generation may submit late observational evidence when policy permits, but cannot mutate current Action state directly.

`AgentRun` binding generation and Action dispatch generation are separate:

```text
AgentRun generation -> who may control the run
Action generation   -> who may control this effect dispatch
```

Neither substitutes for the other.

## 10. Executor capability contract

An executor that permits replay/reissue/reconciliation must declare versioned capabilities.

At minimum:

```text
ExecutorCapabilityVersion
operation identity mechanism
idempotency key semantics
duplicate-request guarantee
guarantee validity/expiry
partial-effect semantics
status/reconciliation query semantics
restart/failover semantics
resource-version/CAS behavior
cancellation semantics
```

A generated UUID alone is not proof of idempotency.

If any required guarantee is absent, expired, unsupported or unverifiable, Seyal uses conservative `EffectUnknown` semantics.

## 11. Replay classes

### 11.1 Non-replayable

No trusted duplicate-suppression/status proof exists.

After ambiguous dispatch, no automatic replay.

### 11.2 Idempotent-by-resource semantics

Executor/resource guarantees that repeating the exact same operation against the exact allowed resource version cannot widen/duplicate effect.

Replay is allowed only while those preconditions remain current.

### 11.3 Idempotency-key protected

External system contract guarantees duplicate suppression for the exact Action/operation identity within a declared window.

Replay is allowed only inside the verified guarantee window and current policy/resource constraints.

### 11.4 Reconciliation-only

Executor supports authoritative status lookup but not safe replay.

Recovery queries status and resolves or remains `EffectUnknown`.

## 12. Resource/version freshness

The dispatch transaction revalidates target identity/version/fingerprint.

If resource state changed since authorization:

- old authorization is not widened;
- old Action does not silently target the new version;
- consumer either cancels/re-prepares a materially new Action or obtains a new exact authorization under current state.

This applies to files, Git refs, processes, deployments, remote resources and other typed authorities.

## 13. Privacy/security use-time check

ADR-013/SPEC-015 privacy eligibility is part of the same atomic pre-dispatch transaction.

If payload/context eligibility is revoked before the transaction commits:

- dispatch fails closed;
- any authorization tied to old payload assumptions is no longer consumable;
- Action is re-prepared/re-authorized if the materially changed operation is still wanted.

If privacy revocation occurs after durable `Dispatching`, this spec does not claim the external effect was prevented or unsent. ADR-014 reconciliation semantics apply.

## 14. Result evidence

An authoritative result binds at least:

```text
ActionId
AgentRunId
dispatch generation
executor identity/capability version
resource identity
operation identity/result identity
observed outcome
result timestamp
provenance/authentication evidence
```

A result cannot be accepted solely from:

- terminal text;
- model narration/self-report;
- display strings;
- stale adapter events;
- the resource coincidentally matching desired state.

Post-hoc state inspection may be reconciliation evidence, but it must be labeled as reconciliation, not fabricated original execution evidence.

## 15. Reconciliation

`EffectUnknown` and `CancelledAfterDispatch` may transition only when authoritative reconciliation evidence is obtained.

### 15.1 EffectUnknown exits

```text
EffectUnknown -> Succeeded
EffectUnknown -> FailedKnown
```

If evidence remains insufficient, it stays `EffectUnknown`.

### 15.2 CancelledAfterDispatch exits

```text
CancelledAfterDispatch -> Succeeded
CancelledAfterDispatch -> FailedKnown
CancelledAfterDispatch -> EffectUnknown
```

Audit history preserves that cancellation was previously requested and/or ambiguity previously existed.

Reconciliation does not rewrite history to pretend no ambiguity/cancellation occurred.

## 16. Cancellation semantics

### Before dispatch

Cancellation produces `CancelledBeforeDispatch` and prevents executor invocation.

### After dispatch

Cancellation means no further work should be initiated and, if supported, an executor cancel request may be sent.

It does **not** mean:

- effect never happened;
- partial effect was undone;
- resource returned to original state.

If compensation/undo is desired, it is a new Action with its own ActionId, authorization, dispatch and result evidence.

## 17. Recovery matrix

| Failure point | Required recovery |
|---|---|
| before durable ActionIntent | no durable Action; nothing may be claimed/replayed |
| after Prepared, before Authorized | same Action may be reconsidered; no dispatch through authority |
| after Authorized, before atomic Dispatching commit | old authorization invalidated; fresh authorization required |
| after durable Dispatching, before executor invocation | conservative ambiguity unless authoritative executor proves not dispatched |
| during executor call / timeout | reconcile; no blind retry |
| effect occurred, result persistence failed | unresolved dispatch -> EffectUnknown unless authoritative evidence resolves |
| after EffectUnknown | reconcile to Succeeded/FailedKnown or remain unknown |
| after CancelledAfterDispatch | reconcile to Succeeded/FailedKnown/EffectUnknown |
| after terminal result committed | never redispatch merely during state replay/recovery |
| stale dispatcher returns | reject state mutation unless processed through current reconciliation contract |
| durable persistence unavailable | fail closed for new dispatches; bounded retry/degraded state; terminal progresses |

## 18. Duplicate/replayed request behavior

For the same `ActionId`:

- duplicate prepare request returns existing identity/state or an explicit conflict; it does not create another effect;
- duplicate approval event is idempotent and cannot consume twice;
- duplicate dispatch request from stale/same generation cannot invoke twice unless executor contract explicitly allows safe reissue and Action state permits it;
- duplicate result with same authenticated operation identity is idempotent;
- conflicting duplicate result requires reconciliation/quarantine, not last-writer-wins.

## 19. External agent boundary

If an external CLI agent invokes effects through its own process/harness outside Seyal dispatch control:

```text
Observed / UpstreamRequestable != SeyalEnforced
```

Seyal may display typed observations/advisory evidence but cannot claim:

- approval prevented the effect;
- Action fencing controlled it;
- cancellation stopped it;
- Action lifecycle is execution truth for that bypassed effect.

A future adapter may become `SeyalEnforced` only when Seyal actually owns the enforceable dispatch boundary.

## 20. Payload retention and deletion

Action metadata/payload follows ADR-013/SPEC-015 sensitivity and retention policy.

Rules:

1. Keep only data needed for identity, safety, reconciliation, audit and user-visible evidence under policy.
2. Redaction/deletion cannot turn a replay-unsafe action into replay-safe.
3. If required reconciliation payload is deleted, report reconciliation prerequisite unavailable; do not reconstruct from hashes.
4. Action fingerprints prove equality/dependency only; they do not restore erased data.
5. Logs/errors never retain sensitive arguments unnecessarily.

## 21. Persistent failure behavior

### Action persistence unavailable before dispatch

No new Action may cross the dispatch boundary if safety-critical durable state cannot be committed.

### Result persistence unavailable after effect

Treat result as unresolved; do not blindly retry effect. Preserve/recover through executor reconciliation where possible.

### Reconciliation service unavailable

Remain `EffectUnknown`; bounded backoff/retry. Human Attention may be surfaced according to #680, but terminal progress remains independent.

### Repeated failure

Retry/backoff is bounded and observable. No tight loops, unbounded queues or unbounded disk/RSS growth.

## 22. Security requirements

1. Authorization binding uses canonical normalized arguments/resource identity, not UI strings.
2. Stale AgentRun/dispatch generations fail closed.
3. Unauthenticated/untrusted result evidence cannot settle Action outcome.
4. Unknown/newer durable Action schema is quarantined/ineligible for dispatch.
5. Logs/traces are sensitivity-aware.
6. Capability policy cannot be widened by model/provider narration.
7. Raw terminal escape/text input cannot authorize/complete an Action.
8. Control-plane replay must be idempotent or explicitly rejected.

## 23. Resource/performance requirements

Action safety work is outside terminal hot paths.

Required implementation controls:

- bounded pending Action queues;
- bounded result/reconciliation records;
- bounded retry/backoff;
- cancellation propagation without blocking terminal reactor;
- resource cleanup after terminal Action states;
- failure injection under persistence/executor outage;
- CPU/RSS/disk/latency measurements under active Actions;
- zero synchronous dependency from PTY/VT/render onto Action storage/executor/network/licensing/cloud.

Concrete budgets are calibrated under #841/#680/#839 implementation readiness.

## 24. Required tests

### 24.1 Intent/authorization

- exact ActionIntent round-trip/versioning;
- materially changed args/resource/version/policy require new preparation/authorization;
- duplicate approval cannot consume twice;
- expired approval cannot dispatch;
- approval for AgentRun A cannot authorize AgentRun B;
- argument canonicalization ambiguity fails closed.

### 24.2 Atomic dispatch boundary

- injected crash before transaction commit -> remains Authorized/recovers requiring fresh authorization;
- injected crash after transaction commit before executor call -> Dispatching ambiguity, not blind retry;
- no state exists with consumed approval but no committed dispatch generation/state;
- no state exists with committed Dispatching but missing dispatch ownership generation.

### 24.3 Fencing

- stale AgentRun worker cannot dispatch;
- stale Action dispatch generation cannot invoke/complete;
- stale result cannot overwrite reconciled current state;
- replacement dispatcher handles only explicitly authorized reconciliation/reissue.

### 24.4 Effect ambiguity

- executor timeout -> EffectUnknown unless authoritative failure/no-effect proof;
- effect succeeds but result persistence fails -> recovery reconciles rather than redispatches;
- missing local result does not imply no effect;
- model/terminal success string cannot settle state.

### 24.5 Idempotency

- no declared contract -> no automatic replay;
- expired idempotency guarantee -> no replay;
- valid duplicate-suppression contract permits safe same-Action reissue only within stated rules;
- partial-effect-capable executor does not report ordinary failure without reconciliation semantics.

### 24.6 Cancellation/reconciliation

- cancel before dispatch -> executor never invoked;
- cancel after dispatch + later success -> Succeeded with cancellation history retained;
- cancel after dispatch + proven known failure -> FailedKnown;
- cancel after dispatch + uncertain status -> EffectUnknown;
- compensation creates new ActionId and approval.

### 24.7 Privacy

- revocation before dispatch transaction -> dispatch denied;
- revocation after Dispatching -> no false unsent/rollback claim;
- deleted reconciliation payload -> explicit unavailable, no hash reconstruction.

### 24.8 Fault/resource

- persistence outage before dispatch fails closed;
- persistence outage after external success does not duplicate effect;
- repeated executor/status failure bounded;
- queue saturation bounded;
- fuzz malformed Action/result/authorization records;
- active/failing Action plane does not stall PTY/VT/render.

## 25. Conformance fixtures

At least these executor fixtures are required before implementation can claim generality:

1. non-replayable executor fixture;
2. authoritative status-query-only fixture;
3. idempotency-key fixture with finite guarantee window;
4. resource-version/CAS fixture;
5. cancellation-with-ambiguous-effect fixture;
6. stale dispatcher/result fixture;
7. persistence crash-injection fixture.

Fixtures may be synthetic/local; they test contracts, not vendor products.

## 26. Compatibility/versioning

- Durable Action schema is versioned.
- Executor capability contract is versioned independently.
- Unknown required fields/capability versions fail closed for dispatch.
- Reader upgrades may interpret older terminal states, but cannot invent missing authorization/effect guarantees.
- Migration must preserve prior `EffectUnknown`, cancellation and reconciliation audit history.

## 27. Acceptance criteria

SPEC-016 is accepted only when independent review establishes that:

- lifecycle and crash boundaries are complete/deterministic;
- approval cannot be replayed/widened;
- atomic dispatch ordering has no TOCTOU hole;
- stale workers/dispatchers/results are fenced;
- missing result cannot cause blind retry;
- idempotency is executor-specific and evidence-backed;
- cancellation does not imply rollback;
- reconciliation exits are authoritative and preserve history;
- context/privacy preconditions consume ADR-013/SPEC-015 rather than duplicate authority;
- external-agent bypass effects are not mislabeled enforced;
- failures/resources are bounded and terminal isolation remains absolute;
- exact-head Foundation Quality is green.

## 28. Explicit non-goals

This specification does not define:

- Attention/approval presentation UX;
- global workflow DAG scheduling;
- generic undo semantics;
- executor implementation details for Git/filesystem/process/cloud APIs;
- model/harness prompting/routing policy;
- provider-specific action schemas;
- concrete database/transaction technology;
- production implementation.
