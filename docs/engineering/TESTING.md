# Testing

Core Seyal behavior is test/spec driven.

## Rule

```text
test/fixture proving desired behavior
→ implementation
→ regression validation
```

Do not write implementation first and then add tests that merely describe what happened to be built. Never weaken a valid test to make implementation pass.

## Required high-rigor areas

Use TDD for:

- VT parser/state and mode handling
- Unicode/grapheme/width behavior
- PTY lifecycle and child management
- attach/detach/reconnect
- binary protocol and projection validation
- Block invariants and line anchors
- persistence contracts
- security boundaries

Rendering may combine deterministic unit tests, projection fixtures, controlled golden/reference tests, native integration tests, and end-to-end validation.

## Test classes

- **Unit:** local state transitions/invariants.
- **Byte fixtures:** exact terminal bytes → expected canonical state/damage; record provenance for reference/conformance fixtures.
- **Property:** chunking equivalence, invariant preservation, monotonic generations, bounds.
- **Integration:** real PTY/shell, runtime lifecycle, protocol, native boundary.
- **Fuzz:** parser/protocol/projection/reconnect decoders and state boundaries.
- **Renderer:** deterministic projection → draw preparation; controlled image/golden tests only where stable.
- **Failure:** child exit, malformed input, stalled/killed client, reconnect/resync, resource cleanup.
- **Security:** hostile local client inputs, permissions, bounds, authority checks.

## Unsupported behavior

Unsupported or deferred terminal behavior must remain explicitly classified. Tests must verify safe parser continuity where required. Passing through or approximately rendering a sequence does not make it supported.

## M001 special gates

Follow the canonical `docs/milestones/MILESTONE-001.md`. Its retained M001 reference/conformance corpus and local IPC/shared-memory security tests are acceptance requirements, not optional later hardening.

## CI tiers

Fast PR CI runs deterministic formatting/lints/build/unit/integration/regression smoke checks appropriate to the changed area. Expensive fuzz campaigns, sanitizer matrices, deep renderer checks and performance suites may run scheduled/release, but a PR affecting those areas must provide targeted evidence before merge.
