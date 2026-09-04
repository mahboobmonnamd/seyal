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

## Native UI rule

Material macOS UI changes are incomplete without tests in the same PR.

- Swift UI source changes require XCTest component/invariant coverage.
- Visible or interactive `*View.swift`, `AppDelegate.swift`, or `Main.swift` changes also require XCUIAutomation coverage.
- Native UI E2E must launch the real macOS app through the shared Xcode scheme; browser automation is not a substitute.
- Stable accessibility semantics/identifiers should be used for interactive controls where appropriate.
- `make test` runs the native XCTest/XCUIAutomation suite on macOS; `make ui-test` runs it directly.
- CI enforces the same-PR coverage rule with `scripts/check-ui-test-policy.py` and executes the native tests on the macOS runner.

## Test classes

- **Unit:** local state transitions/invariants.
- **Byte fixtures:** exact terminal bytes → expected canonical state/damage; record provenance for reference/conformance fixtures.
- **Property:** chunking equivalence, invariant preservation, monotonic generations, bounds.
- **Integration:** real PTY/shell, runtime lifecycle, protocol, native boundary.
- **Fuzz:** parser/protocol/projection/reconnect decoders and state boundaries.
- **Renderer:** deterministic projection → draw preparation; controlled image/golden tests only where stable.
- **Failure:** child exit, malformed input, stalled/killed client, reconnect/resync, resource cleanup.
- **Security:** hostile local client inputs, permissions, bounds, authority checks.

## Harness layout

- retained VT/reference fixtures: `tests/fixtures/vt/`;
- cross-boundary integration tests: `tests/integration/`;
- native macOS component tests: `macos/Seyal/Tests/SeyalTests/`;
- native macOS XCUI E2E: `macos/Seyal/Tests/SeyalUITests/`;
- M001 fuzz registry/corpora: `fuzz/`;
- benchmark definitions/environment contract: `benches/`.

`tests/fixtures/vt/manifest.toml` is deterministic and `provenance.schema.toml` defines the minimum provenance record for future semantic fixtures. Pass 1 intentionally contains no fake terminal-behavior fixtures; Pass 2 adds the first real VT corpus test-first.

`fuzz/targets.toml` registers the required M001 fuzz surfaces before their implementations exist. A target remains `pending-production-surface` until its owning pass exposes the real API. Pending targets validate corpus/ownership only. They must not use no-op adapters to claim parser, protocol, projection or reconnect coverage. Once a target is `active`, the smoke runner requires and executes its adapter for every retained seed and requires a mapped libFuzzer binary for campaign parity. `non-production-comparator` rows (for example legacy Candidate-B shared-projection) retain corpora/adapters but are not Pass 10 §6.9 production coverage. Pass surface decisions such as Pass 9 `N/A` are recorded in the registry and proven in `docs/engineering/M001-FUZZ-EVIDENCE.md`.

Canonical `make test` and `make check` validate the harness contracts and fuzz registry. `make bench` validates benchmark-environment recording without publishing a performance result.

## Unsupported behavior

Unsupported or deferred terminal behavior must remain explicitly classified. Tests must verify safe parser continuity where required. Passing through or approximately rendering a sequence does not make it supported.

## M001 special gates

Follow the canonical `docs/milestones/MILESTONE-001.md`. Its retained M001 reference/conformance corpus and local IPC/shared-memory security tests are acceptance requirements, not optional later hardening.

## CI tiers

Fast PR CI (`Foundation Quality`) runs deterministic formatting/lints/build/unit/integration/regression smoke checks appropriate to the changed area. See `docs/engineering/GITHUB-WORKFLOW.md` for the exact job split, including that `native-macos-smoke` runs XCTest/XCUIAutomation and a display-link-off bench (`SEYAL_REQUIRE_DISPLAY_LINK_BENCHMARK=0`) that is **not** headed presentation proof.

Expensive fuzz campaigns, sanitizer matrices, deep renderer checks and performance suites may run path-filtered, scheduled/release, or on controlled hosts. A PR affecting those areas must provide targeted evidence before merge. Green Foundation CI alone is not Pass 10 performance, presentation or continuous-fuzz proof.
