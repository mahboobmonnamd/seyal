# Repository and workspace structure

This document defines Seyal's physical repository layout and the accepted logical ownership/dependency boundaries that are created incrementally as M001 reaches them.

## Principle

A crate/module exists only when it represents a real ownership, portability, process, ABI/dependency, or testing boundary.

The public Seyal repository is the single canonical OSS codebase. Headless, lightweight and full OSS forms are compositions of the same terminal/runtime authority, not separate repositories.

## Current physical Rust layout

M001 Pass 3 has two justified production Rust ownership boundaries:

```text
/
├─ Cargo.toml
├─ Cargo.lock
└─ crates/
   ├─ seyal-terminal/          # canonical portable terminal-semantics ownership boundary
   └─ seyal-exec/              # TerminalExecution / PTY endpoint + child lifecycle boundary
```

`seyal-terminal` owns the permanent incremental VT/parser/state model introduced by Issue #38. `seyal-exec` was introduced by Issue #28 because PTY descriptor ownership, child lifecycle and terminal execution are a real process/platform boundary. The merged Pass 3 implementation feeds PTY bytes into that same authoritative terminal state and keeps the master descriptor encapsulated.

No `seyal-core` crate exists yet because there is not yet a demonstrated shared stable-value boundary that warrants one. Likewise, `seyal-workspace`, `seyal-protocol`, `seyal-render` and `seyal-runtime` remain logical boundaries until their dependency-ordered M001 passes require physical packages. Creating all diagram names as empty crates would violate the repository principle above.

Pass 4 is now the first point at which `seyal-runtime` becomes a justified physical boundary: the headless per-user Runtime must own the execution registry, logical attachments and bounded multi-execution supervision. ADR-006 keeps the macOS PTY readiness-composition mechanism inside `seyal-exec`; the Runtime consumes safe reactor events and does not receive PTY ownership/raw descriptors.

## Current physical native macOS layout

M001 Pass 1 / Issue #10 establishes the permanent native application boundary without terminal behavior:

```text
macos/Seyal/
├─ Seyal.xcodeproj/            # native macOS application target + shared scheme
├─ Info.plist
├─ README.md
└─ Sources/
   ├─ Main.swift               # NSApplication entry point
   ├─ AppDelegate.swift        # AppKit window lifecycle
   └─ MetalSurfaceView.swift   # NSView + CAMetalLayer + MTLDevice seam
```

This boundary is **Swift + AppKit + Metal**. Issue #10 contains no Objective-C, Objective-C++, C++ or SwiftUI terminal surface. Metal shader code, when introduced by its owning renderer Issue, is Metal Shading Language rather than Objective-C. A second native implementation language requires concrete evidence that Swift plus the approved coarse Rust/native boundary cannot satisfy the requirement.

The native skeleton owns no VT/grid/PTY/runtime state and performs no terminal rendering. Its `CAMetalLayer` is only the permanent platform rendering surface seam that later consumes derived renderer data.

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
├─ crates/                     # created incrementally when a real boundary is required
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

Names may be adjusted during M001 only if evidence shows a smaller correct layout, but authority/dependency rules below may not be silently changed.

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
- `seyal-exec`: terminal endpoint/PTY, child lifecycle, `TerminalExecution`, and the macOS safe readiness-composition seam; consumes terminal semantics. Reactor registration never owns the execution.
- `seyal-workspace`: Block/workspace metadata keyed by stable execution/history identities; no PTY/VT ownership.
- `seyal-protocol`: versioned messages/projection types and validation; no authoritative terminal state.
- `seyal-render`: derived render preparation; no canonical VT/grid ownership.
- `seyal-runtime`: per-user authoritative execution registry, logical attachment ownership and bounded multi-execution orchestration.
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

The current two-member Cargo workspace is acyclic by construction: `seyal-exec → seyal-terminal`. `scripts/check-layering.py` validates forbidden edges for physical crates as they appear. The public `Foundation Quality` `repository-policy` job runs this check on every PR/push and `scripts/test-ci-validators.py` proves controlled forbidden dependencies are rejected. The Pass 4 implementation must add `seyal-runtime → seyal-exec` without allowing the reverse edge and update the validator in the same owning Issue.

## Commercial repository boundary

`seyal-commercial` is outside this repository and consumes a pinned Seyal OSS revision as a Git submodule once the canonical public repository identity is finalized.

```text
seyal-commercial/private → Seyal OSS
Seyal OSS                ↛ proprietary code
```

The public repo does not contain private implementations, private SKU modules or license-aware branches. A public extension seam is acceptable only when it is a coherent capability useful to any OSS user and required by a real milestone.

## Forbidden dependencies

- terminal → workspace/Blocks, agents, cloud, licensing, telemetry, UI/native frameworks
- exec → renderer/UI/licensing/cloud/runtime ownership
- runtime readiness → GUI/Swift, renderer, agents, licensing, cloud or commercial code
- workspace → PTY ownership or canonical VT mutation
- render → mutable Runtime/TerminalState internals
- macOS app → a second VT/grid implementation
- OSS production code → proprietary/commercial packages or commercial entitlement state

Enforce these with Cargo workspace layering checks/lints or a small dependency-graph CI script as physical crates are introduced.

## Test/fixture/benchmark locations

- Unit tests live with owning modules.
- External/reference terminal fixtures live under `tests/fixtures/vt` with provenance.
- Cross-crate/runtime/native integration tests live under `tests/integration` or a justified harness package.
- Fuzz targets live under `fuzz/`.
- Reproducible performance workloads/results metadata live under `benches/` and documented artifacts; generated result blobs should not pollute production modules.

These locations were introduced incrementally by their owning Pass-1 Issues. Issue #11 establishes the general harness contracts without fake terminal semantics; production-specific fixtures and fuzz adapters become active only when their owning implementation passes exist. Issue #28 follows the same rule for PTY tests/benchmarks. Pass 4 must add real multi-execution Runtime fairness/resource measurements rather than a no-op reactor benchmark.

## Platform boundary

Portable terminal/runtime behavior is Rust. macOS-only AppKit/Metal/input/accessibility code stays under the macOS host. Darwin `kqueue` details stay in the existing macOS platform/exec boundary rather than leaking into portable Runtime domain logic. Do not create a generic cross-platform GUI or reactor framework before another platform is under active development.

The native-language default is Swift. Introduce Objective-C/Objective-C++ only when a reviewed Issue demonstrates a specific API or interoperability need that Swift cannot satisfy cleanly. Cross the future Rust/native boundary with coarse C-compatible arrays/runs/batches rather than per-cell callbacks.

## Agent instructions

Use root `AGENTS.md`. Add nested `AGENTS.md` only if a real subsystem later needs scoped instructions that would be harmful globally.

## Build interface

M001 Pass 1 Issue #8 pins the deterministic toolchain and canonical root `make bootstrap/build/test/check/bench` interface. Issue #9 activates that interface against the initial Rust workspace. Issue #10 activates the native `Seyal.app` build/smoke path on macOS. Issue #11 establishes the test/fuzz/benchmark harness contracts. Issue #12 makes the corresponding public CI and architecture dependency gates deterministic and self-validating. Issue #38 adds the permanent VT/state implementation. Issue #28 adds the justified `seyal-exec` Pass 3 boundary and PTY behavior. ADR-006/SPEC-003 now define the Pass 4 Runtime/reactor contract before `seyal-runtime` is created by its implementation Issue.
