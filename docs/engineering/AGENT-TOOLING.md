# Agent development tooling

Seyal's agent tooling is developer infrastructure only. No skill, MCP server, agent or external documentation service may become a dependency of terminal input/output, VT state, rendering, persistence, local execution or the OSS runtime.

## Canonical skills

`.agents/skills/` is the single source of truth for Seyal-owned workflows. `.claude/skills/` contains thin adapters only.

| Capability originally requested | Canonical Seyal skill / authority |
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

Equivalent existing skills are intentionally reused rather than creating duplicate aliases with competing instructions.

## External skill

`make bootstrap-agents` installs Anthropic's `frontend-design` skill at a reviewed pinned Git commit for installed supported coding agents. It is for Seyal web prototypes, HTML/CSS artifacts and design exploration only.

It is **not** authority for production AppKit behavior, Metal terminal rendering, native accessibility, runtime state or terminal architecture.

The pinned revision is declared in `scripts/bootstrap-dev.sh` and changes only through normal review.

## MCP/tool matrix

| Tool | Agent bootstrap | Scope |
| --- | --- | --- |
| GitHub MCP | Required when supported locally | Issues, PRs, repository/CI workflow. Credentials remain user-local/runtime-only. |
| Apple Xcode MCP (`xcrun mcpbridge`) | Required when provided by installed Xcode | Official Xcode bridge for build/project/tool access. |
| XcodeBuildMCP | Pinned and registered when `npx` exists | Additional macOS build/test/run, screenshot, UI hierarchy and debugging workflows. Supplemental to Apple's official bridge. |
| Playwright MCP | Pinned and registered when `npx` exists | Web prototype/docs testing only. Never production AppKit/Metal UI authority. |
| AppleDeepDocs | Explicit opt-in only | Supplemental discovery for Apple/Xcode docs. Material decisions still require first-party Apple evidence. |

AppleDeepDocs is intentionally opt-in because it is third-party and can expose a large documentation tool surface. When enabled, agent bootstrap uses its reduced code-execution mode and a pinned source commit.

```sh
SEYAL_ENABLE_APPLE_DEEP_DOCS=1 make bootstrap-agents
```

## Standard setup

First run the deterministic repository/toolchain bootstrap required for build/test/CI:

```sh
make bootstrap
```

Then, when local coding-agent/MCP provisioning is wanted, run the explicit developer-tool setup:

```sh
make bootstrap-agents
```

Agent bootstrap is separate because it may provision pinned user-level developer tools and mutate supported coding-agent configuration. It must not be required for `make build`, `make test`, `make check`, `make bench`, CI, or terminal/runtime operation. It must not write credentials to the repository. It initializes its reviewed/pinned inputs and skips unavailable optional coding-agent CLIs instead of installing those agents implicitly.

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

## Updating external tooling

External skill commits and MCP package versions are intentionally pinned. Updating them requires a normal Issue/PR that records:

1. old and new version/ref
2. upstream release/security changes
3. bootstrap idempotency verification
4. any new permissions, telemetry or tool-surface implications
5. evidence that existing Seyal skills remain the governing workflow
