# Seyal Current Product Backlog

**Purpose:** Provide a complete, current stock of unfinished Seyal product capabilities without treating migrated RILL/terminal issues as implementation authority.

`docs/product/FEATURES.md` remains the canonical product capability/disposition registry. This file is the implementation-tracking index that maps unfinished capabilities to current GitHub owners.

## Core rule

The 511 imported `legacy-rill` / `legacy-terminal` issues are historical evidence. Their old open/closed/completed state and old engine mechanisms do not determine current Seyal implementation status.

Every canonical product capability must have exactly one current disposition:

1. **Open current owner** — wanted but unfinished, including `Foundation exists` and `Deferred / decision required`.
2. **Implemented** — closed only with current Seyal working + tested + demonstrable + benchmarked-where-relevant evidence.
3. **Superseded** — closed legacy shape with a link/mapping to the replacement current capability.
4. **Rejected** — closed by an explicit current product/architecture decision.
5. **Historical-only** — retained only as evidence with a reason.

A legacy issue being closed or marked completed is never sufficient evidence for disposition 2.

## Single GitHub entry point

Current backlog umbrella: [#650](https://github.com/mahboobmonnamd/seyal/issues/650)

## Historical F-* catalog owners

| Canonical area | Current owner | Coverage |
|---|---|---|
| 5.1 Hierarchy, dashboards and navigation | [#640](https://github.com/mahboobmonnamd/seyal/issues/640) | All unfinished F-001–F-029 rows; rejected/superseded rows explicitly excluded |
| 5.2 Persistence and recovery | [#641](https://github.com/mahboobmonnamd/seyal/issues/641) | All unfinished F-030–F-047 rows |
| 5.3 Mouse, input editor and Blocks | [#642](https://github.com/mahboobmonnamd/seyal/issues/642) | All unfinished F-050–F-089 plus F-252/F-253/F-254/F-256 |
| 5.4 Terminal fidelity | [#643](https://github.com/mahboobmonnamd/seyal/issues/643) | All unfinished current Seyal terminal-fidelity rows; legacy libghostty directions excluded |
| 5.5 Attention and notifications | [#644](https://github.com/mahboobmonnamd/seyal/issues/644) | All unfinished F-110–F-124 rows |
| 5.6 Agents | [#645](https://github.com/mahboobmonnamd/seyal/issues/645) | All unfinished current agent product rows; superseded/rejected shapes excluded |
| 5.7 Remote | [#646](https://github.com/mahboobmonnamd/seyal/issues/646) | All unfinished remote rows except rejected tmux-mirror shape |
| 5.8 Extra surfaces | [#647](https://github.com/mahboobmonnamd/seyal/issues/647) | All unfinished cold/additive workspace surfaces |
| 5.9 Appearance/configuration/platform/security | [#648](https://github.com/mahboobmonnamd/seyal/issues/648) | All unfinished current rows; superseded sync/control shapes excluded |
| 5.10 Development / IDE boundary | [#649](https://github.com/mahboobmonnamd/seyal/issues/649) | Current coding/development boundary, linked to detailed coding issues |

The checklist inside each owner epic is the stock for that canonical area. When a capability becomes active implementation work, split/refine it into a dedicated child issue and link that issue back from the checklist instead of reopening the migrated historical ticket.

## Seyal-native capabilities

Post-RILL capabilities are tracked in [#262](https://github.com/mahboobmonnamd/seyal/issues/262), with existing R&D owners reused instead of duplicated. This includes Resource Addressing, context-aware CLI, teammate handoff, worktree awareness, agent presence, provider-neutral SCM/CI, secure SSH multiplexing, Integration CLI, capability-scoped Control API, Block references, Command Library, selective sync, latency, Local Context Engine, routing, workflows/orchestration, DevOps workspace, Changes inspector, and evaluation/budget/explainability.

Detailed coding + agentic development work is tracked in [#197](https://github.com/mahboobmonnamd/seyal/issues/197) and its child issues.

Current agent-platform R&D owners include #48, #51, #52, #53, #54, #55, #56 and #57.

## Implementation promotion rule

A product-backlog checkbox is not itself authorization to code. Significant work still follows the repository sequence:

```text
Current product capability
→ R&D / ADR where required
→ specification
→ milestone / implementation issue
→ red tests / fixtures
→ implementation
→ conformance / security / failure testing
→ benchmark evidence where relevant
→ demonstrated definition of done
→ close current backlog item
```

## Engine migration rule

Historical behavior may remain desirable while the historical mechanism is obsolete. Preserve the product requirement, then rewrite it against current Seyal authority:

```text
Runtime
→ Workspace / Session / Tab / Pane
→ TerminalExecution
→ one PTY/endpoint
→ canonical TerminalState
→ damage/projection
→ Metal / other presentation
```

Blocks, raw terminal and TUI remain projections of the same execution/state. Do not revive legacy libghostty production-engine ownership, duplicate grids, PTY-per-Block designs, hidden input injection, or other superseded RILL/terminal mechanisms.

## Editor boundary clarification

The old `F-240 Native code editor` rejection is under explicit reconsideration through [#209](https://github.com/mahboobmonnamd/seyal/issues/209). Current tracking treats a first-class editor surface as **open R&D**, not as authorization to rebuild VS Code. LSP and debugger/DAP remain separate explicit decisions.

## How to read GitHub

- Use [#650](https://github.com/mahboobmonnamd/seyal/issues/650) as the product-backlog entry point.
- Use the ten `[Current backlog]` section epics for complete F-* coverage.
- Use #262 for Seyal-native capabilities and #197 for coding/agentic features.
- Use migrated `historical-evidence` issues only to recover rationale, tests, bugs and old implementation lessons.
- Never infer current completion from the state of a migrated issue.
