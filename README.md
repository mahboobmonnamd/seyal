# Seyal

Seyal is an open-source, commercial, enterprise-grade, agent-native terminal workspace for software development and operations.

## Contributing

Seyal OSS welcomes focused, evidence-backed contributions. Start with [CONTRIBUTING.md](CONTRIBUTING.md), choose the appropriate [issue form](.github/ISSUE_TEMPLATE/config.yml), and use the pull-request template. Please read the [Code of Conduct](CODE_OF_CONDUCT.md) and [Security Policy](SECURITY.md) before participating.

The public repository owns the generic terminal foundation. Commercial Pro, Teams, Enterprise, hosted-service, billing, identity, and private-deployment capabilities belong in the separate commercial composition repository and must not become dependencies here.

The foundation architecture is accepted and **Milestone 001 is Done / closed** (Passes 1–10; Issues #5 and #727 closed). Current production work follows the accepted milestone sequence starting at **M002+**. Machine Pass 9 RSS gate remains **`CLIENT_RSS_KIB = 1536`** (unchanged by Pass 10; see #784).

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

## License

Seyal OSS is licensed under the [Apache License 2.0](LICENSE).
