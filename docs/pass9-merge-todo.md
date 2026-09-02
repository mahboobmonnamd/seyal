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
- [x] `two_simultaneous_runtime_starters_produce_exactly_one_canonical_endpoint`
  (`crates/seyal-runtime/tests/local_ipc_adversarial.rs`) proves the SPEC-009
  §8.1/§15 "two simultaneous Runtime starters" requirement under a real
  multi-thread race (8 contenders on a shared `Barrier`, `Runtime::new` never
  crosses a thread boundary so the assertion holds without depending on
  `Runtime: Send`): exactly one contender binds the canonical endpoint, every
  loser fails with `RuntimeError::AlreadyRunning` before ever reaching
  directory/socket creation, and the winner's own canonical socket answers
  `ClientHello` with its own `RuntimeId`. Ran 16 consecutive times locally with
  no flake. The prior claim that "singleton races" were covered referred only
  to the pre-existing *sequential* `singleton_uses_live_lock_not_stale_file_metadata`
  test in `macos_runtime.rs`; this closes the true-concurrency gap.
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

## Independent review findings and fixes (2026-09-02)

An independent review of this PR at head `05ed5df` (documented separately)
found the closure checklist above accurately BLOCKED, and additionally found:

1. every "independent" architecture/security/performance disposition recorded
   on Issue #719 was authored by the same account as the implementer, so the
   independent-review gate has never actually been satisfied at any head, let
   alone the exact final one — this is not resolved by the fixes below, which
   were produced by the same reviewing session and therefore cannot themselves
   count as the required independent review either;
2. the two-simultaneous-Runtime-starters race required by SPEC-009 §8.1/§15
   was untested under real concurrency (fixed, see Automated evidence above);
3. "malformed/stale reconnect frames remain bounded" was under-credited: the
   pre-existing `fuzz/fuzz_targets/local_binary_protocol_decode.rs` target
   already feeds truncated/partial payloads into the production
   `FrameHeader::decode`/`decode_message` path for every message type
   (including `Attach`) and runs in the passing `candidate-d-libfuzzer` CI
   job — no new test was needed for this item, only this correction;
4. `BundledRuntimeLauncher.validateCodeSignature()`/`spawn()` (including the
   `POSIX_SPAWN_CLOEXEC_DEFAULT` close-all-fd claim) remain untested by any
   file in the diff — **not fixed this session**: the fix session ran on an
   Apple Silicon host with only Xcode Command Line Tools installed, so no
   Swift/Xcode build, XCTest, or XCUITest could be run or verified here, and
   no Swift source was therefore touched;
5. VoiceOver/dead-key/real-IME validation, the controlled-host Apple-Silicon
   performance cohorts, and the packaging/signing inspection remain
   unaddressed for the same reason — the CI `make bench` run inspected during
   review self-labels every sample `performance_claim=false` because hosted
   macOS runners lack `CAMetalDisplayLink`, and that has not changed;
6. a true "endpoint disappears between the socket-leaf check and `connect()`"
   integration test was scoped but not added: on macOS, `UnixStream::connect`
   to a path whose listener has gone away typically yields `ECONNREFUSED`
   (`ConnectionRefused`), not the `ECONNRESET`/`ENOTCONN` that
   `classify_connect_error` maps to `EndpointDisappeared` — those kinds
   normally arise from a read/write on an already-established connection, not
   from the initial `connect()` syscall on AF_UNIX. Reproducing the exact
   `EndpointDisappeared` branch deterministically needs either a different
   injection point than `connect()` or a documented rationale for why it is
   unreachable there; this needs a decision, not a test, before it can be
   closed;
7. a real different-effective-UID wrong-owner-socket test remains impractical
   without a multi-user CI runner; the existing coverage
   (`connected_peer_requires_the_effective_user` in `discovery.rs`) is a
   synthetic pure-function check only.

The pre-Ready governance violation recorded above is unchanged by this
session: the commits that violated it remain in history rather than being
discarded and re-authored, per the existing rebaseline record.

## Current status

Production fixes from the closure rebaseline are implemented on the PR branch,
including the late security ownership correction that makes client discovery
verification-only and Runtime directory creation singleton-owned. Because that
late correction changes lifecycle/security behavior, all exact-head automated,
native, performance, packaging, and specialist-review evidence must be rerun.

This session closed one automated-evidence gap (item 2 above) and corrected
one over-stated finding (item 3), verified with `cargo test`/`cargo fmt --check`/
`cargo clippy -D warnings` on the pinned `1.98.0` toolchain. It did not, and in
this environment could not, touch native/Swift code, produce native or
performance evidence, or supply the independent review the Definition of Done
requires.

**Merge status: BLOCKED until every unchecked acceptance/review item above is
satisfied with exact-head evidence.**
