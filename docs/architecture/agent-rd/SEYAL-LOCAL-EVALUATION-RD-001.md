# Seyal Local Evaluation, Outcome and Cost R&D

**Status:** Proposed  
**Issue:** #54  
**Dependency:** #48 / PR #60  
**Scope:** OSS evidence, evaluation and local cost accounting; no implementation.

## Decision

Separate process/run lifecycle, evaluator verdict, per-Attempt disposition and durable WorkItem outcome.

```text
AgentRun terminates
      ↓
RunEvents + ArtifactRefs + CostEvents
      ↓
EvaluationObservations
      ↓ evaluator policy
Evaluation { pass | fail | inconclusive }
      ↓
AttemptDisposition
      ↓ WorkItem acceptance policy across one or more Attempts
WorkItem Outcome
```

A harness saying “done” is evidence of termination/progress, not authoritative proof that the engineering task succeeded.

## Evidence model

`EvaluationObservation` contains:

- observation ID and schema version;
- evaluator/source identity + version;
- WorkItem/Attempt/AgentRun/Artifact references;
- observed input fingerprint;
- result type/value;
- timestamp and source-local ordering metadata;
- trust class;
- optional structured diagnostics;
- reproducibility metadata where applicable.

Recommended default trust ordering:

```text
local deterministic process/test evidence
> authenticated CI/source-system evidence
> explicit human acceptance/rejection
> structured harness/provider reports
> model self-report
```

The evaluator policy decides what combination is sufficient. It may return `inconclusive` instead of fabricating certainty.

## Evaluators

OSS evaluator interface should support:

- command/test execution with exact exit/result capture;
- build/lint/static-analysis results;
- git/diff/worktree state checks;
- existing CI/status checks when locally accessible;
- review/human decision observation;
- artifact schema/content checks;
- harness structured completion metadata;
- pluggable project-specific evaluators.

Evaluators never mutate terminal state and do not run merely because untrusted repository text requested them.

## Outcome model

Do not overload one boolean `success`.

```text
RunTermination = completed | cancelled | interrupted | crashed
Evaluation = pass | fail | inconclusive
AttemptDisposition = accepted_candidate | rejected | inconclusive | cancelled | interrupted | superseded
Outcome = accepted | rejected | unresolved | abandoned     # WorkItem only
```

`AttemptDisposition` is immutable/auditable per Attempt except through an explicit append-only correction/revision event. A retry creates a new Attempt and never rewrites the earlier Attempt's evidence/disposition. Parallel candidate Attempts may each finish differently.

Only the durable WorkItem owns `Outcome`. An `accepted_candidate` Attempt identifies work eligible for WorkItem acceptance policy; it does not by itself overwrite the WorkItem Outcome. The final Outcome records decisive Attempt/Evaluation references when applicable.

A WorkItem can be `unresolved` after a clean AgentRun termination. A failed first Attempt followed by an accepted second Attempt remains one WorkItem with two Attempts and two retained dispositions.

Outcome accounting must retain all terminal states in cohort denominators. Failed, rejected, unresolved and abandoned WorkItems do not disappear merely because cost-per-accepted-work calculations use accepted outcomes as one denominator.

## Cost and effort events

`CostEvent` records facts, not forecasts:

- input/output/cached tokens reported by provider/harness;
- provider-reported monetary charge when available;
- local/remote compute duration/resource class;
- wall elapsed duration;
- attention request opened/resolved timestamps;
- explicit user interaction intervals where Seyal can measure them without invasive surveillance;
- explicit user-entered labor-cost assumptions only for derived ROI views.

Store raw usage units and pricing-source/version separately. Never bake today's provider price into an immutable historical token event.

### Human attention is not approval wait time

Do **not** infer human labor from the time between `AttentionItem` creation and resolution. An engineer may be doing unrelated work for most of that interval.

Keep at least these concepts separate:

```text
attention_wait_duration
human_active_interaction_duration
elapsed_work_duration
```

- `attention_wait_duration` is factual queue/blocking latency from request to resolution.
- `human_active_interaction_duration` is recorded only from defensible local interaction evidence or explicit user/customer input and must identify its measurement method.
- `elapsed_work_duration` is end-to-end wall time and is not labor time.

