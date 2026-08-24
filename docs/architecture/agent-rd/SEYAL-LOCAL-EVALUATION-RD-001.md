# Seyal Local Evaluation, Outcome and Cost R&D

**Status:** Proposed  
**Issue:** #54  
**Dependency:** #48 / PR #60  
**Scope:** OSS evidence, evaluation and local cost accounting; no implementation.

## Decision

Separate process/run lifecycle from evidence and accepted outcome.

```text
AgentRun terminates
      ↓
RunEvents + ArtifactRefs + CostEvents
      ↓
EvaluationObservations
      ↓ evaluator policy
Evaluation { pass | fail | inconclusive }
      ↓
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
Outcome = accepted | rejected | unresolved | abandoned
```

A WorkItem can be `unresolved` after a clean AgentRun termination. A failed first Attempt followed by an accepted second Attempt remains one WorkItem with two Attempts.

## Cost and effort events

`CostEvent` records facts, not forecasts:

- input/output/cached tokens reported by provider/harness;
- provider-reported monetary charge when available;
- local/remote compute duration/resource class;
- wall elapsed duration;
- human intervention/attention start and resolution times;
- explicit user-entered labor-cost assumptions only for derived ROI views.

Store raw usage units and pricing-source/version separately. Never bake today's provider price into an immutable historical token event.

Derived local metrics:

```text
first_attempt_success_rate
attempts_per_accepted_work_item
ai_cost_per_accepted_work_item
elapsed_time_per_accepted_work_item
human_attention_minutes_per_accepted_work_item
uncached_input_tokens_per_accepted_work_item
cache_tokens_per_accepted_work_item
```

`total engineering cost` is a derived model requiring explicit labor/compute assumptions; it must be labeled modeled rather than provider-billed fact.

## Event ordering

Use idempotent EventIds and per-source/run sequence numbers where available. Do not serialize every agent/evaluator event through one global order.

Late observations are allowed. An Outcome may be revised from `unresolved` to `accepted/rejected` when stronger evidence arrives; the revision is append-only/auditable rather than rewriting prior evidence.

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
- retry after evaluation failure;
- cache-enabled vs cache-disabled comparison;
- same task across two harness/model choices.

## Evaluator trust threats

- agent modifies or disables tests;
- harness forges completion/usage fields;
- evaluator executes repository-provided malicious test hooks;
- CI status belongs to wrong commit;
- stale artifact evaluated after further worktree mutation.

Mitigations: bind observations to exact input/worktree/artifact fingerprints, preserve evaluator provenance, distinguish trusted project tests from newly generated tests, and require policy for executable evaluators.

## Router dependency

#55 may consume only measured/derived fields whose provenance is known. Sparse local history is not evidence for confident routing; the router must surface insufficient evidence rather than manufacture a success probability.

## OSS/commercial boundary

**OSS:** schemas, local evaluator interface, fixture format, local history/metrics/cost visibility and comparisons.  
**Commercial:** organization aggregation, benchmark cohorts, proprietary predictors, ROI dashboards and policy/spend controls. Raw local evidence remains exportable/inspectable.

## Success / kill criteria

Pass when retained fixtures distinguish run completion from accepted outcome, bind evidence to exact inputs, reproduce deterministic evaluator results, and compare two routes without an agent grading itself.

Reject designs where the model/harness is the sole success judge, pricing assumptions overwrite raw usage, or evaluation blocks terminal I/O.

## ADR/spec before implementation

- shared Agent Platform lifecycle/ownership ADR;
- RunEvent/EvaluationObservation/Outcome/CostEvent schema spec;
- evaluation fixture/evaluator trust spec;
- metric-definition document with denominators and missing-data rules.
