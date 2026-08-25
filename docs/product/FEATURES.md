# Seyal Product Feature Inventory

**Status:** Canonical product feature inventory  
**Purpose:** Track accepted product capabilities and important product-level requirements without turning architecture documents into a feature backlog.

This document is the canonical inventory of accepted Seyal product features. Architecture, ADRs, specs and milestone documents remain authoritative for implementation design and behavior. A feature appearing here does not bypass normal R&D, ADR, specification, security, performance or milestone gates.

Seyal OSS must remain independently excellent. `seyal` must not depend on commercial code; commercial products consume the OSS repository through stable seams.

## Status vocabulary

- **Accepted direction** — product capability is accepted, but implementation details or milestone may still require R&D/specification.
- **Foundation exists** — supporting architecture already exists, but the user-facing capability may not yet be implemented.
- **Implemented** — production capability exists and has passed its milestone definition of done.

## Interaction, addressing and handoff

### Seyal Resource Addressing

**Status:** Accepted direction

Seyal resources such as Workspace, Execution, Block, Worktree/Attempt, AgentRun and Artifact should have stable addressable identities that can be consumed by multiple UX surfaces.

User-facing entry points may include:

- Copy Seyal Link from a Block, execution, worktree, artifact or agent run;
- Share action;
- Command Palette action to copy a link to the current context;
- `seyal link` from a terminal;
- navigation from another application or teammate through a Seyal resource reference.

Resource addresses are references, never credentials. Opening a shared/enterprise resource must still pass the receiving user's authorization and policy checks.

### Context-aware Seyal CLI

**Status:** Accepted direction

Commands executed inside Seyal should automatically resolve the current context where safe, including Workspace, Execution, repository, worktree, remote target and relevant work/agent identity.

Examples include `seyal split`, `seyal link` or other commands acting on the current context without requiring the user to copy opaque IDs manually.

Context inference must remain explicit and inspectable when an action could have destructive or security-sensitive effects.

### Exact teammate handoff

**Status:** Accepted direction; collaboration/commercial capability consumes OSS resource identities

A user should be able to share an exact Seyal resource so an authorized teammate can land on the relevant Workspace, Execution, Block, artifact, diff or work item rather than reproducing navigation manually.

Potential actions for a live execution include observe, request control, fork investigation, or open an independent shell in the same authorized context. Sharing never grants permissions implicitly.

## Worktrees and agent isolation

### Agent Worktree Awareness

**Status:** Accepted direction; foundation exists in local workflow/isolation architecture

When an agent creates or uses a Git worktree, Seyal should make that worktree visible as first-class work context instead of hiding it behind an opaque agent process.

The worktree association should be connected to the relevant WorkItem / Attempt / AgentRun and may expose:

- repository;
- path;
- branch;
- base revision;
- changed files;
- PR/MR association;
- lifecycle state;
- open shell;
- reveal;
- diff;
- archive/cleanup actions where safe.

### Safe worktree shell transition

**Status:** Accepted direction; requires shell-integration R&D

Seyal must not silently retarget the cwd of an arbitrary running shell. The preferred path is to create/focus a TerminalExecution whose initial cwd is the agent worktree.

If a user explicitly chooses to adopt an agent worktree into an existing shell, Seyal may automate `cd` only when trusted shell integration proves the shell is back at an idle prompt and the target is safely escaped. Output silence alone is not sufficient evidence.

Seyal must not intercept or replace `git`, inject cwd changes into a running TUI/agent, or guess prompt safety from terminal text.

### Worktree lifecycle UX

**Status:** Accepted direction

User-facing lifecycle should support appropriate combinations of create, pin, archive, cleanup, auto-delete policy, retain-after-agent-completion, merge/review and open/reveal operations.

Worktree remains an execution/work isolation primitive, not the root Seyal Workspace abstraction because many Seyal workloads are not Git-based.

## Agent awareness and supervision

### Tiered Agent Presence Detection

**Status:** Accepted direction

Seyal should support agent presence/status across agents with different integration quality using this confidence order:

1. native adapter / structured protocol;
2. official agent hooks;
3. process identity plus shell/terminal signals;
4. terminal-text/TUI heuristics as a last-resort fallback.

Structured integrations are the primary source. Terminal text fallback exists so unknown agents can still gain basic presence UX, but heuristic state must be labeled/treated as lower confidence.

Heuristic terminal text must never become trusted approval, authorization, audit truth or automatic permission input.

## Source control and CI

### SCM/CI capability adapters

**Status:** Accepted direction

Seyal should expose repository/worktree review state through provider-neutral capability adapters rather than hardcoding one vendor into core architecture.

Potential providers include GitHub, GitLab, Bitbucket, Jenkins, Buildkite and enterprise/custom systems.

The user experience may surface branch, changed files, PR/MR state, checks, reviewers, merge readiness and related actions in workspace/inspector/artifact surfaces.

## Remote execution

### Secure Remote Connection Multiplexing

**Status:** Accepted direction; security R&D required

Multiple compatible remote operations may reuse one authenticated SSH transport rather than repeatedly reconnecting and re-authenticating.

Connection reuse must be keyed by security identity, not hostname alone. The effective identity should account for hostname, port, remote user, authentication identity, host-key/trust policy, proxy/jump chain and Workspace security policy.

