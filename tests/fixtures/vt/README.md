# VT fixture corpus

This directory is the retained location for byte-level VT/reference fixtures.

Issue #11 creates the harness contract only. It intentionally does **not** add terminal-behavior fixtures before the Seyal VT implementation exists.

Each future fixture must be registered in `manifest.toml` and include:

- a stable fixture id;
- M001 behavior classification (`supported`, `tested-deferred`, or `unsupported-deferred`);
- exact input bytes in a repository file;
- deterministic expected canonical state/damage data where semantics are claimed;
- provenance matching `provenance.schema.toml`;
- the owning Issue/pass that introduced or changed the expectation.

Reference/conformance expectations must identify the authoritative specification or independently established behavior source. A fixture may not be rewritten merely to match the implementation.

Fixture loaders must sort by stable id/path rather than filesystem enumeration order. Fuzz corpus inputs are separate under `fuzz/` and do not imply correctness semantics.
