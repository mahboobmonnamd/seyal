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

The repository does not silently install host package managers or execute downloaded shell scripts.

Required before `make bootstrap`:

- Git;
- GNU Make;
- `rustup`, installed explicitly from the official Rust project;
- network access to the official Rust distribution when the pinned toolchain is not already installed.

On macOS, also install/select Apple's Xcode or Command Line Tools so `xcode-select`, `xcrun` and `clang` are available. Swift and the Metal toolchain are not required by Issue #8 because no native application surface exists yet; the Issue that first builds the macOS application must activate and validate those requirements rather than hiding them here.

The repository pins Rust in `rust-toolchain.toml`. For M001 Pass 1 / Issue #8 the pin is Rust **1.98.0** with the `minimal` rustup profile plus `rustfmt` and `clippy`. Cargo is supplied by that same pinned Rust toolchain.

`make bootstrap` is idempotent where rustup permits: it validates host prerequisites, installs/verifies exactly the repository-pinned Rust toolchain/components through rustup, initializes repository-declared pinned submodules if any, and validates the result. It does not run `curl | sh`, invoke Homebrew, install optional MCP servers, or write credentials.

Optional developer-agent/MCP configuration from the repository is deliberately separate:

```sh
make bootstrap-agents
```

That command configures already-installed supported tools where possible; it does not install optional packages or credentials.

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

Current Issue #8 behavior intentionally respects the Issue #8/#9 boundary:

- `make bootstrap` provisions and verifies the pinned toolchain;
- `make build` validates the toolchain and, until Issue #9 creates the Rust workspace, reports that there is nothing to build and succeeds without inventing production code;
- `make test` runs deterministic repository/tooling tests and will also run workspace tests once a real workspace exists;
- `make check` validates the pinned toolchain, shell syntax, governance, local documentation links, architecture layering and tooling tests; workspace format/lint/test gates activate automatically when `Cargo.toml` exists;
- `make bench` validates the toolchain and, until a benchmarkable production surface exists, reports that no benchmark is applicable and makes no performance claim.

Once a real Cargo workspace exists, canonical Cargo operations use the pinned toolchain and `--locked` where dependency resolution applies.

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

There are no required private repositories, `seyal-commercial` dependencies, shell-profile assumptions, Homebrew assumptions or hidden environment variables for this flow.

## Generated and fixture data

Generated files must be clearly marked and reproducible. Fixtures live outside production code and record provenance where external/reference semantics matter. Benchmarks must record environment metadata and be reproducible locally and in CI where practical.
