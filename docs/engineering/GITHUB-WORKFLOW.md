# GitHub workflow configuration

Repository documents define policy; GitHub's control plane should enforce it where the platform supports enforcement.

## Project status model

Seyal uses these workflow states conceptually:

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

Only work explicitly marked **Ready** is eligible for implementation pickup.

A GitHub Project with these states is the preferred long-term control plane. While the repository remains under a personal account or the Project is not configured, the explicit `## State` field in each Issue is the approved temporary source of workflow status. Keep it current; do not infer readiness from an open Issue alone.

Recommended Project views once enabled:

- Ready queue: `Status = Ready`, grouped by milestone/parent.
- Active: In Progress + In Review + Validation.
- Blocked: Status = Blocked with dependency fields visible.
- M001: filtered to the M001 milestone/parent hierarchy.

## Issue hierarchy and dependencies

Native sub-issues and native blocked-by/blocking relationships are preferred when available.

Until those controls are available, the approved temporary fallback is explicit parent/dependency text in the Issue body. The relationship must be unambiguous and kept current; do not create a second spreadsheet or planning database merely to emulate GitHub-native relationships.

This fallback is sufficient for current M001 work and must not block terminal implementation. Migrate to native relationships when the repository moves to an organization/control plane that supports them cleanly.

## Issue types

Preferred native types once the repository is organization-owned:

```text
Epic     # custom organization issue type
Feature  # native/default organization issue type
Task     # native/default organization issue type
Bug      # native/default organization issue type
```

GitHub manages native Issue Types at the organization level. While Seyal remains under a personal account, Issue Forms plus clear titles are the approved temporary fallback. Organization transfer is recommended before contributor/team scale, but lack of native Issue Types is not an M001 implementation blocker.

## Labels

Keep labels orthogonal to native type. Recommended small set:

### Area

`area:terminal`, `area:vt`, `area:exec`, `area:runtime`, `area:render`, `area:macos`, `area:blocks`, `area:protocol`, `area:workspace`, `area:agents`

### Special state/risk

`blocked`, `needs-spec`, `needs-adr`, `performance-sensitive`, `security-sensitive`, `breaking-change`

Use `type:architecture`, `type:performance`, `type:security`, or `type:spike` only where native types do not express the distinction. Avoid duplicating Feature/Bug/Task as labels after organization Issue Types are available.

## Branch / pull-request protection

The policy is fixed even when repository settings cannot be inspected or configured from an automation client:

- no production feature work is pushed directly to `master`;
- every implementation change uses branch → pull request → validation/review → merge;
- required CI must be green before merge;
- core/high-risk changes require independent review when an independent reviewer is available;
- stale approvals must not be treated as valid after a material head change.

A GitHub ruleset/branch-protection configuration should enforce these rules when available. Lack of platform-level enforcement is a governance-hardening item, not permission to bypass the policy and not a blocker for the current owner-controlled M001 Pass 1.

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

The repository is now public and `Foundation Quality` has executed successfully with both governance and rust-foundation jobs. A future red or non-executing run is a real readiness signal and must be investigated; it must never be described as passed validation.

## Private `seyal-commercial` CI policy

`seyal-commercial` is a private superproject that consumes a pinned Seyal OSS revision at `oss/seyal`.

For now, do **not** add GitHub-hosted Actions workflows to the private repository because of private-repository CI cost. This is a temporary execution-cost decision, not permission to lower engineering standards.

Commercial PRs must still record the canonical local build/test/check/integration evidence relevant to the change. When commercial code becomes substantial, introduce private CI using self-hosted runners or paid hosted capacity rather than relying indefinitely on manual validation.

The public OSS workflow must never be moved into the private repository or made dependent on the private repository.

## Repository ownership note

The OSS repository is currently public under a personal GitHub account. Moving Seyal to an organization remains recommended before external contributor/team scale so native Issue Types, Projects, team reviewers, rulesets and future enterprise administration can be configured cleanly. That migration is governance hardening and does not block M001 Pass 1 under the temporary fallbacks above.
