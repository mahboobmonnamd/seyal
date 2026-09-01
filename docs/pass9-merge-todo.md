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

- [ ] Rust unit/integration/fuzz suites cover schedules, failures, identity
  suppression, detach/reattach, snapshot interruption, Block continuity, and
  singleton races.
- [ ] Swift component tests cover lifecycle states, cancellation, stale-frame
  suppression, input admission, focus, accessibility, IME reset, and geometry.
- [ ] Native headed XCTest/XCUIAutomation proves real shell close/reopen,
  forced GUI loss, same Runtime/Execution identity, input, Control-C, resize,
  dead key, IME commit, accessibility focus/geometry, VoiceOver smoke, and
  alternate-screen recovery.
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

## Current known status

The bundled helper, typed identities, bounded recovery state, explicit retry,
and several Rust/runtime tests are implemented. Headed native recovery,
dedicated lifecycle queue integration, stress/resource cohorts, and independent
final reviews remain open until evidenced.
