# OSS and commercial repository boundary

## Decision

Use a **public canonical Seyal OSS repository plus a private `seyal-commercial` superproject**.

The public repository is authoritative for terminal/runtime/workspace foundations and all OSS product compositions. The private repository consumes a pinned OSS revision as a Git submodule and adds proprietary composition/services above it.

This is formalized by `docs/architecture/ADR-003-OSS-COMMERCIAL-REPOSITORY-BOUNDARY.md`.

The current OSS repository may remain private during transition, but the target is that it becomes the public canonical repository rather than a generated export of a private monorepo.

## What remains in Seyal OSS

Foundational technology belongs in the public repository unless a later ADR provides a strong contrary reason:

- VT/parser/authoritative terminal state
- Unicode/grapheme/width/history/reflow foundations
- PTY/local execution and Runtime foundations
- rendering foundations
- Block fundamentals
- local workspace foundations
- stable protocol/capability foundations when justified by an active milestone
- headless, lightweight and full OSS compositions
- macOS OSS application foundation
- tests, fixtures, conformance, fuzzing and benchmarks
- public GitHub workflow quality gates

Headless, lightweight and full are **not separate repositories**. They are different compositions of the same authoritative implementation.

## What belongs in `seyal-commercial`

Detailed proprietary composition ownership belongs in the private repository, including when implemented:

- commercial/hosted agent implementations and model services
- smart routing and managed multi-agent orchestration
- Pro, Teams and Enterprise composition
- hosted/cloud execution services
- cross-device commercial services
- collaboration/team workflow services
- identity/RBAC/SSO integrations
- policy/audit/enterprise administration
- private deployment/control-plane tooling
- billing/entitlement infrastructure
- commercial support/SLA operations

The private repository is the authority for those proprietary details; the public repository records only the boundary necessary to protect OSS architecture.

## Dependency rule

```text
seyal-commercial → pinned Seyal OSS APIs/protocols/capabilities
Seyal OSS        ↛ seyal-commercial or proprietary code
```

The OSS repository must remain independently cloneable, buildable, testable and useful.

## No SKU/license branches in OSS

Do not put proprietary product conditions in public production code.

Forbidden examples include:

```text
if enterprise_license { enable_private_behavior(); }
if pro_license { alter_workspace_behavior(); }
if commercial_entitlement { change_terminal_path(); }
```

The prohibition is broader than terminal hot paths: the OSS implementation should not need knowledge of commercial SKUs or proprietary entitlement state at all.

When a real product requirement needs extensibility, expose a coherent public capability that **any OSS user can implement and use**. Do not add hidden enterprise hooks and do not create speculative extension traits for future products.

Licensing/entitlement enforcement belongs in the private commercial composition layer.

Independently, no cloud/licensing/telemetry/agent/persistence service may synchronously block PTY input/output, VT mutation, damage, shaping or rendering.

## Agent split

Public Seyal may own agent-native primitives that are valuable without proprietary services, such as execution/task identity, attention/approval primitives, terminal-safe integration points and support for external/user-provided agents when justified by milestones.

Commercial Seyal may own hosted model access, managed orchestration/routing, commercial agent UX/services, account/usage systems, collaboration and managed policy implementations.

This is a repository/product boundary only; implementation remains milestone-driven.

## CI policy

The public OSS repository owns authoritative GitHub Actions quality gates.

For the private `seyal-commercial` repository, GitHub-hosted Actions are intentionally deferred for now because of private-repository CI cost. This does **not** lower engineering standards: commercial changes must run the canonical local build/test/check/integration commands and record evidence in the PR. Private CI can later use self-hosted runners or paid hosted capacity.

## Legal/contributor boundary

Keeping foundational code in the public canonical repository gives contributors a clear legal and technical target, avoids accidental proprietary dependencies, and allows the private repository to choose its own access controls and release cadence.

Any code moving from private to public must pass provenance/license review. Public APIs used by commercial code must still be coherent public architecture seams rather than entitlement backdoors.

## Software license

No final OSS license is selected by this document. Product-owner approval is required. Evaluate **Apache-2.0** versus **Apache-2.0 OR MIT dual-license** before public launch, including patent grant, ecosystem familiarity, contributor expectations and dependency compatibility. Do not add a LICENSE file until that decision is approved.
