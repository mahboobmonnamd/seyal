# M001 distributed development strategy

M001 remains the single accepted vertical milestone in `docs/milestones/MILESTONE-001.md`. GitHub Issues decompose it without changing scope or architecture.

## Parent epic

Create one M001 Epic/parent Issue linked to the GitHub Milestone. Its acceptance criteria are the M001 acceptance gates and demo procedure. The epic itself is not an implementation Issue.

## Dependency frontier

Use the accepted pass order as the outer dependency chain:

```text
Pass 1 repository/build/test foundation
→ Pass 2 VT parser/state
→ Pass 3 PTY + TerminalExecution
→ Pass 4 headless Runtime
→ Pass 5 local attach/projection + security/transport benchmark
→ Pass 6 Metal renderer
→ Pass 7 native input + resize
→ Pass 8 minimal Block + logical anchor
→ Pass 9 detach/reconnect + GUI crash survival
→ Pass 10 milestone conformance/performance/failure validation
```

A later pass is not Ready until its predecessor's required exits are evidenced.

## Granularity

Do not assign an entire multi-week pass to one coding agent if it crosses several independently testable ownership boundaries. Refine the active pass into coherent Feature/Task Issues just before execution.

### Pass 1 recommended Issues

- deterministic toolchain + canonical root task interface;
- minimal Rust workspace/module scaffolding justified by ownership boundaries;
- native macOS build skeleton using permanent AppKit/Metal direction without terminal functionality;
- test/fixture/fuzz/benchmark harness skeletons;
- fast PR CI foundation and architecture/dependency check.

All must remain scaffolding: no production terminal feature implementation.

### Pass 2 recommended Issues

- permanent incremental VT parser framing + parser-state tests/fuzz seam;
- printable UTF-8 + CR/LF/BS/HT state behavior;
- cursor movement/position + erase behavior;
- SGR/color representation;
- primary/minimal alternate screen + cursor save/visibility;
- resize, `LineId` minimum and damage-generation invariants;
- deferred/unknown-sequence continuity diagnostics/tests;
- retained M001 reference/conformance corpus with provenance;
- Pass 2 validation Issue.

Split further only if ownership/test independence is clear. Do not parallelize changes that both rewrite the parser authority/state core.

### Passes 3–10

Refine only when the preceding pass is nearing completion. Preserve the milestone's required exits as the parent Feature acceptance criteria, then create smaller Tasks around real module/test boundaries.

## Parallelism rules

Parallel work is allowed only when:

- dependencies are complete;
- files/modules/state ownership do not overlap materially;
- integration contract is already specified/tested;
- each Issue can merge independently without temporary architecture;
- neither Issue needs the other branch's unmerged assumptions.

If two tasks touch canonical `TerminalState`, PTY lifecycle, projection generation protocol, or renderer/native boundary, default to serial execution unless independence is proven explicitly in Refinement.

## Validation Issues

Each pass gets an independent validation Issue if its exit requires cross-task evidence. Pass 10 is the final milestone-validation Issue and must use `.agents/skills/milestone-validation/SKILL.md`.

## Future milestones

Do not populate GitHub with detailed M002+ implementation tasks now. Create only minimal placeholders where needed to show a hard dependency or explicitly deferred behavior. Detailed Issues are generated from accepted specs when that milestone approaches activation.
