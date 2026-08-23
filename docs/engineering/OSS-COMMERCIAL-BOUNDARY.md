# OSS and commercial repository boundary

## Decision

Use **Model A: a public Seyal OSS repository as the canonical home of terminal/workspace foundations, plus separate private commercial repositories/services that consume stable OSS capabilities**.

This is formalized by `docs/architecture/ADR-003-OSS-COMMERCIAL-REPOSITORY-BOUNDARY.md`.

The current repository may remain private while foundations are being prepared, but the architectural target is that this repository becomes the public canonical OSS foundation rather than a stripped export of a proprietary monorepo.

## OSS foundation

Foundational technology belongs here unless a later ADR provides a strong contrary reason:

- VT/parser/terminal state
- Unicode/width/history/reflow foundations
- PTY/local execution and runtime foundations
- rendering foundations
- Block fundamentals
- local workspace foundations
- stable protocol/API foundations where appropriate
- macOS OSS application foundation
- tests, fixtures, conformance and benchmarks for these foundations

The OSS terminal must remain genuinely excellent. Commercial tiers must not degrade local terminal fundamentals.

## Commercial/private layers

Private repositories/services may provide:

- hosted/cloud execution services
- hosted/model agent services
- cross-device continuation services
- collaboration and team workflow services
- identity/RBAC/SSO integrations
- policy/audit/enterprise administration
- private deployment/control-plane tooling
- billing/entitlement infrastructure
- commercial support/SLA operations

These consume versioned OSS capabilities rather than injecting license-aware branches into terminal fundamentals.

## Dependency rule

```text
private/commercial → stable OSS APIs/protocols/capabilities
OSS terminal core  ↛ private/commercial code
```

Forbidden examples:

```text
if enterprise_license { change_vt_behavior(); }
if cloud_available { allow_pty_progress(); }
```

No licensing/cloud/telemetry call belongs in PTY/VT/render hot paths.

## Legal/contributor boundary

Keeping foundational code in the public canonical repository gives contributors a clear legal and technical target, avoids accidental proprietary dependencies, and allows commercial repositories to choose their own access controls and release cadence.

Any code moving from private to public must pass provenance/license review. Public APIs exposed specifically for commercial consumption should still be useful, coherent architecture seams rather than hidden entitlement backdoors.

## Software license

No final OSS license is selected by this document. Product-owner approval is required. Recommendation: evaluate **Apache-2.0** versus **Apache-2.0 OR MIT dual-license** before public launch, with explicit consideration of patent grant, ecosystem familiarity, contributor expectations, and compatibility with dependencies. Do not add a LICENSE file until that decision is approved.
