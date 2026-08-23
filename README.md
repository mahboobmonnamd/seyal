# Seyal

Seyal is an open-source, commercial, enterprise-grade, agent-native terminal workspace for software development and operations.

The foundation architecture is accepted and **Milestone 001 is ready for implementation**. Production work must follow the accepted milestone sequence and its pass/acceptance gates.

## Start here

Read [`docs/architecture/README.md`](docs/architecture/README.md).

That index links to the accepted foundation architecture, its decision/prohibition rationale, distinct ADRs, the authoritative M001 implementation specification, UI architecture, and source research brief.

## Documentation rule

Keep one canonical file for one purpose:

- foundation architecture decisions live in the foundation document;
- M001 implementation scope, passes, tests, security gates, benchmarks, and acceptance criteria live in `docs/milestones/MILESTONE-001.md`;
- rationale documents explain why decisions exist;
- ADRs are reserved for genuinely separate architectural decisions with their own lifecycle;
- Git history and pull requests preserve prior wording and corrections.

Do not create competing `-v2`, `-final`, `-new`, `-amendment`, or correction copies when the owning canonical document can be updated directly.
