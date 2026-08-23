# Repository and workspace structure

This document defines logical ownership/dependency boundaries before M001 Pass 1 creates production scaffolding. It intentionally does **not** create empty crates merely because a diagram contains names.

## Principle

A crate/module exists only when it represents a real ownership, portability, process, ABI/dependency, or testing boundary.

The public Seyal repository is the single canonical OSS codebase. Headless, lightweight and full OSS forms are compositions of the same terminal/runtime authority, not separate repositories.

## Target logical layout

```text
/
├─ AGENTS.md
├─ PRODUCT.md
├─ docs/
│  ├─ architecture/
│  ├─ specs/
│  ├─ milestones/
│  └─ engineering/
├─ crates/                     # created incrementally by M001 Pass 1+
│  ├─ seyal-core/              # stable IDs/common value types only if justified
│  ├─ seyal-terminal/          # VT/TerminalState/history/damage semantics
│  ├─ seyal-exec/              # TerminalEndpoint/PTY + child lifecycle + TerminalExecution
│  ├─ seyal-workspace/         # BlockTimeline/workspace metadata referencing ExecutionId
│  ├─ seyal-protocol/          # versioned local/remote protocol + projection schema
│  ├─ seyal-render/            # portable render preparation, not Metal ownership
│  └─ seyal-runtime/           # per-user authority/orchestration of executions/attachments
├─ macos/
│  └─ Seyal/                   # AppKit/Swift/native Metal host
├─ tests/
│  ├─ fixtures/
│  │  └─ vt/
│  ├─ integration/
│  └─ conformance/
├─ fuzz/
├─ benches/
└─ scripts/
```

Names may be adjusted during Pass 1 if evidence shows a smaller correct layout, but authority/dependency rules below may not be silently changed.

Do not create speculative `headless/`, `lite/`, `full/`, `agent/`, `pro/` or `enterprise/` crates merely to mirror product names. Composition roots should be introduced only when a real binary/application boundary exists.

## OSS variants

Conceptually, all variants consume the same foundational implementation:

```text
                       shared Seyal OSS core
                              │
                ┌─────────────┼─────────────┐
                ▼             ▼             ▼
            headless      lightweight      full OSS
             runtime        terminal        native app
```

The exact binaries/packages are decided by active milestones. The architectural rule is fixed: there is one PTY implementation, one VT/state model, one Runtime ownership model and one renderer architecture. Variants may omit layers they do not need; they do not fork those layers.

## Ownership

- `seyal-terminal`: canonical terminal semantics only. No GUI, licensing, cloud, workspace Blocks or process ownership.
- `seyal-exec`: terminal endpoint/PTY, child lifecycle and `TerminalExecution`; consumes terminal semantics.
- `seyal-workspace`: Block/workspace metadata keyed by stable execution/history identities; no PTY/VT ownership.
- `seyal-protocol`: versioned messages/projection types and validation; no authoritative terminal state.
- `seyal-render`: derived render preparation; no canonical VT/grid ownership.
- `seyal-runtime`: per-user authoritative execution registry and attachment orchestration.
- `macos/Seyal`: AppKit/native lifecycle/input/Metal surface; consumes derived projection only.

## Allowed dependency direction

Conceptually:

```text
core/value types
   ↓
terminal
   ↓
exec
   ↓
runtime

workspace → core/value types + terminal history identity interfaces
protocol  → core/value types + stable projection/value contracts
render    → protocol/derived projection types
macOS app → protocol/render/native bridge
runtime   → exec + terminal + workspace + protocol producer
```

Avoid circular dependencies. If `seyal-core` becomes a dumping ground, split or remove it; it may contain stable identity/value types only.

## Commercial repository boundary

`seyal-commercial` is outside this repository and consumes a pinned Seyal OSS revision as a Git submodule once the canonical public repository identity is finalized.

```text
seyal-commercial/private → Seyal OSS
Seyal OSS                ↛ proprietary code
```

The public repo does not contain private implementations, private SKU modules or license-aware branches. A public extension seam is acceptable only when it is a coherent capability useful to any OSS user and required by a real milestone.

## Forbidden dependencies

- terminal → workspace/Blocks, agents, cloud, licensing, telemetry, UI/native frameworks
- exec → renderer/UI/licensing/cloud
- workspace → PTY ownership or canonical VT mutation
- render → mutable Runtime/TerminalState internals
- macOS app → a second VT/grid implementation
- OSS production code → proprietary/commercial packages or commercial entitlement state

Enforce these with Cargo workspace layering checks/lints or a small dependency-graph CI script once crates exist.

## Test/fixture/benchmark locations

- Unit tests live with owning modules.
- External/reference terminal fixtures live under `tests/fixtures/vt` with provenance.
- Cross-crate/runtime/native integration tests live under `tests/integration` or a justified harness package.
- Fuzz targets live under `fuzz/`.
- Reproducible performance workloads/results metadata live under `benches/` and documented artifacts; generated result blobs should not pollute production modules.

## Platform boundary

Portable terminal/runtime behavior is Rust. macOS-only AppKit/Metal/input/accessibility code stays under the macOS host. Do not create a generic cross-platform GUI layer before a second GUI platform is under active development.

## Agent instructions

Use root `AGENTS.md`. Add nested `AGENTS.md` only if a real subsystem later needs scoped instructions that would be harmful globally.

## Build interface

M001 Pass 1 creates deterministic toolchain pins and wires the root `make bootstrap/build/test/check/bench` interface. Until then, governance validation may exist without fake production crates.
