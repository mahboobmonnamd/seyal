# Pass 9 final merge TODO

This checklist is the delivery boundary for Issue #719 and PR #734. It mirrors
the closure rebaseline in Issue #719 comment `5498085185` and keeps production
implementation separate from acceptance evidence. A checked implementation
item does not waive any exact-head evidence or independent-review gate.

## Closure implementation

- [x] Restore legacy `seyal_bridge_connect_first` connect-and-adopt semantics so
  retained callers own a live Pane-local client instead of a disposable pending
  handle.
- [x] Dispose pending lifecycle handles on cancellation, generation replacement,
  failed/stale completion, coordinator deallocation, failed adoption, and window
  closure without adding another Runtime or terminal-state authority.
- [x] Carry one recovery-episode deadline through Swift scheduling and Rust
  discovery, connect, hello, attach, and authoritative initial snapshot reads.
- [x] Preserve typed bundled-Runtime launch failures and transition them to
  deterministic blocked recovery rather than retrying or guessing.
- [x] Verify the canonical socket leaf before/after connect and verify the
  connected AF_UNIX peer UID; reject symlink/non-socket, ownership, writable
  substitution, and peer-identity failures.
- [x] Add lifecycle/deadline/launcher/security regression coverage, including
  stalled startup reads and adversarial socket-leaf/peer cases.
- [x] Keep client discovery verification-only. An absent Runtime directory is
  observed without mutation and still resolves to endpoint-missing; after the
  singleton is acquired, Runtime alone creates/verifies the 0700 directory and
  retains stale-endpoint cleanup/bind ownership.

## Core Pass 9 implementation invariants

- [x] Lifecycle coordinator uses the dedicated lifecycle executor with injected
  clock, scheduler, launcher, attempt, and handle-adoption hooks.
- [x] Recovery uses exactly seven scheduled opportunities at 0, 10, 20, 40,
  80, 160, and 250 ms inside one one-second episode ceiling, with cancellation,
  generation replacement, launch-once behavior, exhaustion, and explicit retry.
- [x] Typed discovery distinguishes endpoint absence/refusal/disappearance,
  security/path failure, no execution, ambiguous executions, Controller busy,
  protocol failure, and successful Runtime/Execution/Attachment identity.
- [x] Runtime helper is built and embedded as a bundle-relative helper; launcher
  validation and launch remain outside terminal I/O/rendering hot paths.
- [x] Reconnect preserves expected Execution identity, creates a fresh
  Attachment, discards disposable connection/display request state, and accepts
  interaction only after a complete authoritative snapshot is reconstructed.
- [x] Runtime attachment/controller cleanup remains authoritative and stale,
  malformed, interrupted, or cross-attachment data fails closed.
- [x] Native recovery reconstructs the existing Metal presentation and restores
  input/focus/geometry state without creating a second VT/grid/renderer authority.
- [x] Recovery diagnostics expose bounded typed state/identity only and do not
  log terminal content or secrets.

## Automated evidence

- [x] Rust unit/integration/fuzz suites cover recovery scheduling, failure
  classification, identity suppression, detach/reattach, snapshot interruption,
  Block continuity, singleton races, and local IPC adversarial cases.
- [x] Swift component tests cover lifecycle state, cancellation/generation
  replacement, stale completion suppression, input admission, focus,
  accessibility state, IME reset, and geometry behavior.
- [x] Runtime production-path tests cover output while detached and naturally
  exited children without creating replacement executions.
- [ ] Freeze the final implementation head and pass exact-head `make bootstrap`,
  `make build`, `make test`, `make check`, and `make bench`.
- [ ] Exact final head passes `repository-policy`, `rust-and-harness-quality`,
  `native-macos-smoke`, Pass 5 production fuzz, native clean-checkout tests, and
  package inspection. Do not infer this from an earlier green commit.

## Required native continuity evidence

- [ ] Retain a fresh-user headed native run covering graceful GUI close/reopen
  and forced GUI death with the same RuntimeId/ExecutionId and fresh AttachmentId.
- [ ] Retain alternate-screen recovery plus post-recovery direct input,
  Control-C, resize, focus, and finite/contained accessibility geometry.
- [ ] Add/retain detached-output evidence produced while no GUI is attached and
  prove the reconstructed surface contains that authoritative output.
- [ ] Add/retain detached-child-exit evidence proving natural exit is drained and
  reported without manufacturing a replacement execution.
- [ ] Retain stale, malformed, and interrupted reconnect/snapshot cases on the
  production path and prove they cannot admit input or stale rendering.
- [ ] Run at least 100 graceful and 100 abrupt recovery cycles and prove
  attachment, Controller, fd, renderer-resource, and RSS counters return to the
  accepted baseline/budget.

## Native accessibility and text-input evidence

- [ ] Real dead-key input validation through the production NSTextInputClient.
- [ ] Real IME commit and cancel/replacement validation through the production
  NSTextInputClient; component-only synthetic marked-text tests are insufficient.
- [ ] VoiceOver focus/recovery validation, including usable-state announcement
  and finite geometry after reconnect.
- [ ] Confirm ordinary keyboard focus and focus restoration remain correct after
  both graceful and abrupt recovery.

## Performance and resource evidence

- [ ] Collect five independent exact-head Apple-silicon 100-cycle cohorts after
  warm-up under the accepted controlled-host preconditions.
- [ ] Validate SPEC-009 reconnect/cleanup/RSS/fd/Controller/attachment/renderer
  resource and detached-idle CPU budgets against retained raw artifacts.
- [ ] Compare the paired Pass 8 baseline and explain every >5% movement; reject
  any unexplained >10% regression.
- [ ] Validate the retained exact-head artifact with
  `python3 scripts/check-pass9-production-budget.py --expected-head <head> <artifact>`.
  Validator self-tests and PR #726 calibration are not production evidence.

## Packaging, governance, and independent review

- [ ] Retain exact-head package inspection proving bundled-helper location,
  identifier, entitlements, bundle seal/signing/Team identity, environment/fd
  closure, direct no-shell launch, and Release rejection of ad-hoc trust.
- [x] Record the pre-Ready production-start ordering violation: production work
  began at `eada76640776779ad3f8bd65a0cb8199d91f396f` before Issue #719 was
  explicitly Ready.
- [x] Rebaseline PR #734 against accepted master, SPEC-009, and authorized
  calibration authority; no pre-Ready implementation is grandfathered.
- [ ] Freeze one exact final head after all fixes/evidence are committed.
- [ ] Obtain fresh independent implementation, architecture, security,
  performance, and accessibility reviews against that exact head with no
  unresolved blockers.
- [ ] Update Issue #719 with the final evidence matrix and truthful Done state.
- [ ] Update PR #734 body to the final scope/evidence and correct owning-Issue
  relationship. Use a closing relationship only if every Done gate is proven.
- [ ] Ask for explicit user confirmation before merging PR #734.

## Current status

Production fixes from the closure rebaseline are implemented on the PR branch,
including the late security ownership correction that makes client discovery
verification-only and Runtime directory creation singleton-owned. Because that
late correction changes lifecycle/security behavior, all exact-head automated,
native, performance, packaging, and specialist-review evidence must be rerun.

**Merge status: BLOCKED until every unchecked acceptance/review item above is
satisfied with exact-head evidence.**
