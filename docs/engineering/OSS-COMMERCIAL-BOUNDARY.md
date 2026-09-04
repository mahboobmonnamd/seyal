# OSS and commercial repository boundary

## Decision

Use a **public canonical Seyal OSS repository plus a private `seyal-commercial` superproject**.

The public repository is authoritative for terminal/runtime/workspace foundations and all OSS product compositions. The private repository consumes a pinned OSS revision as a Git submodule and adds proprietary composition/services above it.

This is formalized by `docs/architecture/ADR-003-OSS-COMMERCIAL-REPOSITORY-BOUNDARY.md`.

The canonical Seyal OSS repository is public and is the authoritative source repository, not a generated export of a private monorepo.

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

Two product terms are intentionally distinct:

```text
Seyal Agent Sessions
= OSS understanding/management of agent sessions running in Seyal

Seyal AI Agent
= separate first-party Seyal agent product/composition
```

`docs/product/SEYAL-AGENT-SESSIONS.md` is the OSS product contract for the first term.

Public Seyal may own agent-native primitives that are valuable without proprietary services, including:

- execution/task/`AgentRun` identity;
- harness capability and event seams;
- terminal-safe support for external/user-provided agent CLIs;
- agent session detection, status, Attention and local notifications;
- local context/evaluation/routing/workflow primitives when independently useful to OSS consumers;
- provider-neutral model/control interfaces when justified by milestones.

An external agent must remain a valid ordinary terminal/TUI workload even when Seyal has no adapter for it. Seyal Agent Sessions must never require a Seyal AI subscription, hosted inference, account, entitlement or cloud service.

The first-party Seyal AI Agent may reuse the same public `WorkItem` / `Attempt` / `AgentRun` / `Attention` / execution model and appear through the same Agent Sessions UX. That reuse does not move commercial product logic into OSS and does not create a second session authority.

Commercial Seyal may own the first-party paid/service composition above those seams, including managed or bundled model access, proprietary harness/service policy, hosted/cloud agents, adaptive/learned routing, account/usage systems, commercial collaboration, managed enterprise policy and future proprietary Seyal-model IP.

This is a repository/product boundary only; implementation remains milestone-driven.

## CI policy

The public OSS repository owns authoritative GitHub Actions quality gates.

For the private `seyal-commercial` repository, GitHub-hosted Actions are intentionally deferred for now because of private-repository CI cost. This does **not** lower engineering standards: commercial changes must run the canonical local build/test/check/integration commands and record evidence in the PR. Private CI can later use self-hosted runners or paid hosted capacity.

## Legal/contributor boundary

Keeping foundational code in the public canonical repository gives contributors a clear legal and technical target, avoids accidental proprietary dependencies, and allows the private repository to choose its own access controls and release cadence.

Any code moving from private to public must pass provenance/license review. Public APIs used by commercial code must still be coherent public architecture seams rather than entitlement backdoors.

## Software license

Seyal OSS uses **Apache License 2.0 (`Apache-2.0`)** as its single open-source license. The canonical license text is the root `LICENSE` file.

The choice is intentionally permissive and includes Apache-2.0's explicit patent-license terms, which fit a contributor-facing, commercial-friendly systems project. Do not add a second OSS license or custom license clauses without an explicit product/legal decision and corresponding documentation update.

A `NOTICE` file is added only when Seyal has attribution notices that actually require distribution; do not create an empty or ceremonial NOTICE file.