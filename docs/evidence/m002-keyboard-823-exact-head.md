# M002 #823 exact-head keyboard evidence record

- **Issue:** #823
- **Authority:** SPEC-006 M001 native input contract and the #823 issue acceptance gates
- **Measured production code head:** `beeace4`
- **Recorded:** 2026-09-09
- **Host/build boundary:** local Apple Silicon macOS host
- **Claim status:** automated and source-build evidence only; native/manual gates remain unverified

## Automated evidence

The exact production code head passed the repository gate:

```text
make check
git diff --check
```

At exact head `beeace4`, the full `make check` passed repository/static
analysis, Rust and component tests, all active fuzz-smoke targets, the ARM64
Xcode build, native Swift/AppKit/Metal shell smoke, deterministic
renderer/input/recovery, real Runtime-to-Swift metadata acceptance, and live
Candidate-D-to-Metal checks:

```text
[seyal macOS test] Pass 8 real Runtime-to-Swift metadata acceptance passed.
[seyal macOS test] AppKit + Candidate-D + permanent Metal renderer acceptance passed.
[seyal macOS test] Swift + AppKit + Metal + UI shell scaffold acceptance passed.
```

This is exact-head repository and native-shell evidence; it does not
substitute for the missing headed keyboard matrix or physical/manual IME
evidence.

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
| `make check` | **Automated** | Full exact-head `make check` passed at `beeace4`; separate headed/manual keyboard, workload, latency, and physical IME gates remain open. |

This record is evidence for the tested boundaries and does not authorize
merging or closing #823 while the native, manual, latency, and workload gates
remain open.

## Focused headed XCTest rerun

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
physical repeat-under-load, dead-key/IME, target-TUI negotiation, dedicated
keyboard latency/fuzz evidence, and independent review remain open.
