# M001 Pass 10 — Final Milestone Validation Evidence

**Owning Issue:** #727  
**Parent milestone:** #5  
**Normative protocol:** `docs/engineering/M001-PASS10-VALIDATION.md`  
**Phase 1 ledger:** `docs/evidence/m001-pass10-code-quality-review-ledger.md`

## Freeze

| Field | Value |
|---|---|
| **Final frozen production head** | `c536c5454583f6a036910e145fe1187446319630` |
| Last production behavior change | `a012ab0b71e74a18c37becacb2bfc1c505f1248c` (#789 history Metal atlas deferral) |
| Honesty tip | `680805277637940f78310398a1bcf1d15fb8e9c8` (#790) |
| Baseline tip | `c536c5454583f6a036910e145fe1187446319630` (#791) |
| Prior historical freeze | `d845c6ddbe86f20183186f1aa69f2293aa8356ba` (#776) — superseded for final closeout |
| Freeze date (UTC) | 2026-09-05 |
| Harness tip | `e2b76024de2c85b3e9adb6dd5dcadb7b40881079` (#778 UITest helper packaging) |
| Gate note | Machine RSS gate remains `CLIENT_RSS_KIB = 1536` (#784). Engineering Quality Baseline landed (#788/#791). |
| Phase 1 status | FINDINGS DISPOSITION COMPLETE (#748–#760 closed; #764–#768 closed post-M001). Residuals #784/#786/#787/#788 closed. |
| Clean-checkout demo | PASS — all of bootstrap/build/test/check/bench EXIT:0 on exact freeze; see `docs/evidence/pass10-final-freeze-clean-demo-c536c54.md` |
| Host (this evidence run) | Mahboob MacBook Pro (2), arm64 |
| macOS | 26.5.2 (25F84) |
| Rust toolchain | rustc/cargo 1.98.0 |
| Build mode | release / debug as noted per command |

Any production commit after this freeze SHA invalidates affected criterion evidence.

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
| Detach without kill | PASS | Pass 9 merge-acceptance on harness-fixed head: soak result=ok; `client_rss_delta_kib` graceful=224 abrupt=512 ≤ `CLIENT_RSS_KIB` 1536. Artifact `/tmp/pass10-evidence-3f7b2d9/pass9-rss-fix/pass9-merge-acceptance-3f7b2d926dca.json`. Machine budget unchanged by Pass 10. |
| Forced GUI exit survival (SIGKILL/forced terminate of GUI process) | PASS | Headed XCUI `testPass9ProductionRecoverySurvivesGracefulAndForcedGUIExit` on harness tip — graceful + **forced GUI exit** with Runtime continuity (`UITEST_EXIT:0`; `/tmp/pass10-evidence-3f7b2d9/section-6.12-headed-uitest-unblocked.log`). **Not** proven by `abrupt_socket_loss` soak alone (that mode is socket-loss, not GUI-process death). |
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
| Registry/campaign parity (F-008) | PASS | controlled-host | 8/8 production fuzz targets `EXIT:0` @600s (`/tmp/pass10-evidence-3f7b2d9/fuzz/campaigns/`; `ALL_EXIT:0`). Campaigns run on `3f7b2d9` worktree; `3f7b2d9..d845c6d` delta is Pass9 RSS harness + `macos_environment` test only — fuzz crates/targets unchanged |
| Required production surfaces covered or N/A+proof | PASS | controlled-host | Targets: `vt_byte_parser`, `parser_state_mutation`, `local_binary_protocol_decode`, `display_decode`, `display_state_machine`, `reconnect_resync_state_machine`, `pass7_protocol_decode`, `pass8_block_state_decode` |
| Milestone §6.9 grade (ci-smoke alone insufficient) | PASS | controlled-host | Controlled 600s campaigns (not ci-smoke); summary `/tmp/pass10-evidence-3f7b2d9/fuzz/campaigns/summary.txt` |

### 6.10 Security and privacy

| Criterion | Verdict | Evidence |
|---|---|---|
| Socket ownership/permissions | PASS | `local_ipc_protocol` + adversarial suites EXIT:0; same-user UDS trust contracts |
| Same-user auth / attachment identity | PASS | `local_ipc_protocol` / adversarial Observer-Controller auth EXIT:0 |
| Bounds/version validation | PASS | `local_ipc_adversarial` + `local_ipc_ctrunc` + FFI misuse EXIT:0 |
| Focused M001 threat review recorded | PASS | `docs/evidence/m001-pass10-security-review.md` (Pass 9 review artifact not used as sole proof) |

### 6.11 Performance / memory / resources

| Criterion | Verdict | Evidence class | Evidence |
|---|---|---|---|
| Required measurements recorded vs targets (Pass 9 RSS / release-qual) | PASS | controlled-host | Pass9 merge-acceptance graceful=224 / abrupt=512 ≤1536; release-qual artifact `docs/evidence/pass9-release-qualification-d845c6ddbe86.json` accepted by production budget validator (`ACCEPTED_ARTIFACT_BUDGET:PASS`; `CLIENT_RSS_KIB=1536`) |
| Pass 9 budget artifacts retained | PASS | controlled-host | Exact prior-freeze artifact retained: `docs/evidence/pass9-release-qualification-d845c6ddbe86.json`; validator PASS in `/tmp/pass10-evidence-3f7b2d9/pass9-budget-recheck.log` |
| Headed presentation-proxy (`SEYAL_REQUIRE_DISPLAY_LINK_BENCHMARK=1`) | PASS | controlled-host | On production tip `a012ab0` with ad-hoc diagnostic codesign (`SEYAL_CODESIGN_IDENTITY=-`): `make bench` **EXIT:0**; Pass-6 `display_link_samples=120/120`; `committed_generation_to_presented_frame_proxy` recorded (one-shot CAMetalDisplayLink→command commit). Summary: `docs/evidence/pass10-787-headed-display-link-summary-a012ab0.txt`; exit file `docs/evidence/pass10-787-headed-display-link-bench-a012ab0.exit`. Full log: `/tmp/pass10-787-evidence/headed-display-link-bench.log` |
| CI benches not mislabeled as headed proof | PASS | CI | Foundation `native-macos-smoke` uses `SEYAL_REQUIRE_DISPLAY_LINK_BENCHMARK=0` — class `CI` only; **not** substituted for the controlled-host headed row above |

### 6.12 Clean-checkout production demo

| Step | Verdict | Evidence |
|---|---|---|
| Clean checkout of validated head | PASS | Exact freeze `c536c54…`; clean demo `docs/evidence/pass10-final-freeze-clean-demo-c536c54.md` |
| bootstrap/build/check | PASS | `BOOTSTRAP:0` `BUILD:0` `CHECK:0` on freeze |
| `make test` (XCTest + XCUI) | PASS | `TEST:0` — full XCTest + XCUIAutomation including forced-GUI-exit recovery on freeze tip |
| `make bench` (diagnostic packaging) | PASS | `BENCH:0` with `SEYAL_CODESIGN_IDENTITY=-` and headed DisplayLink (`display_link_samples=120`) |
| Runtime + Seyal.app attach demo | PASS | Production XCUI through external Runtime + Metal; `--renderer-self-test` EXIT:0 |
| TERM/terminfo, input, resize, ?1049 | PASS | Prior §6.12 alt/terminfo + freeze recovery UITest path |
| Detach/forced-GUI-exit/reconnect/terminate | PASS | `testPass9ProductionRecoverySurvivesGracefulAndForcedGUIExit` PASS on freeze tip |

### 6.13 Non-goals / authority consistency

| Criterion | Verdict | Evidence |
|---|---|---|
| No silent M002/scrollback/tabs/agents/cloud absorption | PASS | Layering check EXIT:0; milestone non-goals unchanged; no commercial crates in OSS workspace; UI preview tabs are non-acceptance surfaces |
| Status docs match freeze head | PASS | Evidence tip records production freeze `d845c6ddbe86…` and harness tip `#778` / `e2b7602…`; machine RSS gate `CLIENT_RSS_KIB=1536` unchanged by Pass 10 (#784 corrects prior false 768 claims) |

## Aggregate gates

| Gate | Verdict | Notes |
|---|---|---|
| `make check` on final freeze | PASS | `CHECK:0` on `c536c54…` (`/tmp/pass10-final-freeze/check.exit`) |
| Targeted Pass 10 / Pass 9 suites | PASS | Pass9 budget validator PASS; headed DisplayLink 120/120 on freeze; XCUI forced-GUI-exit PASS |
| Clean production demo | PASS | bootstrap/build/test/check/bench all EXIT:0 — `docs/evidence/pass10-final-freeze-clean-demo-c536c54.md` |
| Independent final review | PASS | READY on `c536c54` — `docs/evidence/m001-pass10-independent-final-review.md` |

## Harness note (Pass 9 merge-acceptance RSS)

`Pass9MergeAcceptance` previously sampled client RSS baseline before warmups, charging Metal/IMK cold caches into `client_rss_delta` (observed 6480–7184 KiB vs the historical pre-calibration 768 constant). Aligned with `Pass9ReleaseQualification`: cold-start settle + warmups before baseline. Machine gate remains **`CLIENT_RSS_KIB = 1536`** (Pass 9 calibration at `1005bc4`; unchanged by Pass 10). Post-fix evidence: graceful=224, abrupt=512. Landed in #776; production freeze `d845c6ddbe86…`. Closeout honesty corrected in #784.

## Harness note (#778 UITest Runtime helper)

`build-for-testing` omitted `Contents/Helpers/seyal-runtime`. Ad-hoc codesign without `--identifier dev.seyal.Seyal.runtime` failed `BundledRuntimeLauncher` trust and permanently blocked recovery after `endpointMissing`. #778 installs the helper with the correct identifier after `build-for-testing` (validation harness only; production freeze unchanged).

## Final conclusion

**M001 Pass 10:** `PASS` — final freeze `c536c5454583f6a036910e145fe1187446319630` (last production behavior `a012ab0` #789; honesty #790; Engineering Quality Baseline #791). Clean-checkout demo EXIT:0 for bootstrap/build/test/check/bench; headed DisplayLink 120/120; machine RSS gate `CLIENT_RSS_KIB = 1536`. Independent Phase 2 READY. Owning Issue **#727** and parent **#5** are **closed Done**. M002 may start.
