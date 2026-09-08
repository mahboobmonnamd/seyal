# M002 #823 exact-head keyboard evidence record

- **Issue:** #823
- **Authority:** SPEC-006 M001 native input contract and the #823 issue acceptance gates
- **Measured production code head:** `8e4b52e34ed6461850cc66bbee90a301ee5e7e98`
- **Recorded:** 2026-09-08
- **Host/build boundary:** local Apple Silicon macOS host
- **Claim status:** automated and source-build evidence only; native/manual gates remain unverified

## Automated evidence

The exact production code head passed:

```text
cargo fmt --all -- --check
cargo check -p seyal-client --locked
cargo test -p seyal-client --lib --locked        # 46 passed
cargo test -p seyal-terminal --test m002_keyboard --locked  # 3 passed
cargo test -p seyal-protocol --test pass7_input_resize --locked  # 8 passed
cargo test -p seyal-runtime --lib runtime::local::ingress::tests::v2_cursor_and_keypad_modes_select_canonical_bytes --locked  # 1 passed
git diff --check
```

`make check` completed repository validation, fuzz smoke, workspace tests,
Rust tests, ARM64 Swift compilation, native shell smoke, and deterministic
renderer/input/recovery self-tests. It stopped at the existing host singleton
condition:

```text
Error: AlreadyRunning
Seyal Pass 8 Runtime-to-Swift metadata self-test failed.
make: *** [check] Error 1
```

The ARM64 Xcode build itself reported `BUILD SUCCEEDED`; the final app link
and packaged native test lane were not treated as production evidence because
the local Rust bridge/runtime singleton gate was not clean.

The production input self-test covers legacy Enter/Tab/Backspace exclusions,
navigation/function/keypad classification, immutable `[input] option_as_alt`
loading, shifted ASCII derivation from AppKit event characters, and release
metadata retention when key-up events have no characters.

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
| `make check` | **Blocked on host** | Existing `AlreadyRunning` Pass 8 singleton failure described above. |

This record is evidence for the tested boundaries and does not authorize
merging or closing #823 while the native, manual, latency, and workload gates
remain open.
