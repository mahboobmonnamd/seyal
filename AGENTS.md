# Seyal agent entry point

Seyal is an open-source, commercial, enterprise-grade, agent-native terminal workspace. Terminal correctness, low latency, low CPU/RSS, and one authoritative runtime state take priority over local convenience.

## Authority

Read and obey, in order:

1. Seyal Product & Engineering Constitution / project instructions.
2. `docs/architecture/README.md` and accepted foundation architecture.
3. Accepted ADRs and rationale records.
4. Applicable specification or milestone definition.
5. The GitHub Issue marked **Ready**.
6. This repository's engineering procedures.
7. Existing implementation.

An Issue or PR cannot override architecture/specification. Existing code is never architectural authority.

## Non-negotiable architecture invariants

- Runtime owns the authoritative `TerminalState`; the GUI never mirrors a second VT/grid authority.
- One independent `TerminalExecution` owns one terminal endpoint/PTY and one canonical terminal state.
- `BlockTimeline` is Runtime/workspace metadata keyed by `ExecutionId`; Blocks own no PTY, VT, grid, child, renderer, or copied output.
- No synchronous IPC ping-pong, JSON, agents, persistence, cloud, licensing, telemetry, Lua, or Block semantics in terminal hot paths.
- Metal is the first production macOS terminal renderer; no temporary text renderer or temporary production VT path.
- Headless Runtime exists from M001; GUI detach/crash must not kill the execution.
- Terminal fundamentals stay license/cloud independent.

## Before changing code

1. Read the Issue and verify Project status is **Ready**.
2. Read every linked architecture/spec/milestone document.
3. Verify dependencies are complete and ownership/module boundary is explicit.
4. If architecture is missing or contradictory: **STOP** and use the `architecture-change` skill. Do not invent a workaround.
5. Use one Issue → one assignee/agent → one isolated worktree → one branch → one PR.
6. Core behavior is test-first. Do not weaken tests to make code pass.
7. Do not refactor unrelated code. Create/link another Issue instead.
8. If an approved screenshot/mockup is visual authority for native UI, run the `image-to-code` skill before implementation. Complete its forensic design/component inventory and issue plan first; split the work into multiple Issues when the visual spans independently reviewable boundaries.

## Repository map

- `docs/architecture/` — accepted architecture, ADRs, rationale, UI architecture.
- `docs/specs/` — observable behavior specifications (when introduced).
- `docs/milestones/` — bounded vertical milestones and acceptance gates.
- `docs/engineering/` — development, issue, testing, performance, security, repository and OSS/commercial rules.
- `docs/engineering/AGENT-TOOLING.md` — canonical skills, external skill pinning and developer MCP/tool policy.
- `.agents/skills/` — canonical portable agent skills.
- `.github/` — issue/PR forms and CI.

Start with `docs/engineering/DEVELOPMENT.md` and `docs/engineering/REPOSITORY-STRUCTURE.md`.

## Canonical commands

Use the root task interface. Once M001 Pass 1 creates the workspace, these commands must remain canonical:

```sh
make bootstrap
make build
make test
make check
make bench
```

Until production scaffolding exists, `make check` validates governance/documentation only and implementation commands explain that Pass 1 has not yet created the workspace.

## Pull requests

Every implementation PR links its Issue, stays inside scope, cites architecture/spec authority, includes required tests and measurable evidence, states security/docs implications, and gives a reproducible verification procedure. CI evidence is required. Core/high-risk changes require independent review; implementers do not self-approve.

See `docs/engineering/ISSUE-PROTOCOL.md` for Ready/Done rules and `.agents/skills/implement-issue/SKILL.md` for the mandatory execution workflow.
