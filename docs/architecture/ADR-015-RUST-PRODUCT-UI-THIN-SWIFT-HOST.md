# ADR-015 — Rust Product UI and Thin Swift Host

- **Status:** Accepted on merge
- **Date:** 2026-09-12
- **Issue:** #877
- **Depends on:** `R-010`, `R-011`, `R-012`, ADR-004, ADR-007, ADR-009, ADR-011, SPEC-006, SPEC-007, SPEC-008, SPEC-009, [`SEYAL-UI-ARCHITECTURE-001.md`](ui/SEYAL-UI-ARCHITECTURE-001.md)
- **Coordinates with:** #875 / PR #876 agent and review enforcement; M001.1 parent #878

## Decision requested

This repository keeps one mixed-language macOS application. It does **not** extract
Swift UI to another tree and does **not** delete Swift.

Ownership is split by *authority*, not by deleting a language:

```text
Rust owns portable Seyal product/UI state, policy and behavior.
Swift owns only inherently macOS platform integration and disposable native realization.
```

There is no second Swift UI product elsewhere. Future Windows/Linux hosts must
implement the same portable Rust product contracts; they must not reimplement
Seyal product semantics.

macOS-first means implementation order, not architecture ownership. This ADR does
not create a generic cross-platform GUI abstraction or choose a universal widget
toolkit before another platform is actively developed.

## Problem and conflict

`R-010` assigns macOS application/platform behavior to Swift + AppKit. That is
correct for windowing, IME, accessibility and Metal drawable hookup. It is not
a license for Swift to own Workspace/Tab/Pane models, Blocks, composer,
commands, focus, layout, presentation policy, theme/config semantics, recovery
or agent/inspector behavior.

#875 / PR #876 already made that split a merge-blocking agent/review rule.
This ADR is the independent architecture decision those rules enforce. It defines language ownership only: existing accepted ADRs/specifications remain the behavior authority. In any conflict about which language may own a behavior, ADR-015 governs ownership; in any conflict about behavior, the owning contract governs and must be changed through its own review/merge gate. This ADR does not promote or modify separately proposed behavior amendments.

A prior reading of #877 that would remove all Swift sources, isolate a separate
Swift UI repository, or keep portable product authority in Swift because macOS
ships first is rejected.

## Alternatives considered

### A. Remove all Swift from this repository

Rejected. IME (`NSTextInputClient`), accessibility, AppKit window lifecycle and
Metal drawable creation remain first-class macOS integration. Deleting Swift
would force a premature universal GUI or a weaker input/accessibility path.

### B. Extract Swift UI to a sibling repository / superproject tree

Rejected. Product intent is one OSS application with a thin native host, not
two UI products. A second tree would duplicate packaging, CI and the
Rust↔native boundary without changing terminal authority.

### C. Keep product UI in Swift because the macOS host already exists

Rejected. That is the drift #875 forbids: Windows/Linux hosts would have to
rewrite Seyal feature logic. Passing XCTest/XCUI and local convenience are not
architecture.

### D. Rust product/UI authority; Swift thin OS adapter in this repository

Selected. Matches `R-010` native-host intent, `R-012` (no premature universal
GUI framework), and Metal as the production renderer (`R-011`).

## Normative split

### Rust MUST own portable product authority

At minimum:

- Workspace identity and persisted domain state remain under the single Runtime/domain authority defined by ADR-007. Rust owns derived portable Tab/Split/PaneView presentation state that references stable Workspace identity; M001.1 must reuse, not duplicate, the Workspace authority
- split, focus and navigation policy/actions
- Block state/lifecycle and portable Block presentation semantics
- composer draft/submission lifecycle and command submission semantics
- portable Seyal command/action decisions and product-state transitions after native event classification/normalization. SPEC-006 native `ApplicationCommand` routing remains in the application layer and never reaches the PTY; any such event that changes portable Seyal state is forwarded as a typed Rust action
- Flow / Raw / TUI mode, transition, input-route and presentation-epoch policy
- renderer **intent** and portable presentation policy, including whether full-grid,
  Block-region, cursor or takeover presentation is allowed
- Agent / Inspector / Activity / attention product state and projections
- portable theme/configuration schema, defaults, validation, bounds, precedence,
  semantic color/typography/spacing/depth/motion intent
- portable reconnect/recovery orchestration state, retry/deadline policy and
  continuity decisions; OS lifecycle signals remain native inputs
- platform-independent conformance tests for the above behavior

Rust ownership does not imply that one new crate must mirror every former Swift
file. Reuse existing authority boundaries where appropriate and create a new
module/crate only when a real ownership/dependency boundary justifies it.

### Swift MAY own

- `NSApplication` / `NSWindow` lifecycle and native view construction
- native event collection and primitive normalization;
- native event recognition/classification required by SPEC-006; forward every Seyal product command as a typed event/action for Rust to resolve, including menu and keyboard commands
- IME / `NSTextInputClient` bridge, preedit/selection and first-responder handles
- accessibility adapter / VoiceOver integration
- clipboard / drag-drop / macOS services
- observation and forwarding of raw macOS appearance/accessibility inputs; Rust owns their portable semantic interpretation
- `NSColor` / `NSFont` / `NSVisualEffectView` realization of Rust semantic intent
- Metal drawable/surface/device/queue/pipeline/resource integration
- CoreText/native font realization or shaping where platform-specific
- bundle-relative helper path/signature validation and invocation of the macOS spawn API; Rust owns helper selection, launch count, retry/deadline, endpoint, recovery and failure policy under SPEC-009
- bounded disposable view/resource/cache state required by those APIs

