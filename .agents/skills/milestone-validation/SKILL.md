---
name: milestone-validation
description: Validate every Seyal milestone acceptance criterion from actual CI, test, benchmark, failure and demo evidence before the next milestone is authorized.
---

# Milestone validation

Read the accepted milestone, all amendments, linked Issues/PRs, and applicable architecture/specs.

1. Convert each milestone acceptance checkbox and pass-exit requirement into an evidence item.
2. Gather CI/test/conformance/fuzz/integration/renderer/security/performance/failure evidence from merged work, not agent assertions.
3. Run the documented clean-checkout demo procedure.
4. Compare measured performance/memory results with targets; report target vs measured explicitly.
5. Check no non-goal leaked into the milestone and no accepted architecture was silently changed.
6. Check unsupported/deferred terminal behavior is still classified accurately.
7. Require independent review evidence for core/high-risk changes.
8. Mark the milestone complete only if every mandatory criterion is evidenced.

If any required gate lacks evidence, verdict is not ready; list only concrete blockers. Do not start the next milestone to “make progress” while current acceptance is incomplete.
