---
name: rust-fuzzing
description: Add and operate reproducible Rust fuzzing for Seyal parsers, protocols and other untrusted-input boundaries, retaining minimized failures as regression fixtures.
---

# Rust fuzzing

Use this skill for VT/parser/state-machine changes, IPC/protocol decoders, persistence readers, shell-integration parsers and other byte-oriented or untrusted-input surfaces.

1. Identify the trust boundary, invariant and failure class being fuzzed: panic, memory/resource blow-up, invalid state transition, parser desynchronization, data corruption or security failure.
2. Prefer `cargo-fuzz`/libFuzzer for Rust fuzz targets unless the repository has accepted a different harness for the subsystem.
3. Keep fuzz harnesses outside production hot paths and avoid feature changes made only to satisfy the harness.
4. Seed the corpus with deterministic protocol/VT fixtures, boundary cases, empty/truncated inputs, known regressions and realistic control-sequence fragments.
5. Add structured/property fuzz targets where raw bytes alone cannot reach meaningful states efficiently.
6. For streaming parsers, vary chunk boundaries and assert equivalence with the same bytes consumed as a single stream where the protocol permits it.
7. Enforce invariants after every operation where practical: terminal dimensions/state validity, cursor bounds, mode consistency, ownership/lifecycle rules and bounded resource behavior.
8. When a failure is found, preserve the crashing input, minimize it, explain the violated invariant, add a deterministic unit/regression fixture, then fix the permanent implementation.
9. Never discard or suppress a valid crash merely to make fuzz CI green. Quarantine only with an explicit linked Issue and rationale.
10. Maintain a fast smoke fuzz target suitable for CI and a longer local/nightly campaign when infrastructure exists. Record seed/corpus revision and run duration for material evidence.
11. Treat hangs, pathological allocations and unbounded parser work as failures, not only panics.
12. Run normal unit/conformance tests after each fix to ensure the minimized solution does not change unrelated terminal semantics.

Fuzzing complements, but never replaces, specification-based fixtures and terminal conformance testing.