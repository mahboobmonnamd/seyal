# Pass 9 final merge TODO

This checklist is the delivery boundary for Issue #719 and PR #734. Every
item must have exact-head evidence before the PR is marked ready.

## Implementation

- [ ] Lifecycle coordinator runs discovery/hello/attach on a dedicated serial
  lifecycle queue with injected clock, scheduler, launcher, and attempt hooks.
- [ ] Recovery has exactly seven attempts at 0, 10, 20, 40, 80, 160, and
  250 ms, one-second episode ceiling, cancellation, generation replacement,
  launch-once behavior, and explicit retry after exhaustion.
- [ ] Typed discovery classifies endpoint absence/refusal, security failure,
  Runtime mismatch, no execution, ambiguous executions, Controller busy,
  protocol failure, and success with Runtime/Execution/Attachment identity.
- [ ] Client discovery is verification-only; only Runtime owns socket cleanup,
  singleton binding, stale endpoint removal, and repair.
- [ ] Runtime helper is built, embedded, signed with the app, launched only
  from the trusted bundle path, and never terminated by GUI teardown.
- [ ] Reconnect preserves the expected ExecutionId, discards all disposable
  socket/request/display/controller state, and requires a complete authoritative
  snapshot before input or rendering becomes usable.
- [ ] Runtime cleanup revokes dead attachments and Controller exactly once;
  stale identities and malformed/interrupted frames are rejected.
- [ ] Native surface restores Metal, focus, accessibility, IME, geometry, and
  input admission only after the reconstructed surface is usable.
- [ ] Reconnect logs and diagnostics contain no terminal content or secrets.

## Evidence gates

- [x] Rust unit/integration/fuzz suites cover schedules, failures, identity
  suppression, detach/reattach, snapshot interruption, Block continuity, and
  singleton races.
- [x] Swift component tests cover lifecycle states, cancellation, stale-frame
  suppression, input admission, focus, accessibility, IME reset, and geometry.
- [ ] Native headed XCTest/XCUIAutomation proves the complete required matrix.
  The current headed fixture proves graceful/forced relaunch, same
  Runtime/Execution identity, fresh attachment, input, Control-C, resize,
  focus/geometry, and alternate-screen recovery. It does **not** yet prove
  dead-key input, a real IME commit/cancel flow, VoiceOver focus/geometry,
  detached output, or detached-child exit. Do not mark those cases complete
  from component tests or a synthetic fixture.
- [ ] At least 100 graceful and 100 abrupt cycles return attachment,
  Controller, fd, renderer, and RSS counters to baseline.
- [ ] Five independent 100-cycle performance cohorts after warm-up validate
  SPEC-009 timestamps, cleanup/reconnect/RSS/fd/CPU budgets, and paired Pass 8
  regression movement with no unexplained >5% movement or >10% breach.
  Validate the supplied exact-head measurement artifact with
  `python3 scripts/check-pass9-production-budget.py <evidence.json>`; the
  validator does not generate measurements or turn a self-test into hardware
  evidence. A bounded client allocator delta of at most 4 KiB is accepted only
  when the artifact classifies it as fixed harness-owned capacity, never as
  production retention.
- [ ] Exact final head passes `make bootstrap`, `make build`, `make test`,
  `make check`, `make bench`, native clean-checkout build/tests, and package
  inspection.
- [ ] Independent architecture, security, performance, accessibility, and
  implementation reviews have no unresolved blockers.
- [ ] Issue #719 contains the final evidence matrix and truthful Done state;
  PR #734 has the exact owning-Issue relationship and is no longer Draft.

## Governance and review disposition

- [ ] Record the pre-Ready production-start ordering violation. Production
  work began at `eada76640776779ad3f8bd65a0cb8199d91f396f` before Issue #719
  was explicitly marked Ready. This is an acceptance blocker, even if the
  implementation is later corrected.
- [ ] Re-baseline PR #734 against the accepted dependency/calibration state
  after recording that ordering violation. Freeze the resulting exact head
  and obtain fresh independent implementation, architecture, security,
  performance, and accessibility reviews against that head.
- [ ] Record the corrective review decision and evidence links in Issue #719;
  do not infer approval from passing CI or from PR #726 calibration evidence.

## Current known status

The bundled helper, typed identities, bounded recovery state, explicit retry,
reconnect reconstruction, and Rust/runtime suites are implemented. The typed
error and lifecycle-queue fixes require fresh exact-head review. `make test` is
not yet clean because the standalone live renderer self-test collides with an
already-running canonical Runtime in this host. Deterministic headed
IME/dead-key input-source behavior, VoiceOver recovery, detached-output and
child-exit workloads, physical stress/resource cohorts, release-signing
evidence, governance re-baselining, and independent final reviews remain open.

**Merge status: BLOCKED.** This document is a checklist, not evidence; a
passing validator or CI workflow cannot check an item without the retained
artifact and exact-head provenance required by the specification.
