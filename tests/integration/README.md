# Integration tests

Cross-crate, Runtime, PTY and native integration tests live here when their owning production surfaces exist.

Issue #11 establishes the location and harness contract only; it does not create fake PTY, Runtime, projection, renderer or reconnect tests before those implementations exist.

Rules:

- use real system boundaries when the test claims integration behavior;
- keep deterministic fixtures and temporary runtime state isolated per test;
- never require developer-global shell configuration, secrets or commercial code;
- record platform prerequisites for macOS-only tests;
- retain failure/regression cases once discovered.
