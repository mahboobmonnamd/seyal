# M001 Pass 10 — Final Milestone Validation Evidence

**Owning Issue:** #727  
**Parent milestone:** #5  
**Normative protocol:** `docs/engineering/M001-PASS10-VALIDATION.md`  
**Phase 1 ledger:** `docs/evidence/m001-pass10-code-quality-review-ledger.md`

## Freeze

| Field | Value |
|---|---|
| Frozen production head | `3f7b2d926dcab888e4dadc480033c1d137fd5ad7` |
| Freeze date (UTC) | 2026-09-05 |
| Gate note | Re-frozen on `master` after squash-merge of #775 (Pass 7 mark-test race, fuzz lockfile, PTY env drain/reap, Phase 1 honesty). Prior provisional freeze `e8431f0…` is superseded. |
| Phase 1 status | FINDINGS DISPOSITION COMPLETE (#748–#760 closed; #764–#768 parked post-M001). File inventory bound; Phase 2 criterion evidence in progress on this freeze. |
| Aggregate `make check` | PASS on freeze (`EXIT:0`, log `/tmp/pass10-evidence-3f7b2d9/make-check-final.log`) |
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
| One authoritative VT/state per TerminalExecution | PASS | `cargo test -p seyal-exec --test macos_pty --locked` EXIT:0; single-state assertion in PTY→VT path. Log `/tmp/pass10-evidence-3f7b2d9/macos_pty.log` |
| PTY owned by TerminalExecution in Runtime | PASS | `macos_pty` + `macos_runtime` + architecture layering (`scripts/check-layering.py` EXIT:0) |
| BlockTimeline is Runtime/workspace metadata by ExecutionId | PASS | `pass8_blocks` EXIT:0; BlockTimeline keyed by ExecutionId in Runtime |
| Blocks own no PTY/VT/grid/renderer | PASS | `pass8_blocks` + cohesion review inventory; Blocks are metadata-only |
| Client/Metal state derived/disposable only | PASS | Candidate-D client tests + Metal self-tests via `make check` macOS scaffold |
| No GUI VT mirror / second engine | PASS | `scripts/check-hot-path.py` EXIT:0; macOS skeleton/UI smoke forbids second engine |
| No renderer/Block/agent/cloud on PTY→VT hot path | PASS | hot-path registry check EXIT:0 (display/Metal registered as presentation, not PTY→VT authority) |
| OSS has no commercial dependency | PASS | `scripts/check-layering.py` EXIT:0; workspace builds without commercial crates |

### 6.2 VT, terminal state and terminfo

| Criterion | Verdict | Evidence |
|---|---|---|
| M001 VT unit/property/byte fixtures | PASS | `cargo test -p seyal-terminal --test m001_vt --locked` EXIT:0 (`/tmp/pass10-evidence-3f7b2d9/m001_vt.log`) |
| Reference/conformance corpus + provenance | PASS | `cargo test -p seyal-terminal --test fixture_corpus --locked` EXIT:0 |
| Chunk-boundary parser equivalence | PASS | `m001_vt::printable_utf8_survives_arbitrary_chunking` EXIT:0 |
| Primary + scoped ?1049 alternate screen | PASS | `m001_vt::alternate_screen_preserves_primary_and_is_discarded_on_leave` EXIT:0 |
| Malformed/parser-fuzz invariants | PASS | `cargo test -p seyal-terminal --test fuzz_smoke --locked` EXIT:0 + registry smoke; campaign evidence pending §6.9 |
| Real shell TERM=seyal-m001 + bundled terminfo | PASS | `macos_runtime` seyal_term filter + `macos_terminfo_clean` EXIT:0 |
| Terminfo capability honesty audit | PASS | Manual audit of `resources/terminfo/seyal-m001.src` vs MILESTONE-001 SUPPORTED table: only am/cols/lines/colors#256/cursor/erase/SGR/256-color setaf/setab/DECTCEM/?1049; no mouse/bracketed-paste/OSC/title/sixel/xterm-256color alias. Resolution smoke `macos_terminfo_clean` EXIT:0 |

### 6.3 PTY / child / Runtime lifecycle

| Criterion | Verdict | Evidence |
|---|---|---|
| Spawn/read/write real PTY | PASS | `macos_pty` + `macos_runtime` EXIT:0 |
| Exit vs signal vs EOF/HUP | PASS | `runtime_adversarial` + `macos_failure_contracts` EXIT:0 |
| Terminate + deterministic reap | PASS | `macos_runtime` / `macos_failure_contracts` EXIT:0 |
| Endpoint-first resize | PASS | `seyal-exec` macos_pty resize kernel-visible test EXIT:0; runtime rejects resize after termination begins EXIT:0 |
| Repeated cleanup / resource return | PASS | `repeated_create_and_controlled_terminate_returns_registry_and_budget_to_zero` EXIT:0 |
| TerminationFailed recovery / PrimaryExitPending bound (F-006) | PASS | `runtime_adversarial` with `test-fault-injection`: primary_exit_pending + termination_failed recovery EXIT:0 |

### 6.4 Candidate-D attachment/projection

| Criterion | Verdict | Evidence |
|---|---|---|
| Binary UDS framing + same-user trust | PASS | `local_ipc_protocol` EXIT:0 |
| Observer/Controller auth | PASS | `local_ipc_protocol` + `local_ipc_adversarial` EXIT:0 |
| Snapshot/delta atomic commit | PASS | `final_projection` + Candidate-D live tests EXIT:0 |
| Slow/dead client isolation | PASS | `pass8_stalled_client` / disconnect-during matrix EXIT:0 |
| Disconnect-during matrix (F-007) | PASS | `cargo test -p seyal-runtime --test pass10_disconnect_during --locked` 5/5 EXIT:0 |

### 6.5 Metal renderer and native interaction

| Criterion | Verdict | Evidence |
|---|---|---|
| Permanent AppKit/Metal path | PASS | `Seyal --renderer-self-test` EXIT:0 + `make check` macOS Metal scaffold |
| Damage-driven presentation | PASS | renderer self-test + Candidate-D live Metal acceptance in `make check` |
| Input via Runtime authority | PASS | Pass 7 interactive / production shell command through external Runtime path in macOS tests |
| Hot-path registry includes display/Metal (F-004) | PASS | `scripts/check-hot-path.py` EXIT:0 |

### 6.6 Minimal Block metadata

| Criterion | Verdict | Evidence |
|---|---|---|
| Runtime-owned Block metadata + anchors | PASS | `pass8_blocks` EXIT:0 |
| Current→Completed ordering | PASS | `block_identity_anchor_completion_order_and_retirement_follow_spec007` EXIT:0 |

### 6.7 Pass 9 detach/reconnect/crash continuity

| Criterion | Verdict | Evidence |
|---|---|---|
| Detach without kill | PASS | Pass 9 merge-acceptance on harness-fixed head: soak result=ok; `client_rss_delta_kib` graceful=224 abrupt=512 ≤768. Artifact `/tmp/pass10-evidence-3f7b2d9/pass9-rss-fix/pass9-merge-acceptance-3f7b2d926dca.json`. Prior false-FAIL was baseline-before-warmup (fixed to match release-qual). Budget unchanged. |
| GUI crash survival | PASS | merge-acceptance abrupt_socket_loss cohort exact-return + continuity on freeze path (artifact above) |
| Reattach same ExecutionId | PASS | merge-acceptance continuity fields + Pass 8 resync/reattach EXIT:0 |
| Explicit terminate ≠ detach | PASS | Pass 9 merge-acceptance + runtime terminate contracts; detach leaves execution live |

### 6.8 Failure / adversarial matrix

| Criterion | Verdict | Evidence |
|---|---|---|
| Malformed VT/protocol/projection | PASS | `local_ipc_adversarial` + `ffi_misuse_macos` + VT fuzz_smoke EXIT:0 |
| Disconnect during backpressure/resize/chunking/finalization | PASS | `pass10_disconnect_during` 5/5 EXIT:0 |
| No persistent no-progress wake loops | PASS | `runtime_adversarial` + failure contracts EXIT:0; no spin after persistent failure |

### 6.9 Fuzz and retained corpus

| Criterion | Verdict | Evidence class | Evidence |
|---|---|---|---|
| Registry/campaign parity (F-008) | PENDING | | |
| Required production surfaces covered or N/A+proof | PENDING | | |
| Milestone §6.9 grade (ci-smoke alone insufficient) | PENDING | | |

### 6.10 Security and privacy

| Criterion | Verdict | Evidence |
|---|---|---|
| Socket ownership/permissions | PASS | `local_ipc_protocol` + adversarial suites EXIT:0; same-user UDS trust contracts |
| Same-user auth / attachment identity | PASS | `local_ipc_protocol` / adversarial Observer-Controller auth EXIT:0 |
| Bounds/version validation | PASS | `local_ipc_adversarial` + `local_ipc_ctrunc` + FFI misuse EXIT:0 |
| Focused M001 threat review recorded | PASS | `docs/evidence/m001-pass10-security-review.md` (Pass 9 #745 not used as sole proof) |

### 6.11 Performance / memory / resources

| Criterion | Verdict | Evidence class | Evidence |
|---|---|---|---|
| Required measurements recorded vs targets | PENDING | | |
| Pass 9 budget artifacts retained | INCONCLUSIVE | controlled-host | Retained `docs/evidence/pass9-release-qualification-*.json` are older SHAs; merge-acceptance PASS on harness-fixed head (224/512≤768). Five-cohort release-qual must re-run on final freeze | |
| CI benches not mislabeled as headed proof | PASS | CI | `make check` / Foundation benches remain class `CI`; headed proof requires `SEYAL_REQUIRE_DISPLAY_LINK_BENCHMARK=1` controlled-host (documented in validation §5) | |

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
| No silent M002/scrollback/tabs/agents/cloud absorption | PASS | Layering check EXIT:0; milestone non-goals unchanged; no commercial crates in OSS workspace; UI preview tabs are non-acceptance surfaces |
| Status docs match freeze head | PENDING | |

## Aggregate gates

| Gate | Verdict | Notes |
|---|---|---|
| `make check` on freeze SHA | PASS | `make check` EXIT:0 on `3f7b2d926dcab888e4dadc480033c1d137fd5ad7` (`/tmp/pass10-evidence-3f7b2d9/make-check-final.log`) |
| Targeted Pass 10 / Pass 9 suites | IN PROGRESS | Pass 9 merge-acceptance PASS after RSS harness fix (224/512≤768); §6.9 campaigns in progress (2/8 EXIT:0); lifecycle/F-006/Block order green |
| Clean production demo | PENDING | |
| Independent final review | PENDING | |

## Harness note (Pass 9 merge-acceptance RSS)

`Pass9MergeAcceptance` previously sampled client RSS baseline before warmups, charging Metal/IMK cold caches into `client_rss_delta` (observed 6480–7184 KiB vs 768 soft gate). Aligned with `Pass9ReleaseQualification`: cold-start settle + warmups before baseline. Soft gate **768 KiB unchanged**. Post-fix evidence: graceful=224, abrupt=512. Re-freeze required after this harness fix merges.

## Final conclusion

**M001 Pass 10:** `IN PROGRESS` — re-frozen at `3f7b2d926dcab888e4dadc480033c1d137fd5ad7`. Partial criterion evidence recorded; §6.9 campaigns + Pass 9 requal + clean demo + independent review still open.
