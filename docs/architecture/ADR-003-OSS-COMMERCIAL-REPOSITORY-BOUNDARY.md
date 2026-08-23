# ADR-003 — OSS and commercial repository boundary

**Status:** Accepted

**Date:** 2026-08-23

**Scope:** Repository/dependency boundary between Seyal OSS and proprietary commercial capabilities. This ADR does not select the OSS software license.

## Context

Seyal is both an open-source terminal/workspace foundation and a commercial product family (Pro, Teams, Enterprise). Terminal fundamentals must remain genuinely strong OSS and must never become dependent on licensing, cloud or proprietary services.

The repository model must make that rule difficult to violate accidentally.

## Options considered

### Model A — public OSS canonical repository + separate private commercial repository/repositories

The public repository owns foundational terminal/workspace technology. Private commercial systems consume stable public capabilities.

### Model B — one monorepo containing both OSS and proprietary sections

This simplifies some coordinated changes but creates legal/export complexity and makes it easier for proprietary dependencies or entitlement logic to leak into foundational code.

### Model C — private canonical monorepo with generated/open-source export

Rejected. It would make the public repository derivative rather than authoritative, complicate contributor trust/provenance, and create recurring risk that the actual architecture lives behind private boundaries.

## Decision

Choose **Model A**.

The eventual public Seyal repository is the canonical home of:

- VT/parser/terminal state;
- PTY/local execution/runtime foundations;
- rendering foundations;
- Block and local workspace foundations;
- stable protocol/API foundations where appropriate;
- macOS OSS application foundation;
- tests/conformance/benchmarks for those foundations.

Private commercial repositories/services may own hosted agents/cloud execution services, cross-device services, collaboration/team services, identity/RBAC/policy/audit/admin integrations, private deployment/control plane, billing/entitlements and support operations.

## Dependency invariant

```text
commercial/private → stable OSS capabilities
OSS foundation      ↛ proprietary code
```

No license/cloud/telemetry/policy service may become a synchronous dependency of PTY input/output, VT mutation, damage, shaping or rendering.

Commercial code must not change terminal semantics based on entitlement. If a commercial feature needs additional capability, expose an architecturally coherent OSS capability/protocol seam or place the commercial behavior above the terminal foundation.

## Consequences

- Public contributors work against the real foundational implementation, not an export.
- Legal/provenance boundaries are clearer.
- Commercial services can iterate privately without contaminating terminal fundamentals.
- Cross-repository compatibility requires explicit versioned APIs/protocols and CI in commercial repositories.
- Some coordinated changes require staged releases across repositories; that is preferable to architectural contamination.

## Software license

Deferred to explicit product-owner approval. `docs/engineering/OSS-COMMERCIAL-BOUNDARY.md` records the recommendation and evaluation criteria. This ADR must not be interpreted as selecting a license.

## Revisit only if

Measured development/release friction from Model A materially exceeds its legal/architectural benefits and an alternative can prove equivalent public canonicality, contributor clarity, dependency enforcement and hot-path isolation.
