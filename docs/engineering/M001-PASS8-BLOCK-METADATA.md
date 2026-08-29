# M001 Pass 8 — Block Metadata Completion Evidence

**Status:** Technical completion evidence; independent exact-head review still required  
**Owning issue:** #715  
**Implementation PR:** #721  
**Repository:** Seyal OSS only

This document retains the engineering evidence for M001 Pass 8 minimal Block metadata. It is not an independent review verdict and does not authorize merge by itself.

## A. Implementation authority

- Current master incorporated before final completion validation: `0cadd0921abcfdfaabf866870e32a3b6b521be94`.
- Pass 8 branch was explicitly merged with that master lineage before final completion validation; current master is an ancestor of the completion branch.
- Accepted SPEC-007 refinement authority: `9dc3340f16084193f6575c88752b8a9aae02003c`.
- Pass 7 controlled benchmark executable baseline retained for comparison: `149a8f205848493a4f4d63e1f47005f6987bcd7a`.
- Fully measured same-production-code Pass 8 head before the master-lineage/spec-document-only updates: `18541957ebc2779b8aa9a1fde42e0f76c0c02f0b`.
- Master-lineage merge commit: `7ce7c22f00c23b58d2bba65c0aba8f8326999aee`.
- SPEC-007 PTY/metadata population reconciliation head: `1f7b8f21d35471a62c15e9116a238e2ab2aa83ed`.

The production-code delta between `18541957…` and `1f7b8f21…` is zero: the intervening changes incorporate the current master documentation lineage and reconcile the accepted Pass 8 validation wording. Final exact-head CI after this retained evidence commit remains mandatory and is recorded in the PR/issue once complete.

Pass 8 keeps the accepted single-authority architecture:

```text
Runtime
  → Workspace / execution composition
  → bounded BlockTimeline metadata

TerminalExecution
  → PTY
  → primary child lifecycle
  → canonical TerminalState

TerminalState
  → VT/parser modes
  → primary/alternate screen
  → logical LineId identity
  → damage
```

No Pass 8 Block owns a PTY, child, VT state, terminal grid, alternate grid, transcript, terminal output, renderer state or duplicate execution authority.

## B. Functional validation

The retained deterministic and integration matrix proves:

- nonzero opaque 128-bit `BlockId` identity and 128-bit wire round-trip;
- Workspace ownership and exact `ExecutionId` association;
- one coarse `TerminalActivity` metadata record per successfully admitted execution;
- immutable initial primary-screen `LineId` anchor;
- anchor stability across primary scroll, resize, display resync, detach/reattach and alternate-screen/TUI transitions;
- no command/Enter/output amplification into additional Pass 8 records;
- monotonic `Current` revision 1 → `Completed` revision 2 only after accepted final PTY drain;
- final per-connection ordering of final display state → `BlockState::Completed` → `Lifecycle::Finalized` for negotiated Block-capable attachments;
- existing final-display-before-Finalized ordering for non-Block-capable/no-Block attachments;
- execution-registry retirement removes the M001 Block record in the same bounded finalization lifecycle;
- reconnect/reattach rebuilds disposable metadata from Runtime authority rather than client persistence.

The protocol remains deliberately independent of merged Pass 7.1 command Blocks:

- Pass 7.1 capability bit 4 / client→Runtime type 20 remains separate;
- Pass 8 capability bit 5 / Runtime→client type 26 is separately negotiable;
- Pass 8 `BlockState` payload is exactly 56 bytes.

## C. Failure, quarantine and fail-closed behavior

Production failure-injection and deterministic tests cover:

- Block allocation/admission failure without failing otherwise valid terminal execution;
- completion mutation failure;
- completion encode failure;
- connection-local output/admission pressure;
- malformed, reserved, stale, duplicate and conflicting revisions;
- anchor mutation, BlockId swap and state regression;
- multi-client isolation;
- disposable `BlockCache` invalidation on disconnect;
- semantic-conflict quarantine followed by raw-terminal fallback without capability bit 5;
- no timer/retry/spin loop for failed Block metadata;
- failure-closed finalization so a Block-capable connection cannot receive `Lifecycle::Finalized` while retaining stale `Current` metadata.

Pass 8 metadata failure never changes PTY/VT/terminal execution correctness and never becomes a synchronous dependency of terminal progress.

## D. Production fuzz evidence

Production fuzz coverage includes:

- fixed-layout Pass 8 `BlockState` decoder;
- Candidate-D control/binary protocol decoder;
- display decoder/cache state machine;
- attachment/reconnect/resync state machine.

Same-code exact production fuzz evidence on `18541957…` passed, and the post-master/spec head `1f7b8f21…` production fuzz run `33251593358` also completed successfully, including both Pass 8 BlockState and reconnect/resync targets.

Final exact-head fuzz after this evidence commit is mandatory and is recorded in PR #721 when complete.

## E. Native application path

The validated native path is the real production topology:

