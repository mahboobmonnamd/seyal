# AI-SDLC reference-consumer evidence

## Purpose

Seyal is the first real consumer of AI-SDLC's generic project-context and core development-loop skills. This document records the integration boundary and the concrete Seyal scenarios used to prove that the generic procedures can be consumed without importing Seyal/terminal knowledge into AI-SDLC.

This is **reference-consumer integration evidence**, not the model/evaluation benchmark required before AI-SDLC is promoted on skills.sh.

## Pinned framework

Seyal consumes AI-SDLC `main` at exact commit:

```text
30fdbadfb16080094b42edf4e008e4ed4bef6b44
```

The pin is developer tooling only and is materialized by `make bootstrap-agents` under ignored `.sdlc/framework/`. Product build/test/runtime paths do not depend on it.

## Capability mapping

| Seyal entrypoint | Generic authority | Seyal-only delta |
| --- | --- | --- |
| `project-context` | AI-SDLC `project-context` | Seyal context/index + authority chain |
| `development-readiness` | AI-SDLC `development-readiness` | `ISSUE-PROTOCOL.md` Ready checklist and architecture triggers |
| `issue-refinement` | AI-SDLC `work-item-design` | GitHub Issue fields, milestone frontier, terminal evidence classification |
| `implement-issue` | AI-SDLC `implementation` | one Issue/worktree/branch/PR, `make check`, docs/domain gates |
| `pr-review` | AI-SDLC `code-review` | terminal ownership/hot-path and specialist evidence checks |
| `verification` | AI-SDLC `verification` | Seyal Issue evidence and repository/domain gates |
| `milestone-validation` | AI-SDLC `verification` | aggregate milestone criteria and milestone sequencing |

Seyal intentionally does not add separate local `work-item-design`, `implementation`, or `code-review` aliases because the existing Seyal facades are the project discovery surface for those activities.

## Reference scenario 1 — work-item design and readiness

Issue #58 is the reference work item:

- accepted outcome: consume the merged AI-SDLC core-loop skills;
- explicit non-goal: no terminal/runtime production behavior changes;
- dependency: AI-SDLC project-context/core-loop and Seyal project-context integration already merged;
- ownership boundary: developer tooling, agent skills/adapters, `.sdlc` metadata, and engineering documentation;
- measurable acceptance: generic procedure is not duplicated, adapters stay thin, pin is deterministic, tooling checks stay green.

`issue-refinement` delegates the generic work-item structure to AI-SDLC `work-item-design`; `development-readiness` then adds the Seyal Ready checklist. Missing architecture or a proposal to change Rust/Swift/runtime behavior would route out of this work item instead of being absorbed silently.

## Reference scenario 2 — implementation handoff

The Issue #58 implementation is constrained to developer-workflow surfaces:

```text
scripts/bootstrap-dev.sh
scripts/test-tooling.sh
.agents/skills/* adapters/facades
.claude/skills/* adapters
.sdlc/context + derived index pin metadata
docs/engineering/*
```

The generic AI-SDLC implementation procedure supplies scope/evidence/stop semantics. `implement-issue` adds Seyal's repository workflow and required checks.

A correct implementation handoff is `IMPLEMENTED_FOR_REVIEW`, not `VERIFIED`. Any discovered need to change terminal/runtime code, architecture ownership, or product behavior is a scope/authority conflict and must stop this Issue.

## Reference scenario 3 — code review

`pr-review` delegates generic review discipline to AI-SDLC `code-review` and adds Seyal-specific blocking checks. For this integration, review must reject at least these classes of defect:

- a production Rust/Swift/PTY/VT/renderer/runtime change hidden in the tooling Issue;
- a copied generic SDLC procedure that creates a second authoritative workflow;
- a bootstrap pin that does not match `.sdlc` metadata/index;
- missing generic skill files at the pinned revision;
- duplicate discovery aliases that compete with established Seyal facades;
- weakened tooling checks that allow drift silently.

A clean review is only `APPROVE_FOR_VERIFICATION`.

## Reference scenario 4 — verification

`verification` applies the AI-SDLC criterion/evidence contract to Issue #58. The repository supplies deterministic evidence for the integration contract:

- `scripts/test-tooling.sh` checks the exact full-SHA framework pin;
- all six generic AI-SDLC skills are declared and verified by bootstrap;
- the project-context tool and derived index validation remain required;
- `issue-refinement`, `implement-issue`, `pr-review`, and `milestone-validation` point to their generic AI-SDLC authorities;
- direct `development-readiness` and `verification` adapters exist for `.agents` and Claude discovery;
- `.sdlc/context/_meta.yaml` and `.sdlc/graph/context-index.json` must match the bootstrap pin;
- the generic project-context implementation is not duplicated in Seyal;
- normal product build/test/runtime commands remain independent of AI-SDLC.

The final Issue verdict still depends on normal Seyal PR CI/review evidence. This document does not convert structural integration into a self-approved completion claim.

## What this evidence does not prove

This integration does not by itself prove that AI-SDLC is ready for skills.sh. AI-SDLC still requires its declared evaluation thresholds, model/agent runs, security/privacy review, licensing/versioning decision, and clean skills CLI installation smoke before broad promotion.

Reusable defects found while operating these facades must be fixed in `ai-sdlc` first. Seyal-specific terminal/product rules remain local.
