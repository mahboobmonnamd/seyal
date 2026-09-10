# M002 #823 exact-head keyboard evidence record

- **Issue:** #823
- **Authority:** SPEC-006 M001 native input contract and the #823 issue acceptance gates
- **Measured production code head:** `9848add7b03c8d402b55164cd27f9008a3df052e`
- **Later client-admission fix:** `f24e2a4` (TerminalKeyV2 backpressure + capability/action_id guards; macOS-only client tests not executed on this Linux host)
- **Recorded:** 2026-09-10
- **Host/build boundary:** Linux x86_64 cloud agent (`uname -srm`: `Linux 6.12.94+ x86_64`) plus previously retained Apple Silicon headed notes
- **Claim status:** automated, source-build, security-review, and bounded fuzz evidence; native/manual gates remain unverified; `performance_claim=false`

## Automated evidence

### Repository gate (prior exact head)

At production head `fe70733` (ancestor of `9848add`), the full `make check` passed:

```text
make check
git diff --check
```

That run included repository/static analysis, Rust and component tests, all active fuzz-smoke targets, the ARM64 Xcode build, native Swift/AppKit/Metal shell smoke, deterministic renderer/input/recovery, real Runtime-to-Swift metadata acceptance, and live Candidate-D-to-Metal checks. Subsequent keypad-mapping and exact-head documentation commits landed as `385b5c9`…`9848add` without reopening those native packaging gates on this Linux host.

### Fuzz smoke (this host, `9848add`)

Command:

```text
python3 scripts/fuzz-smoke.py
```

Result (EXIT 0):

```text
[seyal fuzz smoke] active vt-byte-parser: 3 retained seed(s) passed; libfuzzer=vt_byte_parser
[seyal fuzz smoke] active parser-state-mutation: 2 retained seed(s) passed; libfuzzer=parser_state_mutation
[seyal fuzz smoke] active local-binary-protocol-decode: 5 retained seed(s) passed; libfuzzer=local_binary_protocol_decode
[seyal fuzz smoke] active display-binary-decode: 1 retained seed(s) passed; libfuzzer=display_decode
[seyal fuzz smoke] active display-v2-decode: 2 retained seed(s) passed; libfuzzer=display_v2_decode
[seyal fuzz smoke] active display-state-machine: 3 retained seed(s) passed; libfuzzer=display_state_machine
[seyal fuzz smoke] active reconnect-resync-state-machine: 3 retained seed(s) passed; libfuzzer=reconnect_resync_state_machine
[seyal fuzz smoke] active pass7-protocol-decode: 11 retained seed(s) passed; libfuzzer=pass7_protocol_decode
[seyal fuzz smoke] active block-state-decode: 1 retained seed(s) passed; libfuzzer=pass8_block_state_decode
[seyal fuzz smoke] comparator shared-projection-validation: corpus present; not production §6.9 coverage
[seyal fuzz smoke] surface_decision pass 9: N/A (proof=docs/engineering/M001-FUZZ-EVIDENCE.md)
[seyal fuzz smoke] registry valid: 9 active, 0 pending, 1 non-production comparator(s); campaign parity ok.
```

No dedicated encoder/key-admission libFuzzer target exists beyond Pass 7 protocol decode (which includes `TerminalKey` and `TerminalKeyV2`). Mutation campaign units generated under `fuzz/corpus/pass7-protocol-decode/` were discarded and not retained.

### Bounded libFuzzer campaign (this host, `9848add`)

Commands:

```text
rustup toolchain install nightly-2026-08-20 --profile minimal
cargo +nightly-2026-08-20 install cargo-fuzz --version 0.13.2 --locked
LIBRARY_PATH=/usr/lib/gcc/x86_64-linux-gnu/13 CXX=g++ CC=gcc \
  cargo +nightly-2026-08-20 fuzz build pass7_protocol_decode
LIBRARY_PATH=/usr/lib/gcc/x86_64-linux-gnu/13 CXX=g++ CC=gcc \
  cargo +nightly-2026-08-20 fuzz run pass7_protocol_decode corpus/pass7-protocol-decode -- \
    -max_total_time=45 -timeout=10 -rss_limit_mb=1024 -print_final_stats=1
```

Result: **no crash, timeout, or RSS-limit abort.** Evidence grade is `ci-smoke` (45s), not Pass 10 §6.9 `nightly-campaign` (600s).

```text
INFO: Seed: 437768633
INFO: Loaded 1 modules   (2116 inline 8-bit counters)
INFO:       11 files found in corpus/pass7-protocol-decode
INFO: seed corpus: files: 11 min: 32b max: 88b total: 612b rss: 32Mb
#12     INITED cov: 280 ft: 381 corp: 11/612b
… final observed coverage before stop: cov: 844 ft: 3282 corp: 757/90Kb rss: 509Mb
Done 12992053 runs in 46 second(s)
stat::number_of_executed_units: 12992053
stat::average_exec_per_sec:     282435
stat::new_units_added:          2937
stat::slowest_unit_time_sec:    0
stat::peak_rss_mb:              509
```