Transport reuse must not implicitly enable agent forwarding, port forwarding, X11 forwarding, credential propagation or cross-workspace trust. Different credentials or policy boundaries must produce distinct connection pools.

## Attention and navigation

### Stable Workspace ordering with attention projection

**Status:** Accepted direction

Seyal should not automatically reorder a user's workspace/navigation list whenever attention state changes. Stable spatial ordering is the default.

Attention is projected through badges, the existing Notification/Attention area and the global Attention Stack. A user may optionally choose an attention-based sorting mode, but automatic movement of manually arranged workspaces is not the default behavior.

## Integration and control APIs

### Universal Seyal Integration CLI / Shell API

**Status:** Accepted direction

Any CLI, script or tool should be able to integrate with Seyal without requiring a first-party adapter for every product.

Candidate typed operations include:

- create/update an AttentionItem;
- attach/register an Artifact;
- report progress/status;
- open or register a diff;
- address/focus a Seyal resource;
- expose bounded structured metadata to the current execution/work context.

Commands invoked from inside Seyal can inherit authenticated local context where safe. The interface should prefer typed operations over arbitrary proprietary terminal escape-sequence scraping.

### Capability-scoped Control API

**Status:** Accepted direction; security model required before implementation

External programs may control selected Seyal presentation/workflow operations through authenticated, capability-scoped APIs.

Candidate capabilities include navigation, opening artifacts/diffs, creating panes, focusing executions and creating structured attention/progress state.

Unrestricted arbitrary key injection into unrelated terminal executions must not be the default control model. Access is scoped to explicit identities, workspaces and granted capabilities.

## Blocks

### Block references and reusable actions

**Status:** Accepted direction; builds on Blocks foundation

A Block should be addressable and support useful actions such as:

- bookmark/pin;
- copy/share resource link;
- re-run here;
- re-run in a new shell/execution;
- compare with another run/block;
- save command;
- promote to a reusable workflow/runbook where appropriate.

These operations use Block metadata and stable execution/history references; they must not duplicate PTYs, grids or full terminal output.

## Command Library and reusable workflows

### Saved Commands / Command Library

**Status:** Accepted direction

Users should be able to save a useful command directly from a Block or create one manually, optionally parameterize it, describe it, search it and execute it later.

Recommended scopes:

- **Personal** — local user library;
- **Project** — repository/project-owned definitions that can be version controlled;
- **Team** — centrally shared definitions subject to team/enterprise permissions and policy.

Saved commands should work locally without a Seyal cloud account.

### Parameterized commands

**Status:** Accepted direction

A saved command may turn literal values into typed/user-provided parameters, for example pod, namespace, environment or service. Parameter resolution and quoting must be safe and visible before execution.

### Save command sequence as workflow/runbook

**Status:** Accepted direction

Successful multi-command procedures may be promoted into a reusable Seyal workflow/runbook rather than forcing users to reconstruct operational sequences from history.

The workflow model must use the existing effect/replay/idempotency safety model for mutating operations rather than treating a recorded sequence as blindly replayable.

## Sync and portability

### Local-first settings and knowledge sync

**Status:** Accepted direction

Seyal configuration and reusable knowledge should remain useful locally/offline while allowing optional cloud/team synchronization.

Candidate syncable data includes:

- settings;
- themes;
- keybindings;
- saved commands;
- runbooks/workflow definitions;
- agent preferences;
- selected workspace metadata where appropriate.

Raw terminal history/output, secrets, SSH credentials, sensitive environment values and sensitive artifacts are not automatically synchronized. Any future synchronization of sensitive categories requires explicit user/policy controls, encryption/trust design and retention/deletion semantics.

## Performance as a product feature

### Keystroke latency / key-to-photon performance

**Status:** Accepted direction; measurement work required

Terminal responsiveness is a product feature, not only an implementation quality attribute. A correct terminal that feels materially slower than excellent native terminals is not release-quality Seyal.

Seyal should measure and retain reproducible evidence across the complete interaction path where practical:

physical key / NSEvent
→ native input normalization
→ native/Rust boundary
→ Runtime input path
→ PTY write
→ child response
→ PTY read
→ VT/state mutation
→ damage/projection
→ Metal submit/presentation.

No SwiftUI/application reducer state, agent state, Block state, persistence, cloud, licensing or general reactive state propagation may synchronously sit on the per-keystroke terminal hot path.

Benchmark claims such as "fast" or "performance you feel in every keystroke" must be backed by reproducible measurements before Seyal makes them publicly.

## Inventory maintenance rule

When a product capability is accepted in design/research discussion, add or update it here in the same PR that updates the relevant architecture/R&D document when one exists.

Do not use this inventory as an implementation specification. Significant capabilities still require the repository's normal Issue → R&D/ADR/spec → milestone → tested implementation process.

## Historical feature backfill

Earlier Seyal/RILL product discussions contain a much larger set of feature ideas and accepted directions. They were not previously consolidated into one authoritative feature catalog. Until the historical backfill is completed, architecture/R&D documents and accepted repository decisions remain the source for those older capabilities.

A dedicated backfill issue should enumerate, deduplicate, categorize and classify those historical features into this inventory without importing obsolete RILL architecture decisions into Seyal.