When reliable active-interaction measurement is unavailable, report it as missing/unknown instead of substituting attention wait time. Commercial ROI models may use a customer-supplied approximation, but it must remain a modeled assumption rather than a raw event.

Derived local metrics:

```text
first_attempt_acceptance_rate
attempts_per_accepted_work_item
ai_cost_per_accepted_work_item
ai_cost_per_all_work_items
elapsed_time_per_accepted_work_item
attention_wait_minutes_per_work_item
human_active_interaction_minutes_per_accepted_work_item?   # only when measured/model provenance is explicit
rejected_unresolved_abandoned_rate
uncached_input_tokens_per_accepted_work_item
cache_tokens_per_accepted_work_item
```

All costs from failed/rejected/abandoned Attempts remain attributed to their WorkItem and included when computing cohort total cost. A route that cheaply succeeds on a few tasks while abandoning expensive failures must not appear artificially efficient.

`total engineering cost` is a derived model requiring explicit labor/compute assumptions; it must be labeled modeled rather than provider-billed fact.

## Event ordering

Use idempotent EventIds and per-source/run sequence numbers where available. Do not serialize every agent/evaluator event through one global order.

Late observations are allowed. Stronger evidence may append a revised Evaluation/AttemptDisposition and can revise a WorkItem Outcome from `unresolved` to `accepted/rejected` according to policy. Revisions are append-only/auditable rather than rewriting prior evidence or pretending an earlier Attempt never existed.

## Repeatable evaluation fixture

A fixture should define:

```text
fixture version
repository snapshot/base revision
task/WorkItem statement
allowed execution/tool policy
initial worktree state
required evaluator set
acceptance predicates
budget/context constraints
nondeterminism notes
```

Do not require one golden textual patch when multiple correct implementations exist. Prefer behavioral acceptance through tests/contracts plus explicit forbidden changes.

Initial corpus must include:

- straightforward code/test fix;
- ambiguous task requiring clarification/attention;
- agent claims success but tests fail;
- tests pass but forbidden file changes exist;
- cancelled run with partial artifact;
- retry after evaluation failure preserving the first AttemptDisposition;
- parallel candidates where one is superseded and one becomes the accepted WorkItem result;
- cache-enabled vs cache-disabled comparison;
- same task across two harness/model choices;
- expensive failed/abandoned WorkItems to verify cohort denominator/cost accounting;
- long approval wait with near-zero active human interaction to verify wait time is not counted as labor.

## Evaluator trust threats

- agent modifies or disables tests;
- harness forges completion/usage fields;
- evaluator executes repository-provided malicious test hooks;
- CI status belongs to wrong commit;
- stale artifact evaluated after further worktree mutation.

Mitigations: bind observations to exact input/worktree/artifact fingerprints, preserve evaluator provenance, distinguish trusted project tests from newly generated tests, and require policy for executable evaluators.

## Router dependency

#55 may consume only measured/derived fields whose provenance is known. Sparse local history is not evidence for confident routing; the router must surface insufficient evidence rather than manufacture a success probability.

Any routing objective that uses human-time savings must distinguish measured active interaction from attention waiting and modeled labor assumptions.

## OSS/commercial boundary

**OSS:** schemas, local evaluator interface, fixture format, local history/metrics/cost visibility and comparisons.  
**Commercial:** organization aggregation, benchmark cohorts, proprietary predictors, ROI dashboards and policy/spend controls. Raw local evidence remains exportable/inspectable.

## Success / kill criteria

Pass when retained fixtures distinguish run completion from Evaluation, per-Attempt disposition and final WorkItem Outcome; bind evidence to exact inputs; reproduce deterministic evaluator results; compare two routes without an agent grading itself; preserve failed/abandoned cost in cohort accounting; and do not mislabel approval wait time as human labor.

Reject designs where the model/harness is the sole success judge, Attempt disposition is conflated with durable WorkItem acceptance, pricing assumptions overwrite raw usage, failed tasks disappear from economics, attention wait is silently converted into labor cost, or evaluation blocks terminal I/O.

## ADR/spec before implementation

- shared Agent Platform lifecycle/ownership ADR;
- RunEvent/EvaluationObservation/AttemptDisposition/Outcome/CostEvent schema spec;
- evaluation fixture/evaluator trust spec;
- metric-definition document with denominators, failed/abandoned accounting, attention-vs-active-interaction semantics and missing-data rules.
