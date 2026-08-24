# Seyal Explainable Local Routing R&D

**Status:** Proposed  
**Issue:** #55  
**Dependencies:** #48, #54  
**Scope:** OSS deterministic routing/fallback; no implementation.

## Decision

Start with a deterministic, explainable decision pipeline. Do not use an LLM to choose the LLM/harness.

```text
task requirements
 + user/project route constraints
 + policy/security/residency
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
Region/data-residency target when applicable
Context policy/budget
Permission/network-egress policy
Fallback chain
```

`RoutingDecision` records considered candidates, exclusion reasons, input evidence fingerprints, score components and the chosen route.

## RouteConstraintSet and partial pins

Users/projects/tasks must be able to constrain only the dimensions they care about without selecting the whole tuple.

Examples:

```text
provider = anthropic
model = auto
harness = auto
execution = local

harness = codex
provider = auto
model = auto
execution = auto

region = eu
execution = remote
provider = auto
model = auto

model = exact-model-id
harness = auto
```

A versioned `RouteConstraintSet` may pin/allow/deny dimensions such as:

```text
harness / harness mode
provider
model
execution target
region / data-residency class
network-egress class
context policy/budget range
permission policy
cost/concurrency bounds
```

Unspecified dimensions remain `auto`; they are not silently interpreted as a preference for a current default.

Constraint composition is deterministic. A more local/task-specific constraint can narrow an allowed set but cannot widen a higher-authority security/organization/user prohibition. If constraints are unsatisfiable, routing fails with an inspectable conflict; it does not silently discard a pin.

A **hard pin** remains hard during fallback unless the user/project explicitly supplies an allowed relaxation/fallback set for that dimension. For example, `execution=local` cannot fall back to cloud after a local harness failure merely because a remote candidate scores well.

## Stage 1 — hard filters

Reject candidates that violate any non-negotiable requirement:

- required harness capability unavailable;
- model/context-window constraint incompatible;
- provider/model disallowed by user/project policy;
- partial/full route pin incompatible;
- local/remote execution target unavailable;
- sensitivity policy forbids required data path;
- **data-residency/region policy forbids the candidate even when content is not otherwise classified sensitive**;
- required network-egress class cannot be satisfied;
- budget/spend hard cap exceeded;
- harness/provider unavailable or rate-limited with no allowed wait.

Data residency is a first-class policy dimension, not an inference from `sensitive` classification. A repository/team may require all processing to remain on-device or within an allowed region for contractual/operational reasons even when the material is not secret.

Hard policy is never converted into a score that a cheaper/faster candidate can override.

## Stage 2 — precedence

Recommended precedence:

1. non-overridable security/authorization/residency/organization policy;
2. explicit user hard constraints/pins;
3. explicit project route constraints;
4. required capability constraints;
5. explicit task route constraints/preferences;
6. deterministic default route policy;
7. local historical evidence when sufficient.

A lower layer may narrow an allowed set but cannot expand an upper layer's denial. The exact authority chain belongs in the route-policy behavior spec and must be visible in the `RoutingDecision` explanation.

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

Fallback is typed, budgeted and constraint-preserving. Example failure classes:

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

Fallback candidate generation re-applies the complete effective `RouteConstraintSet` and hard policy snapshot. A pinned/required dimension may change only when the policy itself included an explicit relaxation chain, which must be recorded in the RoutingDecision.

Retries, fallbacks and deliberate parallel candidates are separately visible in outcome/cost accounting.

## Historical local evidence

Use only #54 evidence bound to comparable task/route conditions. Start with transparent aggregates (for example accepted/attempted counts and cost distributions), not opaque learned weights.

Evidence should be suppressed or heavily qualified when sample size/task comparability is inadequate. The UI/explanation can say `insufficient local evidence`.

## Example explanations

```text
Selected: Codex/AppServer + model X
Why:
- requires structured approval events: supported
- execution=local hard constraint: preserved
- model Y excluded: context limit below required estimate
- route X has lower measured median direct cost than route Z on comparable fixtures
Fallback:
- rate limit → route Z, still local
- evaluation failure → stronger model route, if retry budget remains
```

```text
No route selected
Why:
- project requires region=eu
- user pins provider=P
- provider P has no eligible EU execution/provider endpoint
Action:
- report unsatisfiable constraints; do not silently use another region/provider
```

## Rejected approaches

### LLM router first

It adds cost/latency, is hard to reproduce, can be prompt-injected by task content and makes route explanations weaker. A learned/LLM router must later beat this baseline under #26 commercial research.

### Cheapest-model router

It minimizes the first attempt, not successful-task cost. Cheap failures can be more expensive than one stronger successful attempt.

### Hidden automatic fallback

It masks costs/retries and makes debugging impossible. Every fallback creates observable routing/attempt state.

### Silent pin relaxation

Changing a pinned provider/model/execution target/region because the preferred route is unavailable makes policy non-reproducible and can create data-residency or cost surprises. Relaxation must be explicitly authorized and recorded.

### Global winner model

Task needs and harness capabilities differ. Rankings must be scoped to task requirements and current capabilities, not one universal benchmark winner.

## Evaluation

Use #54 fixtures plus routing-specific cases:

- capability-required task;
- cheap route fails then escalation succeeds;
- route becomes unavailable/rate-limited;
- user forbids a provider;
- partial provider pin with model auto-selection;
- local-execution hard pin survives fallback;
- region/data-residency policy eliminates otherwise valid candidates;
- conflicting user/project/task constraints produce inspectable no-route result;
- explicit relaxation chain permits a dimension change and records it;
- sensitive context forbids remote route;
- sparse/no local history;
- misleading historical result from unrelated task category;
- fallback budget exhausted.

Compare manual/default fixed route vs deterministic router on accepted outcome rate, attempts, direct cost, elapsed time and human intervention. Report all dimensions; do not claim a saving percentage before measurement.

## OSS/commercial boundary

**OSS:** routing interface, partial/full route constraints, deterministic filters/precedence/scoring, local history use, user/project rules, residency/egress constraints, fallback/escalation, explanations and decision export.  
**Commercial:** organization policy composition, learned success/cost models, organization priors, fleet/load-aware optimization and managed cross-user route intelligence.

## Success / kill criteria

Pass if every choice/exclusion/fallback can be reproduced from the same input snapshot, policy/partial pins/residency cannot be overridden by scoring or hidden fallback, and retained fixtures demonstrate correct capability/policy handling.

A learned commercial router should be rejected if, on a pre-registered representative evaluation set, it does not show a statistically credible improvement in the primary accepted-outcome/cost objective over this deterministic baseline after accounting for its own inference/operational cost.

## ADR/spec before implementation

- RoutingDecision and fallback behavior spec.
- Versioned user/project/task `RouteConstraintSet` format with authority/merge rules.
- Explicit residency/egress policy semantics.
- Evaluation comparison protocol defining comparable task cohorts and missing-evidence behavior.
