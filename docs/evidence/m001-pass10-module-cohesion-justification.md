# M001 Pass 10 — Module cohesion justification (#759)

## Status

Accepted for M001 Pass 10 closure of finding #759 under approach **(2)**:

record explicit Pass 10 accepted justification + deferred decomposition owning Issues outside M001.

This document does **not** claim M001 production modules are “clean” relative to the AGENTS.md handwritten >1000-line review trigger. It records why large decompositions were deferred and what remains owned after Pass 10.

## Authority

- `AGENTS.md` — Module cohesion and design patterns (review trigger / >1000-line justification)
- `docs/engineering/M001-PASS10-CODE-QUALITY-REVIEW.md` §11
- Owning finding Issue: #759 (from Pass 10 / #727)
- Architecture invariants unchanged: one authoritative `TerminalState`; Candidate-D display fanout; no second engine

## Measured surfaces (worktree head at justification)

Handwritten production files above the AGENTS.md justification threshold:

| Path | Approx. lines | Cohesion note |
|------|---------------|---------------|
| `crates/seyal-client/src/local.rs` | ~2465 | Local attach client: discovery, handshake, display apply, input/resize, Block cache |
| `crates/seyal-runtime/src/runtime/local.rs` | ~1739 | Local-IPC attach, display publish/resync, composer/resize control |
| `crates/seyal-runtime/src/runtime.rs` | ~1365 | Runtime composition, lifecycle, poll loop |
| `crates/seyal-client/src/ffi.rs` | ~1273 | Rust↔Swift FFI bridge |
| `macos/Seyal/Sources/SeyalShellView.swift` | ~1723 | Native shell presentation (advisory for M001 size gate) |

Related naming debt (not size): dual `block.rs` / `blocks.rs` — resolved post-M001 by #768 as `activity_block_timeline.rs` / `command_block_timeline.rs` (client `block_cache.rs`).

## Pass 10 decision

Large ownership-boundary decompositions of the files above are **accepted as deferred outside M001** because:

1. Pass 10 is a validation/closure gate, not a structural rewrite pass; late large moves risk reactor/lifecycle/hot-path regressions without changing product authority.
2. AGENTS.md forbids structural refactoring that adds hot-path cost merely to satisfy organization rules; safe splits need calm post-freeze sequencing and adversarial re-review.
3. Single-authority HOLDS — this debt is maintainability, not a second VT/runtime engine.
4. Focused correctness work that *does* belong in Pass 10 (display `published` bookkeeping vs encode failure) is fixed under #759 rather than deferred.

M001 Done for #759 therefore means:

- display publish bookkeeping correctness is fixed and tested; and
- this justification plus deferred owning Issues exist;

not that the oversized modules have already been split.

## Deferred owning Issues (outside M001)

| Issue | Scope |
|-------|--------|
| #765 | Decompose `seyal-client` `local.rs` by ownership boundaries |
| #766 | Decompose `seyal-runtime` `runtime.rs` / `runtime/local.rs` |
| #767 | Decompose `seyal-client` `ffi.rs` and advisory `SeyalShellView.swift` cohesion |
| #768 | Resolve dual `block.rs` / `blocks.rs` naming |

These Issues must not be treated as M001 exit blockers. Prefer landing after M001 freeze (and after #760 ABI/panic policy for FFI moves).

## What #759 fixed in-tree

Runtime display fanout incorrectly advanced execution-scoped `published` generation bookkeeping even when Candidate-D encode failed (`DisplayUnavailable`). That would let later deltas use a base generation no viewer received.

Fix: advance `published` only after successful encode; clear bookkeeping and schedule snapshot recovery on encode failure; update `published` when a resync snapshot encodes successfully.

## Explicit non-claims

- Does not reopen Pass 10 / M001; #727 and #5 are closed with PASS evidence elsewhere.
- Does not authorize speculative module trees or premature cross-milestone abstractions.
- Does not weaken AGENTS.md cohesion rules for future merges: new oversized handwritten files still need justification or decomposition.
