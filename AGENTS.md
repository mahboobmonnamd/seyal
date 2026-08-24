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

An Issue or PR cannot override architecture/specification. Existing code is never architectural authority. `.sdlc` context/index data is navigation support only and cannot override any authority above.

## Non-negotiable architecture invariants

- Runtime owns the authoritative `TerminalState`; the GUI never mirrors a second VT/grid authority.
- One independent `TerminalExecution` owns one terminal endpoint/PTY and one canonical terminal state.
- `BlockTimeline` is Runtime/workspace metadata keyed by `ExecutionId`; Blocks own no PTY, VT, grid, child, renderer, or copied output.
- No synchronous IPC ping-pong, JSON, agents, persistence, cloud, licensing, telemetry, Lua, or Block semantics in terminal hot paths.
- Metal is the first production macOS terminal renderer; no temporary text renderer or temporary production VT path.
- Headless Runtime exists from M001; GUI detach/crash must not kill the execution.
- Terminal fundamentals stay license/cloud independent.

## Module cohesion and design patterns

- Prefer single-responsibility modules with explicit ownership and lifecycle boundaries.
- Use design patterns only when they solve a concrete problem in ownership, lifecycle, extensibility, testability, or correctness. Do not add abstraction layers for pattern purity.
- For handwritten production code, roughly 500–700 lines in one file is a **cohesion review trigger**, not a hard limit. Review whether responsibilities should be separated.
- Handwritten production files above 1,000 lines require explicit PR justification and should normally be decomposed before merge.
- Generated tables/data, Unicode data, protocol fixtures, exhaustive conformance vectors, and comparable machine-oriented artifacts are exempt from the line-count guidance.
- Split by responsibility and stable boundaries, never into arbitrary numbered files such as `part1`, `part2`, or equivalent.
- Avoid god objects/types that own unrelated terminal, runtime, renderer, persistence, agent, or UI concerns.
- Prefer composition and narrow interfaces. Avoid factories, service layers, dependency-injection frameworks, or other indirection unless they materially improve the design.
- Structural refactoring must not add synchronous IPC, serialization, copies, allocations, locks, thread/process hops, or language round-trips to terminal hot paths merely to satisfy code organization rules.
- These rules apply to Rust and native macOS Swift/Metal code equally.

## Before changing code

1. Read the Issue and verify Project status is **Ready**.
2. When the task needs broader project context than the Issue links provide, use the `project-context` skill to load the smallest relevant node/relationship set, validate it, then read the returned authoritative sources. Do not broadly reread the repository or trust the index summary as authority.
3. Read every linked/retrieved architecture/spec/milestone document that materially governs the work.
4. Verify dependencies are complete and ownership/module boundary is explicit.
5. If architecture is missing or contradictory: **STOP** and use the `architecture-change` skill. Do not invent a workaround.
6. Use one Issue → one assignee/agent → one isolated worktree → one branch → one PR.
7. Core behavior is test-first. Do not weaken tests to make code pass.
8. Do not refactor unrelated code. Create/link another Issue instead.
9. If an approved screenshot/mockup is visual authority for native UI, run the `image-to-code` skill before implementation. Complete its forensic design/component inventory and issue plan first; split the work into multiple Issues when the visual spans independently reviewable boundaries.

## Repository map

- `docs/architecture/` — accepted architecture, ADRs, rationale, UI architecture.
- `docs/specs/` — observable behavior specifications (when introduced).
- `docs/milestones/` — bounded vertical milestones and acceptance gates.
- `docs/engineering/` — development, issue, testing, performance, security, repository and OSS/commercial rules.
- `docs/engineering/AGENT-TOOLING.md` — canonical skills, generic AI-SDLC pinning and developer MCP/tool policy.
- `.sdlc/context/` — project-owned portable SDLC metadata/context; never higher authority than source artifacts.
- `.sdlc/graph/` — compact derived navigation index for low-context agent retrieval.
- `.sdlc/framework/` — ignored local materialization of the reviewed AI-SDLC developer framework.
- `.agents/skills/` — Seyal-owned skills plus thin adapters for pinned generic capabilities.
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

`make bootstrap-agents` is optional developer setup for coding-agent/MCP tooling and the pinned AI-SDLC framework; it is never required by terminal/runtime operation.

Until production scaffolding exists, `make check` validates governance/documentation only and implementation commands explain that Pass 1 has not yet created the workspace.

## Pull requests

Every implementation PR links its Issue, stays inside scope, cites architecture/spec authority, includes required tests and measurable evidence, states security/docs implications, and gives a reproducible verification procedure. CI evidence is required. Core/high-risk changes require independent review; implementers do not self-approve.

See `docs/engineering/ISSUE-PROTOCOL.md` for Ready/Done rules and `.agents/skills/implement-issue/SKILL.md` for the mandatory execution workflow.
