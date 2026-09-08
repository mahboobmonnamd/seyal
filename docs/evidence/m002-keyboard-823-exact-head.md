# M002 #823 exact-head keyboard evidence record

- **Issue:** #823
- **Authority:** SPEC-006 M001 native input contract and the #823 issue acceptance gates
- **Measured production code head:** `15c44a6cba337e10de9710889b4052508e0e7372`
- **Recorded:** 2026-09-08
- **Host/build boundary:** local Apple Silicon macOS host
- **Claim status:** automated and source-build evidence only; native/manual gates remain unverified

## Automated evidence

The exact production code head passed:

```text
cargo fmt --all -- --check
cargo check -p seyal-client --locked
cargo test -p seyal-client --lib --locked        # 47 passed
cargo test -p seyal-terminal --test m002_keyboard --locked  # 3 passed
cargo test -p seyal-protocol --test pass7_input_resize --locked  # 8 passed
cargo test -p seyal-runtime --lib runtime::local::ingress::tests::v2_cursor_and_keypad_modes_select_canonical_bytes --locked  # 1 passed
git diff --check
```

The exact-head `make check` completed repository validation, fuzz smoke,
workspace tests, Rust tests, ARM64 Swift compilation, native shell smoke,
deterministic renderer/input/recovery self-tests, Runtime-to-Swift metadata,
and live Candidate-D-to-Metal checks:

```text
[seyal macOS test] Pass 8 real Runtime-to-Swift metadata acceptance passed.
[seyal macOS test] AppKit + Candidate-D + permanent Metal renderer acceptance passed.
[seyal macOS test] Swift + AppKit + Metal + UI shell scaffold acceptance passed.
```

The ARM64 Xcode build reported `BUILD SUCCEEDED`. The deterministic renderer
self-test was not accepted as green because the aggregate `--renderer-self-test`
run exited nonzero without naming a component. This is exact-head build and
source/native smoke evidence; it does not substitute for the missing headed
keyboard matrix or physical/manual IME evidence.

The production input self-test covers legacy Enter/Tab/Backspace exclusions,
navigation/function/keypad classification, immutable `[input] option_as_alt`
loading, shifted ASCII derivation from AppKit event characters, release
metadata retention when key-up events have no characters, old-server fallback
when `CAP_EXTENDED_TERMINAL_KEY` is absent, and dropping held V2 keys across
capability loss or bridge disconnect.

## Acceptance ledger

| Gate | Status | Evidence / remaining work |
| --- | --- | --- |
| Runtime remains mode-sensitive key authority | **Automated** | Runtime V2 protocol and local IPC tests pass; Swift only classifies the bounded native intent. |
| Navigation/function/keypad matrix | **Partial** | Runtime normal/application cursor and SS3 keypad regression fixtures pass; full native key matrix remains unrun. |
| Opt-in modern protocol negotiation | **Automated** | Existing Pass 7 negotiation/rejection fixtures pass; target TUI workload remains unverified. |
| IME/dead-key and shortcut non-leak | **Partial** | AppKit composition path is covered by component self-tests; headed IME/dead-key and Cmd shortcut evidence is missing. |
| Stale/observer injection rejection | **Automated** | Existing client/runtime adversarial tests pass. |
| Repeat/release under load | **Partial** | Release metadata regression is covered; physical repeat under high output is unverified. |
| Fuzzing and latency evidence | **Missing** | No exact-head native key latency matrix or dedicated keyboard fuzz campaign is retained here. |
| Native/XCUI keyboard integration | **Unverified** | Requires a clean headed macOS test lane with real key events. |
| Manual physical keyboard/layout/IME gates | **Unverified** | Do not infer these from source tests or synthetic events. |
| `make check` | **Prior head only** | The recorded green run predates `15c44a6`; current-head renderer self-test exited nonzero without naming a component and requires a clean rerun before acceptance. |

This record is evidence for the tested boundaries and does not authorize
merging or closing #823 while the native, manual, latency, and workload gates
remain open.
