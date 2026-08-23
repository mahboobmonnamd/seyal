# ADR-003 — OSS and commercial repository boundary

**Status:** Accepted

**Date:** 2026-08-23

**Scope:** Repository/dependency boundary between Seyal OSS and proprietary commercial capabilities. The repository boundary is the architectural decision; the selected OSS software license is recorded here for consistency with that boundary.

## Context

Seyal is both an open-source terminal/workspace foundation and a commercial product family (Pro, Teams, Enterprise). Terminal fundamentals must remain genuinely strong OSS and must never become dependent on licensing, cloud or proprietary services.

Seyal also has multiple OSS compositions, including headless, lightweight and full native application forms. Those compositions must share the same authoritative PTY/VT/runtime implementation rather than drift into separate repositories or terminal engines.

The repository model must make these rules difficult to violate accidentally.

## Options considered

### Model A — public canonical OSS repository + private commercial superproject

The public repository owns the complete OSS foundation and OSS product compositions. A private commercial superproject consumes a pinned public Seyal revision as a Git submodule and adds proprietary composition/services above that foundation.

### Model B — one monorepo containing both OSS and proprietary sections

This simplifies some coordinated changes but creates legal/export complexity and makes it easier for proprietary dependencies, SKU checks or entitlement logic to leak into foundational code.

### Model C — private canonical monorepo with generated/open-source export

Rejected. It would make the public repository derivative rather than authoritative, complicate contributor trust/provenance, and create recurring risk that the actual architecture lives behind private boundaries.

### Model D — separate repositories for headless, lightweight and full OSS variants

Rejected. Those are compositions of the same terminal/runtime authority. Splitting them would increase version skew, duplicate integration work and create pressure for divergent terminal semantics.

## Decision

Choose **Model A**.

The public Seyal repository is the canonical home of:

- VT/parser/terminal state;
- Unicode/grapheme/width/history/reflow foundations;
- PTY/local execution/runtime foundations;
- rendering foundations;
- Block and local workspace foundations;
- stable protocol/capability foundations where justified by an active milestone;
- headless, lightweight and full OSS compositions;
- macOS OSS application foundation;
- tests/conformance/fuzzing/benchmarks for those foundations;
- public GitHub workflow quality gates.

The private repository `seyal-commercial` is the commercial superproject. It may contain proprietary agent implementations, Pro/Teams/Enterprise composition, hosted/cloud services, collaboration, identity/RBAC/policy/audit/admin integrations, billing/entitlements, private deployment/control plane and other commercial-only code.

`seyal-commercial` consumes a **pinned** Seyal OSS revision as a Git submodule. Updating that pin is an explicit compatibility change reviewed in the commercial repository.

## Dependency invariant

```text
seyal-commercial/private → pinned Seyal OSS capabilities
Seyal OSS                ↛ proprietary code
```

The public repository must remain independently cloneable, buildable, testable and useful without access to `seyal-commercial`.

The public repository must not contain SKU/license-aware execution branches such as `enterprise_license`, `pro_license` or equivalent commercial entitlement checks. If an extension seam is needed, it must be a coherent public capability that any OSS user can implement and use. Do not create speculative extension traits merely to reserve future commercial hooks; introduce them only when a concrete milestone requires them.

Licensing and entitlement enforcement, when needed, belongs in the private commercial composition layer. VT, PTY, TerminalState, rendering, local execution and OSS workspace foundations must not know that a commercial repository exists.

No license/cloud/telemetry/policy service may become a synchronous dependency of PTY input/output, VT mutation, damage, shaping or rendering.

## Commercial agent boundary

The public repository may own agent-native primitives that are useful without Seyal commercial services: execution/task identity, attention/approval primitives, terminal-safe integration points and support for external/user-provided agents when justified by product milestones.

The private repository may own hosted model access, commercial agent UX/services, smart routing, managed multi-agent orchestration, usage/account systems, team collaboration and managed enterprise policy implementations.

This split is a product/repository boundary, not permission to speculate those APIs before their milestone.

## CI consequence

The public repository owns authoritative GitHub Actions quality gates.

The private commercial repository may temporarily omit GitHub-hosted Actions for cost reasons, but that is an operational choice rather than a quality exemption. Commercial changes must still provide equivalent local build/test/check evidence until private CI or self-hosted runners are introduced.

## Consequences

- Public contributors work against the real foundational implementation, not an export.
- All OSS variants share one terminal/runtime authority.
- Legal/provenance and dependency boundaries are clearer.
- Proprietary code cannot accidentally become required for the OSS terminal.
- Commercial composition can evolve privately while consuming an explicit OSS revision.
- Cross-repository changes require a public change first, followed by a reviewed commercial submodule-pin update.
- Some coordinated changes require staged releases across repositories; that is preferable to architectural contamination.

## Software license

Seyal OSS is licensed under **Apache License 2.0 (`Apache-2.0`)**. The canonical license text lives at the repository root in `LICENSE`.

Apache-2.0 is selected as the single OSS license for the foundation. It keeps the project permissive for individual, commercial and enterprise use while providing an explicit patent-license framework for contributions. Seyal does not use an MIT/Apache dual-license unless a future concrete ecosystem or dependency requirement justifies reopening that choice.

This software-license choice does not permit proprietary entitlement logic to enter OSS production code and does not change the one-way repository dependency invariant above.

## Revisit only if

Measured development/release friction from this model materially exceeds its legal/architectural benefits and an alternative can prove equivalent public canonicality, contributor clarity, one-way dependency enforcement, single terminal/runtime authority and hot-path isolation; or a concrete ecosystem/legal requirement justifies reconsidering the OSS license while preserving the same public-foundation guarantees.
