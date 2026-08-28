# AI-SDLC reference-consumer evidence

## Purpose

Seyal is the first real consumer of AI-SDLC's generic project-context and core development-loop skills. This document records the integration boundary and the concrete Seyal scenarios used to prove that the generic procedures can be consumed without importing Seyal/terminal knowledge into AI-SDLC.

This is **reference-consumer integration evidence**, not the model/evaluation benchmark required before AI-SDLC is promoted on skills.sh.

## Pinned framework

Seyal consumes AI-SDLC `main` at exact commit:

```text
105e0cedc392a4468308d9bbfd6c273ad44924fe
```

This is the merge commit for AI-SDLC PR #9, which adds the generic `pr-review` merge-readiness orchestrator while retaining focused `code-review` and criterion-level `verification` as separate reusable capabilities.

The pin is developer tooling only and is materialized by `make bootstrap-agents` under ignored `.sdlc/framework/`. Product build/test/runtime paths do not depend on it.

## Capability mapping

| Seyal entrypoint | Generic authority | Seyal-only delta |
| --- | --- | --- |
| `project-context` | AI-SDLC `project-context` | Seyal context/index + authority chain |
| `development-readiness` | AI-SDLC `development-readiness` | `ISSUE-PROTOCOL.md` Ready checklist and architecture triggers |
| `issue-refinement` | AI-SDLC `work-item-design` | GitHub Issue fields, milestone frontier, terminal evidence classification |
| `implement-issue` | AI-SDLC `implementation` | one Issue/worktree/branch/PR, `make check`, docs/domain gates |
| `code-review` | AI-SDLC `code-review` | focused terminal architecture/hot-path implementation review |
| `verification` | AI-SDLC `verification` | Seyal Issue criterion/evidence and repository/domain gates |
| `pr-review` | AI-SDLC `pr-review` | final terminal architecture, exact-head, performance/security/evidence and Issue-state merge gates |
| `milestone-validation` | AI-SDLC `verification` | aggregate milestone criteria and milestone sequencing |

`code-review` and `pr-review` are intentionally separate. `code-review` answers the narrow implementation/diff question and returns an implementation-review handoff such as `APPROVE_FOR_VERIFICATION`; `pr-review` answers whether the exact merge candidate is genuinely ready to merge and orchestrates or consumes code review, verification and risk-based specialist evidence.

Seyal intentionally does not add separate local `work-item-design` or `implementation` aliases because the existing Seyal facades remain the project discovery surface for those activities.

## Reference scenario 1 — work-item design and readiness

The reference-consumer pattern begins with a bounded developer-tooling work item:

- accepted outcome is explicit;
- runtime/product behavior is an explicit non-goal unless the owning Issue says otherwise;
- dependencies and ownership boundaries are known;
- acceptance is measurable through deterministic pin/discovery/integration evidence.

`issue-refinement` delegates generic work-item structure to AI-SDLC `work-item-design`; `development-readiness` then adds the Seyal Ready checklist. Missing architecture or an unexpected proposal to change terminal/runtime behavior routes out of the tooling work item instead of being absorbed silently.

## Reference scenario 2 — implementation handoff

A framework-integration Issue is constrained to developer-workflow surfaces such as:

```text
scripts/bootstrap-dev.sh
scripts/test-tooling.sh
.agents/skills/* adapters/facades
.claude/skills/* adapters
.sdlc/context + derived index pin metadata
docs/engineering/*
```

The generic AI-SDLC implementation procedure supplies scope/evidence/stop semantics. `implement-issue` adds Seyal's repository workflow and required checks.

A correct implementation handoff is `IMPLEMENTED_FOR_REVIEW`, not `VERIFIED` or `READY_TO_MERGE`. Any discovered need to change terminal/runtime code, architecture ownership, or product behavior is a scope/authority conflict and must stop the tooling Issue.

## Reference scenario 3 — focused code review

`code-review` delegates focused implementation-review discipline to AI-SDLC `code-review` and adds Seyal-specific blocking checks. It must reject at least these classes of defect when applicable:

- production Rust/Swift/PTY/VT/renderer/runtime changes hidden in a tooling Issue;
- a copied generic SDLC procedure that creates a second authoritative workflow;
- a bootstrap pin that does not match `.sdlc` metadata/index;
- missing generic skill files at the pinned revision;
- incorrect facade mapping;
- weakened tooling checks that allow drift silently.

A clean focused review is only `APPROVE_FOR_VERIFICATION`; it is not a final merge verdict.

## Reference scenario 4 — verification

`verification` applies the AI-SDLC criterion/evidence contract to the owning Issue. Repository evidence for this integration includes:

- `scripts/test-tooling.sh` checks the exact full-SHA framework pin;
- all required generic AI-SDLC skills are declared and verified by bootstrap;
- the project-context tool and derived-index validation remain required;
- `issue-refinement`, `implement-issue`, `code-review`, `pr-review`, and `milestone-validation` point to their intended generic AI-SDLC authorities;
- direct `project-context`, `development-readiness`, `code-review`, and `verification` adapters exist for `.agents` and Claude discovery;
- `.sdlc/context/_meta.yaml` and `.sdlc/graph/context-index.json` match the bootstrap pin;
- generic procedures are not duplicated in Seyal;
- normal product build/test/runtime commands remain independent of AI-SDLC.

A passing verification proves the mapped acceptance criteria represented by that evidence; it still does not by itself issue the final exact-head merge-readiness verdict.

## Reference scenario 5 — final PR review

`pr-review` delegates the generic merge-readiness orchestration to AI-SDLC `pr-review` and adds Seyal's terminal/domain gates.

The facade must ensure or consume:

1. focused `code-review` for non-trivial production changes;
2. criterion-level `verification` for every mandatory acceptance gate;
3. only the specialist reviews required by the Issue/spec/risk profile;
4. exact-head CI/check freshness;
5. truthful benchmark/resource boundaries and exact-revision evidence where required;
6. accurate PR/Issue/docs claims and post-merge Issue state;
7. a final re-resolution of the PR head before `READY_TO_MERGE`.

Green CI is evidence, not proof of unrepresented behavior. Hosted-environment limitations must be classified explicitly and cannot become a pass unless project-authorized alternate evidence proves the criterion on the applicable exact revision.

## What this evidence does not prove

This integration does not by itself prove that AI-SDLC is ready for skills.sh. AI-SDLC still requires its declared evaluation thresholds, model/agent runs, security/privacy review, licensing/versioning decision, and clean skills CLI installation smoke before broad promotion.

Reusable defects found while operating these facades must be fixed in `ai-sdlc` first. Seyal-specific terminal/product rules remain local.
