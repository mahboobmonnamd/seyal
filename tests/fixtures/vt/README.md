# VT fixture corpus

This directory is the retained location for byte-level VT/reference fixtures.

Issue #11 established the harness contract. M001 Pass 2 Issue #38 added the first semantic fixtures alongside the permanent Seyal VT parser/state implementation. Issue #88 adds independently grounded retained conformance evidence for the supported M001 subset.

Each fixture must be registered in `manifest.toml` and include:

- a stable fixture id;
- M001 behavior classification (`supported`, `tested-deferred`, or `unsupported-deferred`);
- an evidence kind (`project-regression`, `authoritative-spec`, or `independent-reference`);
- exact input bytes in a repository file;
- deterministic expected canonical state/damage data where semantics are claimed;
- positive terminal dimensions used by the fixture;
- provenance matching `provenance.schema.toml`;
- the owning Issue/pass that introduced or changed the expectation.

`coverage.toml` is the retained supported-M001 coverage matrix. It maps each current SPEC-001 behavior family to retained evidence and identifies whether the basis is an external reference, a Seyal-specific invariant, or a documented narrower behavior. The harness fails if a required behavior row disappears, if a referenced retained fixture does not exist, or if the `?1049` row stops recording Seyal's deliberately narrower contract.

The harness validates provenance fields and that retained input/expected files exist inside the repository. Rust integration tests execute the fixture bytes against the production `TerminalState` and compare the claimed canonical state.

Reference/conformance expectations must identify the authoritative specification or independently established behavior source. A fixture may not be rewritten merely to match the implementation. Project-authored regression fixtures remain useful but do not count as independent conformance evidence.

External references are evidence only: no foreign emulator implementation is copied, linked, or introduced as a production dependency. Where an external terminal reference is broader than M001, the difference must be explicit rather than silently broadening Seyal's contract. Mode `?1049` is the current retained example: xterm behavior is reference evidence, while SPEC-001 section 11 remains the narrower Seyal authority.

Fixture loaders must sort by stable id/path rather than filesystem enumeration order. Fuzz corpus inputs are separate under `fuzz/` and do not imply correctness semantics.
