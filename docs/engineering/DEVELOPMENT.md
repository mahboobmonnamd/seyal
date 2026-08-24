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

The `.sdlc` context layer is deliberately **not** inserted into the authority chain. It is a compact navigation/provenance layer that helps agents find the relevant authoritative artifacts without rereading the repository.

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

1. When project context beyond the Issue links is needed, use `project-context` to retrieve the smallest relevant node/relationship set, validate the derived index, and read the returned authoritative sources. A stale/no-match index routes to targeted source search; it never authorizes guessing.
2. Refine the Issue using `.agents/skills/issue-refinement/SKILL.md`.
3. Set Project status to **Ready** only after the readiness checklist in `ISSUE-PROTOCOL.md` passes.
4. Assign one owner and create an isolated worktree/branch.
5. Use tests/fixtures first for core behavior.
6. Implement only the Issue scope.
7. Assess **Documentation impact** before final validation. Run the `docs-authoring` skill and update the User Guide and/or Developer Guide in the same Issue/PR when applicable. If no documentation is needed, record a concrete `N/A` rationale in the PR.
8. Run `make check` plus issue-specific tests/benchmarks/security checks. When documentation changed, also run `make docs-check` and `make docs-build`.
9. Open a PR using the repository template, including documentation evidence or the `N/A` rationale.
10. Require CI evidence; high-risk/core work gets independent review.
11. Move to Validation where milestone/demo/performance evidence is required.
12. Merge only after required gates pass. Do not start a dependent milestone early.

## Documentation lifecycle

Documentation is part of feature completeness, not a default follow-up task.

Use `.agents/skills/docs-authoring/SKILL.md` whenever implementation adds or changes:

- user-visible behavior, commands, configuration, workflows, troubleshooting or interaction patterns;
- contributor setup, build/test workflow, architecture orientation, public extension points or engineering procedures;
- screenshots, diagrams or documentation media.

Choose the audience deliberately:

- **User Guide** for observable product behavior and tasks;
- **Developer Guide** for contributor orientation and development workflows;
- authoritative ADR/spec/architecture/engineering records remain under the repository `docs/` authority paths and must not be duplicated into the site as competing truth.

A change with no documentation impact must say why in the PR. Do not satisfy the gate by documenting planned behavior as shipped. Documentation should normally land with the implementation that makes it true so code and docs cannot drift immediately after merge.

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

That explicit opt-in command uses `scripts/bootstrap-dev.sh`, materializes the exact reviewed AI-SDLC developer-framework pin under ignored `.sdlc/framework/`, and may provision the other pinned developer tools documented in `docs/engineering/AGENT-TOOLING.md`. It is not part of product build/test/CI bootstrap and must not become a terminal/runtime dependency.

## Canonical task interface

The stable product human/agent/CI entry points are:

```sh
make bootstrap
make build
make test
make check
make bench
```

Documentation tooling is an opt-in development surface and remains outside the product runtime/build hot path:

```sh
make docs          # install docs dependencies and start the local documentation server
make docs-install  # install documentation dependencies only
make docs-build    # build the static documentation site
make docs-check    # run Starlight/Astro documentation validation
```

`make docs` requires Node.js 22.12 or later. Do not create competing undocumented command paths.

Current behavior after Issue #12:

- `make bootstrap` provisions/verifies the pinned Rust toolchain and, on macOS, validates full Xcode + Swift + macOS SDK + Metal tooling;
- `make build` builds the minimal Rust workspace and, on macOS, builds the native `Seyal.app` Xcode target;
- `make test` validates repository/tooling/workspace and harness invariants, validates the M001 fuzz registry/corpora without pretending pending production targets are active, runs Rust workspace unit tests, and on macOS runs the native app smoke;
- `make check` runs the deterministic repository checks, harness/fuzz validation, controlled negative fixtures proving custom validators actually reject bad inputs, Rust formatting/Clippy/tests, architecture layering and the macOS native skeleton on Darwin;
- `make bench` records and round-trips benchmark environment metadata under `target/benchmarks/`, explicitly marks the harness smoke as not a performance result, and runs real Cargo benchmark targets only once they actually exist;
- `make docs` starts the local Starlight documentation site after installing its isolated Node dependencies;
- `make docs-build` and `make docs-check` validate documentation without becoming dependencies of terminal production execution.

The public `Foundation Quality` workflow separates the fast PR gates into `repository-policy`, `rust-and-harness-quality`, and `native-macos-smoke`. See `docs/engineering/GITHUB-WORKFLOW.md` for the exact responsibility and required-check contract. Linux remains a supported portable-core CI host; native AppKit/Metal build/test steps explicitly skip there instead of introducing a cross-platform GUI abstraction.

Canonical Cargo operations use the pinned toolchain and `--locked` where dependency resolution applies.

Issue #9 creates only the `seyal-terminal` physical Rust crate because M001 Pass 2 immediately needs the permanent terminal-semantics owner. Other accepted logical boundaries become physical crates only when their dependency-ordered Issues require them; do not pre-create empty diagram-driven packages.

Issue #10 establishes the native host under `macos/Seyal` using **Swift + AppKit + Metal**. No Objective-C/Objective-C++ source is required for the current platform boundary. Metal shaders, when introduced by their owning renderer Issue, use Metal Shading Language; future Rust/native interop should cross a coarse C-compatible boundary rather than per-cell language calls.

Issue #11 establishes retained harness locations under `tests/`, `fuzz/` and `benches/`. It does not create terminal behavior to make the harnesses look populated. Production-specific fuzz adapters are activated by the pass that introduces the real target API.

Issue #12 makes the Pass-1 CI gates production-shaped: external workflow actions are pinned by reviewed commit SHA, workflow permissions remain minimal, repository validators are negative-fixture tested, and architecture layering is enforced in the public PR path. Deeper conformance/fuzz/performance/security campaigns remain deferred until their real production targets exist.

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

For coding-agent/project-context tooling, explicitly opt in:

```sh
make bootstrap-agents
python3 .sdlc/framework/tools/project_context.py --root . validate
```

To preview the documentation locally (Node.js 22.12+):

```sh
make docs
```

On macOS, after `make build`, the current non-terminal native skeleton can be launched manually with:

```sh
open target/macos-derived-data/Build/Products/Debug/Seyal.app
```

There are no required private repositories, `seyal-commercial` dependencies, shell-profile assumptions, Homebrew assumptions or hidden environment variables for this canonical product flow. AI-SDLC is an optional public developer-framework dependency materialized only by `make bootstrap-agents`.

## Generated and fixture data

Generated files must be clearly marked and reproducible. Fixtures live outside production code and record provenance where external/reference semantics matter. Benchmarks must record environment metadata and be reproducible locally and in CI where practical.
