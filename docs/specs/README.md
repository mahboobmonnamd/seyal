# Seyal specifications

Specifications define **observable behavior and enforceable contracts** below accepted architecture and above milestones/implementation Issues.

## Active specifications

- [`SPEC-001-M001-VT.md`](SPEC-001-M001-VT.md) — M001 incremental VT parser, canonical terminal state, supported/deferred sequences, line identity, resize and damage behavior.
- [`SPEC-002-M001-PTY.md`](SPEC-002-M001-PTY.md) — M001 local macOS PTY endpoint, nonblocking byte I/O, resize, child lifecycle, detach/terminate and resource behavior.
- [`SPEC-003-M001-RUNTIME.md`](SPEC-003-M001-RUNTIME.md) — M001 headless Runtime ownership, multi-execution readiness/fairness, logical attachment, Workspace ownership association seam, bounded input, child-exit, nonblocking termination and resource-measurement behavior.
- [`SPEC-004-M001-LOCAL-ATTACHMENT-PROJECTION.md`](SPEC-004-M001-LOCAL-ATTACHMENT-PROJECTION.md) — **Accepted for M001 Pass 5:** Candidate-D versioned local binary control/input and snapshot/delta display-state transport, same-user attachment authority, generation/resync behavior, hostile-client/resource constraints and measured production-path acceptance. Its semantic-key and generation-correlated resize extensions are governed by accepted SPEC-006 / PR #703 and were implemented by Pass 7 PR #707, merged as `4490d89fd32f96fe5ff04393a5470944c592f546`.
- [`SPEC-005-M001-METAL-RENDERER.md`](SPEC-005-M001-METAL-RENDERER.md) — **Accepted for M001 Pass 6 via PR #657:** permanent Metal rendering from committed disposable client display state, damage-driven draw preparation, coarse Rust/native batching, shaping/font fallback, bounded glyph cache/atlas, GPU lifecycle, hidden-surface behavior and deterministic renderer acceptance. Implementation ownership and validation are documented in [`../engineering/M001-PASS6-METAL-RENDERER.md`](../engineering/M001-PASS6-METAL-RENDERER.md).
- [`SPEC-006-M001-NATIVE-INPUT-RESIZE.md`](SPEC-006-M001-NATIVE-INPUT-RESIZE.md) — **Accepted for M001 Pass 7 via PR #703; implemented by PR #707:** AppKit input classification, atomic committed-text vs semantic-key routing, Runtime-owned key encoding, Controller authority, bounded client queuing, capability-gated correlated `ResizeRequest`/`ResizeResult`, canonical-generation `appliedAwaitingProjection` fencing, authoritative resize/retry semantics, bounded composition-only `NSTextInputClient` UTF-16 contract, focus/accessibility seam and latency/resource acceptance.
- [`SPEC-007-M001-BLOCKS.md`](SPEC-007-M001-BLOCKS.md) — **Accepted and implemented for M001 Pass 8:** durable Workspace-owned `BlockId`, exact Workspace/Execution association, immutable `BeforeLine(LineId)` primary-history anchor, monotonic final-drain-driven `Current → Completed`, mandatory completed-record retirement, exact final-display → `BlockState::Completed` → `Lifecycle::Finalized` ordering, bounded read-only local projection, deterministic malformed/conflicting-metadata quarantine/raw-terminal recovery and strict no-hot-path/no-copied-transcript constraints. Final reviewed head `54b3a1748effc7c47c409d1f7cfdcbd547e8d1cc` merged by PR #721 as `d9d21187e8429bbd3dbeb3e1c7cc4d05c1d147e6`.
- [`SPEC-008-M003-COMMAND-BLOCKS.md`](SPEC-008-M003-COMMAND-BLOCKS.md) — **Active implementation specification; ADR-009 accepted 2026-08-28:** trusted command boundaries, one logical command Block per Pane submission, one Pane composer, and same-execution full-screen TUI takeover.
- [`SPEC-009-M001-DETACH-RECONNECT.md`](SPEC-009-M001-DETACH-RECONNECT.md) — **Accepted refinement authority for M001 Pass 9; original PR #718 merged as `465ee476124a6d6dd6f48b0485c834d550c684f9`, with post-merge corrective clarification required before production becomes Ready:** GUI detach/crash survival of the same live Runtime, PTY, child and `ExecutionId`; Runtime-owned singleton discovery/stale-endpoint arbitration; fresh attachment/controller/native-input state on reconnect; authoritative current-state reconstruction; SPEC-006 focus/accessibility/IME reconstruction; Pass 8 Block continuity; bounded Controller-race recovery; and reproducible lifecycle/security/resource/performance acceptance without Runtime-crash PTY-survival claims.

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
