---
name: pr-review
description: Independently review a Seyal pull request against Issue scope, architecture/spec authority, tests, performance, security, docs and unintended changes.
---

# Pull request review

Review independently of the implementation agent when required.

1. Read `AGENTS.md`, linked Issue, referenced architecture/spec/milestone and PR template evidence.
2. Verify the Issue was Ready and the PR remains inside its scope.
3. Diff against authority: look for ownership drift, duplicate state, temporary production paths, new synchronous hot-path dependencies and architecture by precedent.
4. Verify tests prove intended behavior and were not weakened or rewritten around implementation errors.
5. Verify required conformance/fuzz/integration/failure evidence.
6. Verify performance/memory claims against actual measurements and baselines.
7. Run/inspect the required security review for trust-boundary changes.
8. Verify documentation and reproducible demo/verification steps.
9. Identify unrelated cleanup, generated artifacts or hidden dependency changes.
10. Approve only when CI evidence and all applicable gates are complete; otherwise request concrete changes or return the Issue to Refinement/architecture review.

A plausible implementation is not enough if it violates accepted architecture or lacks evidence.
