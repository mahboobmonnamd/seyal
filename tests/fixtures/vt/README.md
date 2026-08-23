# VT fixture corpus

This directory is the retained location for byte-level VT/reference fixtures.

Issue #11 established the harness contract. M001 Pass 2 Issue #38 adds the first semantic fixtures alongside the permanent Seyal VT parser/state implementation.

Each fixture must be registered in `manifest.toml` and include:

- a stable fixture id;
- M001 behavior classification (`supported`, `tested-deferred`, or `unsupported-deferred`);
- exact input bytes in a repository file;
- deterministic expected canonical state/damage data where semantics are claimed;
- positive terminal dimensions used by the fixture;
- provenance matching `provenance.schema.toml`;
- the owning Issue/pass that introduced or changed the expectation.

The harness validates provenance fields and that retained input/expected files exist inside the repository. Rust integration tests execute the fixture bytes against the production `TerminalState` and compare the claimed canonical state.

Reference/conformance expectations must identify the authoritative specification or independently established behavior source. A fixture may not be rewritten merely to match the implementation.

Fixture loaders must sort by stable id/path rather than filesystem enumeration order. Fuzz corpus inputs are separate under `fuzz/` and do not imply correctness semantics.
