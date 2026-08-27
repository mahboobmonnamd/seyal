# Contributing to Seyal

Thank you for helping improve Seyal. This repository is the public open-source foundation for the Seyal terminal workspace.

## Before you start

1. Read [AGENTS.md](AGENTS.md), the applicable specification or ADR, and the relevant milestone acceptance criteria.
2. Search existing issues before opening a new one. Use the issue form that best matches the work.
3. Keep one coherent change per branch and pull request.
4. Do not include secrets, credentials, customer data, or sensitive terminal contents.

## Development workflow

Initialize the repository and reviewed agent skills with:

```sh
make bootstrap
```

Run the canonical checks relevant to your change:

```sh
make build
make test
make check
```

Use the applicable specialist checks for terminal conformance, security, accessibility, performance, fuzzing, documentation, or UI work. Pull requests must report commands run, evidence obtained, and any skipped manual, external, credentialed, E2E, or performance gates.

## Scope and architecture

Keep public generic terminal capabilities here. Commercial Pro, Teams, Enterprise, hosted-service, billing, identity, and private-deployment capabilities belong in the separate commercial composition repository. Do not add a dependency from this repository to proprietary code.

Architecture decisions and implementation requirements remain authoritative in the existing `docs/architecture`, `docs/specs`, `docs/milestones`, and `docs/engineering` documents. Summarize those documents rather than creating competing sources of truth.

## Pull requests

Use the pull-request template. Explain the user-visible behavior, ownership, compatibility and security impact, test evidence, and known gaps. Reviewers must be able to verify the change from the description.

## License

By contributing, you agree that your contributions are licensed under the [Apache License 2.0](LICENSE).