```text
Seyal.app
→ RustDisplayBridge / C ABI
→ seyal-client
→ separate seyal-runtime process
→ Runtime
→ TerminalExecution
→ real PTY-backed shell
```

The native test does not create a second Runtime or PTY authority inside AppKit. It proves real Runtime-owned metadata reaches the Swift presentation seam.

The measured same-code head `18541957…` passed Foundation Quality run `33248807041`, including native macOS job `99090789596` with:

- native build;
- macOS `make check`;
- real Runtime/native smoke;
- XCTest;
- XCUIAutomation;
- PTY-backed validation;
- native benchmark gate.

The native Pass 8 metadata self-test uses a bounded readiness window rather than assuming the external Runtime socket is synchronously ready at process launch.

The permanent macOS CI now performs a full-history checkout (`fetch-depth: 0`) before native `make check`, because the UI-policy validator legitimately resolves `origin/master` when determining the PR diff.

## F. Resource and population evidence

The PTY lifecycle and Block-record population gates are intentionally separate.

### Real PTY-backed lifecycle

- At least 10 concurrent real PTY-backed Runtime executions are mandatory.
- The validation probes toward 50.
- Above the floor of 10, early stop is accepted only for the recognized macOS PTY allocator failure `ENXIO` / `Device not configured`.
- Any unrelated admission error fails the gate.
- Fewer than 10 real admitted PTY executions fails the gate.
- There is no silent skip.

### Production BlockTimeline capacity

Exactly 512 simultaneous M001 Block records are admitted against the production `BlockTimeline`; this is not interpreted as a requirement for 512 operating-system PTYs.

Measured same-production-code resource evidence:

```text
live_block_records = 512
attributable_runtime_rss = 96 KiB
gate = 1 MiB
records_after_retirement = 0
unexpected_idle_thread_growth = 0
unexpected_idle_fd_growth = 0
persistent_retry_spin_wakeup = false
```

The Runtime `BlockTimeline` has a hard 512-record bound and rejects further admission with a bounded `CapacityExceeded` result. Retirement releases capacity and a later live record can then be admitted.

## G. Performance evidence

Measured Pass 8 fixed-size encode/decode/client-apply latency on the same-production-code measured head:

```text
p50 = 0.000 µs
p95 = 0.042 µs
p99 = 0.042 µs
gate = 250 µs
```

This is far below the Pass 8 metadata gate. It is not presented as end-to-end user-visible terminal latency.

The benchmark path uses production protocol/client state logic and production `BlockTimeline` population; it does not substitute a separate unbounded or test-only Block store.

## H. Pass 7 regression attribution

The controlled Pass 8 resize attribution uses two real executions under one Runtime/reactor:

- one controller with Pass 8 capability disabled;
- one controller with Pass 8 enabled;
- interleaved A/B ordering;
- seven cohorts × 512 samples per mode;
- paired cohort comparison.

Measured same-code evidence:

```text
pass8_disabled_median_p99 = 14.208 µs
pass8_enabled_median_p99  = 14.625 µs
paired_delta               = +2.93%
```

The observed movement is below the 5% explanation threshold and below the 10% blocking threshold.

A separate Pass 7 resize 120×40 result of `14.292 µs` came from a different host/run and is retained only as historical context; it is not used to manufacture a same-host blocking or improvement claim.

## I. Security, limitations and cleanup

### Security/privacy

The Pass 8 wire record carries only:

- `ExecutionId`;
- `BlockId`;
- revision;
- primary logical `LineId` anchor;
- kind/state;
- zeroed reserved fields.

It carries no:

- command or prompt text;
- shell history;
- cwd;
- environment;
- credentials/secrets;
- PTY bytes;
- terminal cells;
- copied transcript;
- renderer coordinates.

The decoder is fixed-size and exact-length, validates nonzero IDs/revision/LineId, validates enums/reserved fields and does not create attacker-controlled unbounded allocation. Client Block state is bounded and Runtime Block metadata is hard-capped.

### Cleanup audit

No temporary Pass 8 repair workflow, one-off debug workflow, temporary attribution script or alternate production terminal path is part of the retained solution. Test/fuzz/benchmark-only features remain intentional validation surfaces rather than production authorities.

### Explicit limitations

Pass 8 intentionally does not claim:

- Runtime/reboot restoration of a live PTY;
- durable disk Block-history restoration;
- trusted-shell per-command truth beyond the separately merged Pass 7.1 command-Block seam;
- command scraping/prompt inference;
- transcript virtualization;
- multiline composer completion;
- remote/commercial Block transport.

These limitations do not weaken the M001 Pass 8 contract.

## Completion rule

Technical completion requires the final repository-content head containing this evidence record to pass all required exact-head Foundation, production fuzz and native macOS gates, including `make check`, `make bench`, XCTest/XCUIAutomation, native smoke, PTY validation and Pass 8 production fuzz.

After those gates are green, the remaining acceptance item is a **separate independent implementation review on that frozen exact head**. This document and the implementation author must not self-mark that independent review complete.
