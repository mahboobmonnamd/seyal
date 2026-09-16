# #823 thin-host TerminalKeyV2 rewrite (Linux)

## Identity

| Item | Value |
| --- | --- |
| Canonical branch | `issue/823` |
| Relationship | `Refs #823` — do **not** use `Closes #823` |
| Merge base catch-up | `origin/master` merged at `6c19f42` |

## What this slice does

M001.1 deleted the product-authority Swift shell, including the previous
`TerminalInputSurface.swift` that owned both chrome and the SPEC-006 native
classifier. This slice restores **only** native keyboard/IME adapter concerns
on the thin host:

- `InteractiveMetalSurfaceView` classifies AppKit events and submits
  `TerminalKeyV2` through the existing C ABI when the Runtime negotiated
  `CAP_EXTENDED_TERMINAL_KEY`.
- Action-ID exhaustion still stops before wrap (`v2ActionIDBeforeExhaustion`)
  and requests protocol recovery.
- Held-key tracking is bounded to 256 native key identifiers; capability loss
  on reconnect drops pending releases instead of sending unnegotiated V2.
- `input.option_as_alt` is parsed in Rust (`seyal-client::input_policy`) from
  the same `SEYAL_CONFIG` / `~/.config/seyal/config.toml` file as visual
  config. Native consumes `seyal_app_option_as_alt` and does not parse TOML.
- IME preedit stays in the native `NSTextInputClient` adapter.

The deleted `SeyalShell*` / `SeyalUIConfiguration` product types are **not**
restored.

## Linux verification

Headed AppKit/IME/XCUI is `ENVIRONMENT_UNSUPPORTED` on this host.

```text
cargo test -p seyal-client --locked --lib -- input_policy
cargo test -p seyal-terminal --locked --test m002_keyboard
cargo test -p seyal-protocol --locked --lib
python3 scripts/check-thin-swift-boundary.py
```

## Still open (blocks Closes #823)

- Independent re-review GO on the exact head
- Headed keyboard / XCUI / IME on macOS (exclusive Runtime)
- Runtime local-IPC admission (`pass7_local_ipc` is `cfg(macos)`)
- Broad latency / physical perf → #673 / #824