`LIBRARY_PATH` / `g++` were required on this image because clang/`rust-lld` did not find `libstdc++` via the default search path. That is a host toolchain detail, not a product change.

### Key-latency bench (this host)

Existing harness: `crates/seyal-client/benches/pass7_input_resize.rs` (Pass 7 client→Runtime→PTY marks; `performance_claim=false` by construction). Related: `pass7_validation_matrix` (macOS-only key-repeat burst).

Command:

```text
cargo bench -p seyal-client --bench pass7_input_resize --features benchmark-instrumentation --locked
```

Result:

```text
Finished `bench` profile [optimized] target(s) in 2.45s
pass7_input_resize PLATFORM_LIMITED target_os!=macos performance_claim=false
```

No native-key→Runtime admission percentiles, no key→PTY Apple Silicon numbers, and **no scanout / key-to-photon claim**. SPEC-006 §21.7 / #673 ARM64 headed measurement remains **Missing** on this host. No speculative production instrumentation was added.

### Security review

See `docs/evidence/m002-keyboard-823-security-review.md`. Verdict: **PASS** for the source/automated key-admission path (no P0–P2). Headed/manual gates remain open.

## Acceptance ledger

| Gate | Status | Evidence / remaining work |
| --- | --- | --- |
| Runtime remains mode-sensitive key authority | **Automated** | Runtime V2 protocol and local IPC tests pass; Swift only classifies the bounded native intent. |
| Navigation/function/keypad matrix | **Partial** | Runtime normal/application cursor and SS3 keypad regression fixtures pass; full native key matrix remains unrun. |
| Opt-in modern protocol negotiation | **Automated** | Existing Pass 7 negotiation/rejection fixtures pass; target TUI workload remains unverified. |
| IME/dead-key and shortcut non-leak | **Partial** | AppKit composition path is covered by component self-tests and source security review; headed IME/dead-key and Cmd shortcut evidence is missing. |
| Stale/observer injection rejection | **Automated** | Existing client/runtime adversarial tests plus source review of V2 `authorize_mutation`. Dedicated V2 observer IPC fixtures remain thinner than Input/V1. |
| Repeat/release under load | **Partial** | Release metadata regression is covered; physical repeat under high output is unverified. |
| Fuzzing | **Partial (ci-smoke)** | Registry smoke 9/9 active targets; 45s `pass7_protocol_decode` campaign 12_992_053 execs, 0 crashes. Not a 600s nightly. |
| Key latency | **PLATFORM_LIMITED / Missing native** | Linux Release harness printed `PLATFORM_LIMITED`; no ARM64 native-key→PTY matrix. `performance_claim=false`. |
| Native/XCUI keyboard integration | **Unverified** | Requires a clean headed macOS test lane with real key events. Prior focused shortcut XCUI at `fe70733` does not close the full matrix. |
| Manual physical keyboard/layout/IME gates | **ENVIRONMENT_UNSUPPORTED** | See `docs/evidence/m002-keyboard-823-headed-manual.md`. Linux cloud agent; no HID/IME/keypad. |
| Independent security review | **Automated source review** | `docs/evidence/m002-keyboard-823-security-review.md` at `9848add`. Not a substitute for headed evidence. |
| `make check` | **Automated (prior)** | Full exact-head `make check` passed at `fe70733`; this Linux pass did not re-run the native macOS packaging lane. |
| Closing PR | **Not allowed** | Headed/manual/latency/target-TUI gates remain open. |

## Focused headed XCTest rerun (prior, Apple Silicon)

After rebuilding the disposable `SeyalUITests-Runner.app` and applying the
documented ad-hoc signatures, the exact focused headed test passed:

```text
xcodebuild ... -only-testing:SeyalUITests/SeyalShellUITests/testNativeKeyboardShortcutsSwitchWorkspaceTabsAndSidebars test-without-building
Test Case ... testNativeKeyboardShortcutsSwitchWorkspaceTabsAndSidebars passed
Executed 1 test, with 0 failures (12.820 seconds)
** TEST EXECUTE SUCCEEDED **
```

The retained result bundle is
`target/macos-ui-tests-keyboard.xcresult`. This closes only the focused
synthetic/XCUI shortcut workflow. The full navigation/function/keypad matrix,
physical repeat-under-load, dead-key/IME, target-TUI negotiation, native-key
latency, 600s fuzz campaign, and independent headed review remain open.

This record is evidence for the tested boundaries and does not authorize
merging or closing #823 while those gates remain open.
