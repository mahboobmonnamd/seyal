# Seyal specifications

Specifications define **observable behavior and enforceable contracts** below accepted architecture and above milestones/implementation Issues.

## Active specifications

- [`SPEC-001-M001-VT.md`](SPEC-001-M001-VT.md) — M001 incremental VT parser, canonical terminal state, supported/deferred sequences, line identity, resize and damage behavior.
- [`SPEC-002-M001-PTY.md`](SPEC-002-M001-PTY.md) — M001 local macOS PTY endpoint, nonblocking byte I/O, resize, child lifecycle, detach/terminate and resource behavior.
- [`SPEC-003-M001-RUNTIME.md`](SPEC-003-M001-RUNTIME.md) — M001 headless Runtime ownership, multi-execution readiness/fairness, logical attachment, Workspace ownership association seam, bounded input, child-exit, nonblocking termination and resource-measurement behavior.
- [`SPEC-004-M001-LOCAL-ATTACHMENT-PROJECTION.md`](SPEC-004-M001-LOCAL-ATTACHMENT-PROJECTION.md) — **Accepted for M001 Pass 5:** Candidate-D versioned local binary control/input and snapshot/delta display-state transport, same-user attachment authority, generation/resync behavior, hostile-client/resource constraints and measured production-path acceptance.
- [`SPEC-005-M001-METAL-RENDERER.md`](SPEC-005-M001-METAL-RENDERER.md) — **Proposed for M001 Pass 6:** permanent Metal rendering from committed disposable client display state, damage-driven draw preparation, coarse Rust/native batching, shaping/font fallback, bounded glyph cache/atlas, GPU lifecycle, hidden-surface behavior and deterministic renderer acceptance.

## When a specification is required

Create or update a specification before implementation when work defines or changes a reusable behavioral contract whose correctness cannot safely be inferred from a single Issue. This includes, in particular:

- VT parser/state behavior and supported-sequence semantics;
- Unicode/grapheme/width behavior;
- PTY and child lifecycle;
- headless Runtime registry/readiness/lifecycle behavior;
- attach/detach/reconnect behavior;
- local or remote protocols and projection contracts;
- persistence contracts and failure behavior;
- Block invariants and history anchors;
- input/mode routing contracts;
- renderer/projection contracts;
- public API/ABI behavior;
- security-sensitive authority/authorization behavior.

A specification is not required merely to restate an ordinary local implementation detail already fully constrained by architecture and an implementation-ready Issue.

If implementation needs behavior that is not specified and choosing that behavior could affect callers, state ownership, compatibility, correctness, security or performance, stop and refine the specification before coding.

## Required contents

A behavioral specification should contain, as applicable:

```text
purpose / scope
requirements
invariants
inputs / outputs
state transitions
failure / recovery behavior
security behavior
performance / resource constraints
compatibility / versioning behavior
test cases / fixtures / reference provenance
acceptance criteria
explicit non-goals / deferred behavior
```

Specifications describe **what must be observable**, not an incidental internal implementation unless an implementation constraint is itself architecturally required.

## Authority and change discipline

```text
accepted architecture / ADR
→ specification
→ milestone
→ Ready Issue
→ tests
→ implementation
```

A specification cannot override architecture. An Issue cannot override a specification. If evidence indicates the specification needs an architectural change, run the architecture-change process first.

Keep one canonical specification per contract/domain. Amend it through scoped PRs; do not create `-v2`, `-final`, `-new`, or correction copies merely to avoid editing the owning specification.

## Test-driven use

For core behavior, write or enable a failing test/fixture from the specification before implementation. VT work additionally follows `.agents/skills/vt-tdd/SKILL.md` and records external/reference provenance for supported M001 behavior where required by the canonical milestone.
