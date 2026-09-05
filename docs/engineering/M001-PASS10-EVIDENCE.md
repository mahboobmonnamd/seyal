# M001 Pass 10 — Final Milestone Validation Evidence

**Owning Issue:** #727  
**Parent milestone:** #5  
**Normative protocol:** `docs/engineering/M001-PASS10-VALIDATION.md`  
**Phase 1 ledger:** `docs/evidence/m001-pass10-code-quality-review-ledger.md`

## Freeze

| Field | Value |
|---|---|
| Frozen production head | `e8431f01c797b57d7b6ee6a9be65706f77c7d789` |
| Freeze date (UTC) | 2026-09-05 |
| Gate note | Initial freeze `e8431f0…` failed `make check` / workspace test on `pass7_benchmark::…resettable` due to process-global mark races under feature unification via `seyal-client`. Fix in this Phase 2 branch; **re-freeze required after merge** before final PASS. |
| Phase 1 status | FINDINGS DISPOSITION COMPLETE (#748–#760 closed; #764–#768 parked post-M001). Full file-level ledger completeness still required before Phase 2 authorization. |
| Host (this evidence run) | Mahboob MacBook Pro (2), arm64 |
| macOS | 26.5.2 (25F84) |
| Rust toolchain | rustc/cargo 1.98.0 (88d9e12ae / 797e8a9bc) |
| Build mode | release / debug as noted per command |

Any production commit after the freeze SHA invalidates affected criterion evidence.

## Verdict model

Each criterion: `PASS` | `FAIL` | `INCONCLUSIVE` | `PLATFORM_LIMITED` | `N/A`  
Evidence class for perf/fuzz/presentation claims: `CI` | `controlled-host` | `PLATFORM_LIMITED`

## Criterion ledger

Expanded from `docs/milestones/MILESTONE-001.md` §15 and Pass 10 validation §6.

### 6.1 Architecture and ownership

| Criterion | Verdict | Evidence |
|---|---|---|
| One authoritative VT/state per TerminalExecution | PENDING | |
| PTY owned by TerminalExecution in Runtime | PENDING | |
| BlockTimeline is Runtime/workspace metadata by ExecutionId | PENDING | |
| Blocks own no PTY/VT/grid/renderer | PENDING | |
| Client/Metal state derived/disposable only | PENDING | |
| No GUI VT mirror / second engine | PENDING | |
| No renderer/Block/agent/cloud on PTY→VT hot path | PENDING | |
| OSS has no commercial dependency | PENDING | |

### 6.2 VT, terminal state and terminfo

| Criterion | Verdict | Evidence |
|---|---|---|
| M001 VT unit/property/byte fixtures | PENDING | |
| Reference/conformance corpus + provenance | PENDING | |
| Chunk-boundary parser equivalence | PENDING | |
| Primary + scoped ?1049 alternate screen | PENDING | |
| Malformed/parser-fuzz invariants | PENDING | |
| Real shell TERM=seyal-m001 + bundled terminfo | PENDING | |
| Terminfo capability honesty audit | PENDING | |

### 6.3 PTY / child / Runtime lifecycle

| Criterion | Verdict | Evidence |
|---|---|---|
| Spawn/read/write real PTY | PENDING | |
| Exit vs signal vs EOF/HUP | PENDING | |
| Terminate + deterministic reap | PENDING | |
| Endpoint-first resize | PENDING | |
| Repeated cleanup / resource return | PENDING | |
| TerminationFailed recovery / PrimaryExitPending bound (F-006) | PENDING | |

### 6.4 Candidate-D attachment/projection

| Criterion | Verdict | Evidence |
|---|---|---|
| Binary UDS framing + same-user trust | PENDING | |
| Observer/Controller auth | PENDING | |
| Snapshot/delta atomic commit | PENDING | |
| Slow/dead client isolation | PENDING | |
| Disconnect-during matrix (F-007) | PENDING | |

### 6.5 Metal renderer and native interaction

| Criterion | Verdict | Evidence |
|---|---|---|
| Permanent AppKit/Metal path | PENDING | |
| Damage-driven presentation | PENDING | |
| Input via Runtime authority | PENDING | |
| Hot-path registry includes display/Metal (F-004) | PENDING | |

### 6.6 Minimal Block metadata

| Criterion | Verdict | Evidence |
|---|---|---|
| Runtime-owned Block metadata + anchors | PENDING | |
| Current→Completed ordering | PENDING | |

### 6.7 Pass 9 detach/reconnect/crash continuity

| Criterion | Verdict | Evidence |
|---|---|---|
| Detach without kill | PENDING | |
| GUI crash survival | PENDING | |
| Reattach same ExecutionId | PENDING | |
| Explicit terminate ≠ detach | PENDING | |

### 6.8 Failure / adversarial matrix

| Criterion | Verdict | Evidence |
|---|---|---|
| Malformed VT/protocol/projection | PENDING | |
| Disconnect during backpressure/resize/chunking/finalization | PENDING | |
| No persistent no-progress wake loops | PENDING | |

### 6.9 Fuzz and retained corpus

| Criterion | Verdict | Evidence class | Evidence |
|---|---|---|---|
| Registry/campaign parity (F-008) | PENDING | | |
| Required production surfaces covered or N/A+proof | PENDING | | |
| Milestone §6.9 grade (ci-smoke alone insufficient) | PENDING | | |

### 6.10 Security and privacy

| Criterion | Verdict | Evidence |
|---|---|---|
| Socket ownership/permissions | PENDING | |
| Same-user auth / attachment identity | PENDING | |
| Bounds/version validation | PENDING | |
| Focused M001 threat review recorded | PENDING | |

### 6.11 Performance / memory / resources

| Criterion | Verdict | Evidence class | Evidence |
|---|---|---|---|
| Required measurements recorded vs targets | PENDING | | |
| Pass 9 budget artifacts retained | PENDING | controlled-host | |
| CI benches not mislabeled as headed proof | PENDING | CI | |

### 6.12 Clean-checkout production demo

| Step | Verdict | Evidence |
|---|---|---|
| Clean checkout of freeze SHA | PENDING | |
| bootstrap/build/test/check | PENDING | |
| Runtime + Seyal.app attach demo | PENDING | |
| TERM/terminfo, input, resize, ?1049 | PENDING | |
| Detach/crash/reconnect/terminate | PENDING | |

### 6.13 Non-goals / authority consistency

| Criterion | Verdict | Evidence |
|---|---|---|
| No silent M002/scrollback/tabs/agents/cloud absorption | PENDING | |
| Status docs match freeze head | PENDING | |

## Aggregate gates

| Gate | Verdict | Notes |
|---|---|---|
| `make check` on freeze SHA | FAIL→fix | `e8431f0…` workspace test race on Pass 7 benchmark marks; serialized + clock warmup in this branch |
| Targeted Pass 10 / Pass 9 suites | PENDING | continuing after re-freeze |
| Clean production demo | PENDING | |
| Independent final review | PENDING | |

## Final conclusion

**M001 Pass 10:** `PENDING` — Phase 2 not yet authorized. Freeze candidate `e8431f01…` is provisional; evidence criteria remain PENDING.
