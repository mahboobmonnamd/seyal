# Seyal Explainable Local Routing R&D

**Status:** Proposed  
**Issue:** #55  
**Dependencies:** #48, #54  
**Scope:** OSS deterministic routing/fallback; no implementation.

## Decision

Start with a deterministic, explainable decision pipeline. Do not use an LLM to choose the LLM/harness.

```text
task requirements
 + user rules
 + policy/security
 + discovered capabilities
 + availability/limits
 + local measured outcomes/costs
        ↓
hard filtering
        ↓
precedence rules
        ↓
transparent scoring
        ↓
chosen Route + ordered fallbacks + explanation
```

## Route

A route is an explicit tuple, not merely a model name:

```text
HarnessAdapter
Harness mode/capabilities
ProviderRef
ModelRef
Execution target (local/remote when available)
Context policy/budget
Permission policy
Fallback chain
```

`RoutingDecision` records considered candidates, exclusion reasons, input evidence fingerprints, score components and the chosen route.

## Stage 1 — hard filters

Reject candidates that violate any non-negotiable requirement:

- required harness capability unavailable;
- model/context-window constraint incompatible;
- provider/model disallowed by user policy;
- local/remote execution target unavailable;
- sensitivity policy forbids required data path;
- budget/spend hard cap exceeded;
- harness/provider unavailable or rate-limited with no allowed wait;
- task explicitly pins a route.

Hard policy is never converted into a score that a cheaper/faster candidate can override.

## Stage 2 — precedence

Recommended precedence:

1. explicit user route pin;
2. explicit user/project policy rules;
3. required capability/security constraints;
4. explicit task preference;
5. deterministic default route policy;
6. local historical evidence when sufficient.

## Stage 3 — scoring

Do not pretend sparse history gives a precise probability of success. A baseline score should expose normalized components such as:

```text
capability_fit
local_success_evidence
expected_direct_ai_cost
expected_latency
context_fit
known_retry/escalation burden
risk penalty
```

Weights are user/project configurable and versioned. Missing evidence is represented as unknown with conservative defaults, not zero cost or invented confidence.

The long-term objective is lower **expected total cost to an acceptable outcome**, not lowest first-call price. The OSS baseline can approximate this from measured local outcomes without proprietary ML.

## Fallback

Fallback is typed and budgeted. Example failure classes:

```text
unavailable
rate_limited
capability_mismatch
context_overflow
policy_denied
execution_failure
evaluation_rejected
human_escalation_required
```

Each class has an allowed next action. A capability mismatch should normally choose a compatible route; an evaluation rejection may create a new Attempt with a stronger route; a policy denial must not silently fall back to a route that violates the same policy.

Retries, fallbacks and deliberate parallel candidates are separately visible in outcome/cost accounting.

## Historical local evidence

Use only #54 evidence bound to comparable task/route conditions. Start with transparent aggregates (for example accepted/attempted counts and cost distributions), not opaque learned weights.

Evidence should be suppressed or heavily qualified when sample size/task comparability is inadequate. The UI/explanation can say `insufficient local evidence`.

## Example explanation

```text
Selected: Codex/AppServer + model X
Why:
- requires structured approval events: supported
- local-only execution required: supported
- model Y excluded: context limit below required estimate
- route X has lower measured median direct cost than route Z on comparable fixtures
Fallback:
- rate limit → route Z
- evaluation failure → stronger model route, if retry budget remains
```

## Rejected approaches

### LLM router first

It adds cost/latency, is hard to reproduce, can be prompt-injected by task content and makes route explanations weaker. A learned/LLM router must later beat this baseline under #26 commercial research.

### Cheapest-model router

It minimizes the first attempt, not successful-task cost. Cheap failures can be more expensive than one stronger successful attempt.

### Hidden automatic fallback

It masks costs/retries and makes debugging impossible. Every fallback creates observable routing/attempt state.

### Global winner model

Task needs and harness capabilities differ. Rankings must be scoped to task requirements and current capabilities, not one universal benchmark winner.

## Evaluation

Use #54 fixtures plus routing-specific cases:

- capability-required task;
- cheap route fails then escalation succeeds;
- route becomes unavailable/rate-limited;
- user forbids a provider;
- sensitive context forbids remote route;
- sparse/no local history;
- misleading historical result from unrelated task category;
- fallback budget exhausted.

Compare manual/default fixed route vs deterministic router on accepted outcome rate, attempts, direct cost, elapsed time and human intervention. Report all dimensions; do not claim a saving percentage before measurement.

## OSS/commercial boundary

**OSS:** routing interface, deterministic filters/precedence/scoring, local history use, user rules, fallback/escalation, explanations and decision export.  
**Commercial:** organization policy composition, learned success/cost models, organization priors, fleet/load-aware optimization and managed cross-user route intelligence.

## Success / kill criteria

Pass if every choice/exclusion/fallback can be reproduced from the same input snapshot, policy cannot be overridden by scoring, and retained fixtures demonstrate correct capability/policy handling.

A learned commercial router should be rejected if, on a pre-registered representative evaluation set, it does not show a statistically credible improvement in the primary accepted-outcome/cost objective over this deterministic baseline after accounting for its own inference/operational cost.

## ADR/spec before implementation

- RoutingDecision and fallback behavior spec.
- User/project route-policy format.
- Evaluation comparison protocol defining comparable task cohorts and missing-evidence behavior.