Swift may render pixels and forward events. Presentation state shown by Swift
must be derived from the authoritative Rust model. Native ephemeral state such
as hover, press-preview, IME preedit, view identity and GPU resources is not a
second product authority.

### Swift MUST NOT own

- a writable Workspace / Tab / Pane product model
- Block state/lifecycle or an independent transcript authority
- composer product behavior or authoritative draft/submission state
- command submission semantics
- keyboard command decisions beyond primitive native event normalization
- focus/navigation policy
- split/layout product state
- Flow / Raw / TUI state machine or transition policy
- renderer presentation-policy decisions that should be portable
- Agent / Inspector / Activity / attention product state
- portable theme/config defaults, validation, precedence or semantic design rules
- portable reconnect/recovery policy
- other Seyal feature/business logic

If a behavior would have to be rewritten for a second desktop host, it is
Rust-owned unless an accepted architecture decision proves that the behavior is
inherently OS-specific. Ambiguous ownership is an architecture stop, not a
Swift default.

## Rust/native interaction boundary

The preferred product/UI seam is coarse typed **actions/events in** and
immutable/derived **snapshots or projections out**:

```text
native OS event / lifecycle signal
        ↓ normalize only what is OS-specific
Rust typed action / authoritative transition
        ↓
Rust product snapshot / renderer intent
        ↓
native AppKit / accessibility / Metal realization
```

The boundary must not become per-cell, per-glyph, per-frame, JSON, or
synchronously chatty merely to move authority to Rust. Existing coarse
Candidate-D/prepared-render patterns remain the performance model.

Terminal input/output/render progress must never synchronously depend on
product UI state transfer, platform-host acknowledgement, agent work,
persistence or cloud services.

## Migration discipline

M001.1 is a controlled authority reset, not another terminal/runtime rewrite.
Existing proven Rust PTY/VT/Runtime/history/render-preparation foundations are
preserved.

For each portable responsibility currently in Swift:

```text
inventory current Swift behavior + tests
→ define the Rust authoritative contract
→ add platform-independent Rust conformance tests
→ rewire/rewrite the Swift side as a thin adapter over that contract
→ prove headed macOS behavior parity
→ delete the obsolete Swift product authority
```

Do not keep a long-lived old/new dual implementation. A temporary migration seam
is permitted only when one side is explicitly read-only/derived and the single
writer/authority is unambiguous and tested.

## Invariants that remain true

1. One `TerminalExecution` owns one PTY and one canonical `TerminalState`.
   The GUI never owns a second VT/grid.
2. Flow / Raw / TUI presentation semantics remain governed by ADR-009 and its applicable specifications, including each amendment's own acceptance status. ADR-015 assigns portable presentation-policy ownership to Rust but does not adopt or modify presentation behavior.
3. Metal remains the production macOS terminal renderer. No NSTextView,
   SwiftUI, or CPU-full-frame terminal engine (`R-011`).
4. No per-cell/per-glyph/per-frame Rust↔Swift callback on the terminal hot path.
5. IME preedit stays ephemeral host state as required by ADR-011. Swift may
   own the `NSTextInputClient` bridge; it may not commit preedit into
   `TerminalState` or own portable composer/terminal semantics.
6. Headless Runtime survives GUI detach/crash; SPEC-009 continuity semantics are
   preserved while portable client recovery orchestration moves to Rust.
7. This ADR does not choose a new Rust UI toolkit. Toolkit/backend choices require
   evidence from the platform being implemented.
8. OSS remains independent of commercial code.

## Cross-platform parity rule

A feature may ship on macOS first, but portable product behavior is not considered
architecturally complete if a future Windows/Linux host would have to rediscover
or reimplement Seyal semantics that should already exist in Rust.

Future hosts may use different native windowing, accessibility, IME, clipboard
and GPU APIs. They consume the same portable Rust models/actions/policies and
provide platform adapters for the OS-specific pieces.

## Required follow-up (not this PR)

- #886 performs an exhaustive source-by-source Swift ownership/parity manifest
  before production migration begins.
- #879/#880/#740/#861/#881/#882 and related M001.1 children move remaining
  portable product/UI authority into Rust.
- #883 rewrites the macOS product shell as the thin AppKit adapter after the Rust
  authorities it consumes exist.
- #884 removes parallel Swift preview/test product models.
- #885 performs final architecture/parity/performance qualification.
- Re-baseline headed UI latency/CPU/RSS/GPU metrics on the final migrated path.
- Issue-pinned architecture-authority discovery/enforcement is tracked separately;
  agents must not rely on remembering ADR numbers as the registry grows.
- Do not mix implementation PRs with further ADR edits.

## Reopen conditions

Reopen only if measured IME, accessibility, Metal lifecycle, FFI, latency,
resource or platform-port evidence shows that this split cannot preserve native
quality or the terminal hot-path constraints, or that a listed Rust-owned
portable behavior cannot be expressed without creating a worse authority model.

Approved by product authority on 2026-09-12. Independent architecture review
remains required before merge.
