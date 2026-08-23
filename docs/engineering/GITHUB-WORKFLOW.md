# GitHub workflow configuration

Repository documents define policy; GitHub's control plane should enforce it where the platform supports enforcement.

## Project

Create one Seyal engineering Project as a view over repository Issues with Status values:

```text
Backlog
Refinement
Ready
In Progress
In Review
Validation
Blocked
Done
```

Only `Ready` is eligible for implementation pickup. New Issue Forms do not imply Ready.

Recommended views:

- Ready queue: `Status = Ready`, grouped by milestone/parent.
- Active: In Progress + In Review + Validation.
- Blocked: Status = Blocked with dependency fields visible.
- M001: filtered to the M001 milestone/parent hierarchy.

## Issue hierarchy and dependencies

Use native sub-issues for Epic/Feature/Task hierarchy and native issue dependencies for blocked-by/blocking relationships. Do not recreate those relationships as Markdown checklists/comments.

## Issue types

Preferred native types once the repository is organization-owned:

```text
Epic     # custom organization issue type
Feature  # native/default organization issue type
Task     # native/default organization issue type
Bug      # native/default organization issue type
```

GitHub currently manages Issue Types at the organization level. The current personal-account repository therefore cannot satisfy this part of the desired model until it is transferred to a GitHub organization. Until that owner decision is completed, title prefixes/forms may aid readability but must not be treated as an equivalent canonical type system.

## Labels

Keep labels orthogonal to native type. Recommended small set:

### Area

`area:terminal`, `area:vt`, `area:exec`, `area:runtime`, `area:render`, `area:macos`, `area:blocks`, `area:protocol`, `area:workspace`, `area:agents`

### Special state/risk

`blocked`, `needs-spec`, `needs-adr`, `performance-sensitive`, `security-sensitive`, `breaking-change`

Use `type:architecture`, `type:performance`, `type:security`, or `type:spike` only where native types do not express the distinction. Avoid duplicating Feature/Bug/Task as labels after organization Issue Types are available.

## Branch / pull-request protection

The default branch should reject direct production-code pushes and require pull requests. Required checks should include `Foundation Quality / governance` and, once M001 creates production code, the Rust/native build/test jobs relevant to the changed area.

Core/high-risk areas should require at least one independent approving review. Dismiss stale approvals when the head changes materially. Do not allow a merge merely because an implementation agent reports tests passed.

## Public OSS repository CI

The canonical public Seyal repository owns the authoritative GitHub Actions quality gates.

Fast PR gates:

- instruction/governance validation;
- local documentation links;
- architecture layering;
- format/lint/build/unit tests once code exists;
- relevant integration/regression smoke tests.

Deeper scheduled/release or targeted gates as code appears:

- retained VT conformance corpus;
- fuzz campaigns/sanitizers;
- renderer/native validation;
- broad PTY/runtime failure matrix;
- performance/RSS/thread/GPU regression suites;
- dependency/security scanning.

Expensive/noisy checks should be targeted rather than making every documentation PR unusable.

The current repository may temporarily be private during the public-launch transition. A GitHub Actions run that fails before executing any job steps is infrastructure non-execution, not a successful quality result. Once the repository is public, normal PR quality gates must execute and pass.

## Private `seyal-commercial` CI policy

`seyal-commercial` is a private superproject that consumes a pinned Seyal OSS revision.

For now, do **not** add GitHub-hosted Actions workflows to the private repository because of private-repository CI cost. This is a temporary execution-cost decision, not permission to lower engineering standards.

Commercial PRs must still record the canonical local build/test/check/integration evidence relevant to the change. When commercial code becomes substantial, introduce private CI using self-hosted runners or paid hosted capacity rather than relying indefinitely on manual validation.

The public OSS workflow must never be moved into the private repository or made dependent on the private repository.

## Repository ownership note

The current OSS repository is owned by a personal GitHub account. Moving Seyal to an organization is recommended before distributed production implementation so native Issue Types, organization-level governance, team reviewers and future enterprise administration can be configured cleanly.
