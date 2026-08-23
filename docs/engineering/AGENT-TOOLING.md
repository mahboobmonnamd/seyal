# Agent development tooling

Seyal's agent tooling is developer infrastructure only. No skill, MCP server, agent or external documentation service may become a dependency of terminal input/output, VT state, rendering, persistence, local execution or the OSS runtime.

The tooling set is intentionally minimal: do not add generic developer tools merely because they may be useful in other projects. Every external tool must have a concrete Seyal use case that is not adequately covered by the existing native toolchain.

## Canonical skills

`.agents/skills/` is the single source of truth for Seyal-owned workflows. `.claude/skills/` contains thin adapters only.

| Capability | Canonical Seyal skill / authority |
| --- | --- |
| Seyal architecture | `AGENTS.md` + `architecture-change` |
| Native macOS design | `macos-native-design` |
| Native macOS UI testing | `macos-ui-testing` |
| macOS accessibility | `macos-accessibility` |
| Visual regression | `visual-regression` |
| Screenshot/mockup to native implementation | `image-to-code` |
| VT TDD | `vt-tdd` |
| Terminal conformance | `terminal-conformance` |
| Terminal performance | `performance-gate` |
| Metal rendering | `metal-renderer` |
| Rust/parser fuzzing | `rust-fuzzing` |
| Security review | `security-review` |
| PR quality gate | `pr-review` + `milestone-validation` |
| Current Apple platform docs/HIG | `apple-platform-docs` |
| Issue decomposition | `issue-refinement` |

Equivalent existing skills are intentionally reused rather than creating duplicate aliases with competing instructions. Seyal does not install generic external design skills; native AppKit/Metal work is governed by the project skills above and current Apple platform evidence.

## Approved MCP/tool matrix

| Tool | Agent bootstrap | Seyal use |
| --- | --- | --- |
| GitHub MCP | Required when supported locally | Issues, PRs, repository and CI workflow. Credentials remain user-local/runtime-only. |
| Apple Xcode MCP (`xcrun mcpbridge`) | Required when provided by installed Xcode | First-party Xcode project/build/tool integration. |
| XcodeBuildMCP | Pinned and registered when `npx` exists | Native macOS build/test/run, screenshots, UI hierarchy and debugging needed by Seyal UI implementation and validation. |

Browser automation, generic web/front-end design helpers and third-party Apple documentation indexes are not part of Seyal's development bootstrap.

## Standard setup

First run the deterministic repository/toolchain bootstrap required for build/test/CI:

```sh
make bootstrap
```

Then, when local coding-agent/MCP provisioning is wanted, run:

```sh
make bootstrap-agents
```

The agent bootstrap may provision only the approved project-required tooling above and may mutate supported coding-agent configuration. It must not be required for `make build`, `make test`, `make check`, `make bench`, CI, or terminal/runtime operation. It must not write credentials to the repository.

When `seyal-commercial` invokes the pinned OSS `bootstrap-agents` target from its own bootstrap, the same minimal tooling policy applies.

## Screenshot-to-native work

When an approved screenshot/mockup is an implementation reference, use `image-to-code` before writing production UI code. The workflow requires:

```text
source screenshots
→ forensic visual/component inventory
→ design document
→ issue/dependency plan
→ implementation by Ready Issue
→ controlled screenshot/diff convergence
→ native interaction/accessibility validation
```

If the visual spans multiple independently reviewable boundaries, create multiple dependent Issues. Do not implement a large multi-region screenshot as one unreviewable PR merely because it originated from one image.

## Adding or updating external tooling

A new external skill, MCP server or developer dependency requires a normal Issue/PR that records:

1. the concrete Seyal workflow it enables;
2. why existing Seyal skills, Xcode tooling or GitHub tooling are insufficient;
3. version/ref and reproducibility strategy;
4. bootstrap idempotency verification;
5. permissions, credentials, telemetry and tool-surface implications; and
6. evidence that it does not become a runtime or terminal hot-path dependency.

If the tool is merely useful for generic web/front-end work, unrelated repositories or optional documentation discovery, it does not belong in Seyal bootstrap.
