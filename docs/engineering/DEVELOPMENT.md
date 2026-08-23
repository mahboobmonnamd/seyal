# Seyal development workflow

## Authority chain

```text
Product & Engineering Constitution
→ accepted architecture
→ ADRs/rationale
→ specifications
→ milestone definition
→ Ready GitHub Issue
→ pull request
→ implementation
```

Issues and PRs cannot override higher authority. If implementation evidence contradicts an accepted architectural decision: stop implementation, record evidence, run architecture review/ADR, update affected specification and Issue, then resume.

## Unit of work

Default distributed-development unit:

```text
one Ready Issue
→ one human/agent
→ one isolated worktree
→ issue/<number>-<short-name>
→ one scoped PR
```

One Issue should produce one coherent outcome that can normally be tested, reviewed and merged independently. Large or cross-authority work is refined before implementation. Two active Issues must not mutate the same authoritative subsystem unless independence is explicit and reviewable.

## Mandatory flow

1. Refine the Issue using `.agents/skills/issue-refinement/SKILL.md`.
2. Set Project status to **Ready** only after the readiness checklist in `ISSUE-PROTOCOL.md` passes.
3. Assign one owner and create an isolated worktree/branch.
4. Use tests/fixtures first for core behavior.
5. Implement only the Issue scope.
6. Run `make check` plus issue-specific tests/benchmarks/security checks.
7. Open a PR using the repository template.
8. Require CI evidence; high-risk/core work gets independent review.
9. Move to Validation where milestone/demo/performance evidence is required.
10. Merge only after required gates pass. Do not start a dependent milestone early.

## Scope discipline

Do not perform unrelated cleanup. If an out-of-scope problem is discovered, create/link another Issue and continue unless it blocks the current Issue. Do not turn implementation into architecture by precedent.

## Architecture changes

Use `docs/engineering/ISSUE-PROTOCOL.md` and the `architecture-change` skill. Architecture approval and substantial implementation should normally be separate reviewable changes.

## Development prerequisites

The canonical repository bootstrap does not silently install host package managers or execute downloaded shell scripts.

Required before `make bootstrap`:

- Git;
- `make`;
- `rustup`, installed explicitly from the official Rust project;
- network access to the official Rust distribution when the pinned toolchain is not already installed.

On macOS, M001 now requires **full Xcode**, selected with `xcode-select`, because the permanent native app surface exists. `make bootstrap` validates `xcodebuild`, the macOS SDK, Swift compiler and Metal shader toolchain through `xcrun`; Command Line Tools alone are no longer sufficient for the canonical macOS build.

The repository pins Rust in `rust-toolchain.toml`. M001 Pass 1 currently uses Rust **1.98.0** with the `minimal` rustup profile plus `rustfmt` and `clippy`. Cargo is supplied by that same pinned Rust toolchain.

`make bootstrap` is idempotent where rustup permits: it validates host prerequisites, installs/verifies exactly the repository-pinned Rust toolchain/components through rustup, initializes repository-declared pinned submodules if any, and validates the result. It does not run `curl | sh`, invoke Homebrew, install optional MCP/agent tooling, or write credentials.

Optional developer-agent/MCP provisioning is deliberately separate:

```sh
make bootstrap-agents
```

That explicit opt-in command uses `scripts/bootstrap-dev.sh` and may provision the pinned developer tools documented in `docs/engineering/AGENT-TOOLING.md`. It is not part of build/test/CI bootstrap and must not become a terminal/runtime dependency.

## Canonical task interface

The stable human/agent/CI entry points are:

```sh
make bootstrap
make build
make test
make check
make bench
```

Do not create competing undocumented command paths.

Current behavior after Issue #10:

- `make bootstrap` provisions/verifies the pinned Rust toolchain and, on macOS, validates full Xcode + Swift + macOS SDK + Metal tooling;
- `make build` builds the minimal Rust workspace and, on macOS, builds the native `Seyal.app` Xcode target;
- `make test` validates repository/tooling/workspace scaffold invariants, runs Rust workspace unit tests, and on macOS builds and launches the native app binary in deterministic `--smoke-test` mode;
- `make check` validates the pinned toolchain, shell syntax, governance, local documentation links, architecture layering, workspace scaffold invariants, Rust formatting/Clippy/tests, and the macOS native skeleton on Darwin;
- `make bench` reports that no benchmark target exists until Issue #11 creates the benchmark harness, and makes no performance claim.

Linux remains a supported portable-core CI host; native AppKit/Metal build/test steps explicitly skip there instead of introducing a cross-platform GUI abstraction.

Canonical Cargo operations use the pinned toolchain and `--locked` where dependency resolution applies.

Issue #9 creates only the `seyal-terminal` physical Rust crate because M001 Pass 2 immediately needs the permanent terminal-semantics owner. Other accepted logical boundaries become physical crates only when their dependency-ordered Issues require them; do not pre-create empty diagram-driven packages.

Issue #10 establishes the native host under `macos/Seyal` using **Swift + AppKit + Metal**. No Objective-C/Objective-C++ source is required for the current platform boundary. Metal shaders, when introduced by their owning renderer Issue, use Metal Shading Language; future Rust/native interop should cross a coarse C-compatible boundary rather than per-cell language calls.

## Clean-checkout workflow

From a new clone with the prerequisites above:

```sh
git clone https://github.com/mahboobmonnamd/seyal.git
cd seyal
make bootstrap
make build
make test
make check
make bench
```

On macOS, after `make build`, the current non-terminal native skeleton can be launched manually with:

```sh
open target/macos-derived-data/Build/Products/Debug/Seyal.app
```

There are no required private repositories, `seyal-commercial` dependencies, shell-profile assumptions, Homebrew assumptions or hidden environment variables for this canonical flow.

## Generated and fixture data

Generated files must be clearly marked and reproducible. Fixtures live outside production code and record provenance where external/reference semantics matter. Benchmarks must record environment metadata and be reproducible locally and in CI where practical.
