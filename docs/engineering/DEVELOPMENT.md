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

## Canonical task interface

The stable human/agent entry points are:

```sh
make bootstrap
make build
make test
make check
make bench
```

M001 Pass 1 must wire these commands to deterministic Rust/native tooling. Agents must not create competing undocumented command paths.

## Generated and fixture data

Generated files must be clearly marked and reproducible. Fixtures live outside production code and record provenance where external/reference semantics matter. Benchmarks must record environment metadata and be reproducible locally and in CI where practical.
