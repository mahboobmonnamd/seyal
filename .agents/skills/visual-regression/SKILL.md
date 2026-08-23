---
name: visual-regression
description: Compare approved Seyal visual references with native implementation screenshots using reproducible screenshot capture and explicit tolerances.
---

# Visual regression

1. Identify the approved source visual and the exact workflow/state being compared.
2. Capture implementation screenshots at controlled window size, scale, appearance and content.
3. Compare geometry, spacing, typography, clipping, overlays and state transitions; do not accept "roughly similar" for a fidelity task.
4. Use pixel/image diffing where stable; mask only documented nondeterministic regions.
5. Keep terminal correctness separate from appearance: visual baselines cannot redefine VT/grid semantics.
6. Update a baseline only when the design change is intentional and reviewable; never regenerate baselines merely to make CI pass.
7. Attach before/after/diff evidence to the PR for material visual changes.
