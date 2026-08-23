---
name: vt-tdd
description: Add or change Seyal VT behavior through specification/reference evidence, byte fixtures, state expectations, implementation, conformance and fuzz consideration.
---

# VT test-driven development

Read `AGENTS.md`, `docs/engineering/TESTING.md`, the active terminal specification/milestone matrix, and relevant authoritative/reference terminal semantics.

Workflow:

1. Classify the sequence/behavior as supported, tested-but-deferred, or unsupported for the active milestone.
2. Record authoritative/reference provenance where practical.
3. Add exact byte fixture(s), including split-read boundaries where meaningful.
4. Define expected canonical `TerminalState`, modes, cursor/grid and damage effects.
5. Add property/regression expectations for chunking and parser continuity.
6. Run tests and confirm the new supported behavior fails before implementation.
7. Implement only the scoped parser/state behavior in the permanent Seyal VT path.
8. Run retained reference/conformance corpus and parser/state fuzz smoke/regression corpus.
9. Verify deferred/unknown sequences do not corrupt subsequent supported parsing or silently become “supported”.
10. Capture any disagreement with reference behavior explicitly; do not normalize tests to implementation without evidence.

Do not add an alternate VT dependency, temporary parser, GUI VT mirror, or terminal semantics in the renderer.
