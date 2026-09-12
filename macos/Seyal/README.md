# Seyal macOS host

This directory owns the native macOS application boundary for Seyal.

**#890:** the rejected Swift product shell is **off the supported path**. Default `Seyal.app` launch is a native-glue harness only (window/menu + honest “not a product” label). It is **not** a headed Seyal product. Metal/IME/helper-launch/C ABI remain so #883 can write a thin host from scratch. See `docs/engineering/M001.1-SWIFT-OWNERSHIP-PARITY-MANIFEST.md` and ADR-015.

Do not add Workspace/Tab/Pane, Blocks/composer, Flow/Raw/TUI, theme semantics, or recovery-policy features in Swift. Portable product behavior is Rust-owned.

M001 established the permanent **Swift + AppKit + Metal** application direction:

- `NSApplication` / `NSWindow` lifecycle;
- a custom terminal `NSView` backed directly by `CAMetalLayer`;
- acquisition of the system `MTLDevice`;
- native Xcode application target and `.app` bundle;
- deterministic native smoke and live Runtime validation modes for CI.

## Current supported native surface

There is no headed production product in this tree until #883. What remains:

```text
seyal-runtime / TerminalExecution
  -> PTY bytes
  -> canonical VT / TerminalState
  -> Candidate-D display + metadata projection
  -> seyal-client disposable caches
  -> KEEP_NATIVE_GLUE (Metal / IME / helper launch / C ABI)
```

The native application is a **client of the separate per-user Seyal Runtime**. It does not create a second Runtime, PTY, VT parser, grid, or transcript authority inside AppKit.

No Objective-C or Objective-C++ source is justified for this boundary.

Build and validate through the canonical repository interface:

```sh
make bootstrap
make build
make test
make check
make ui-test
make bench
```

On macOS the Debug app is located at:

```text
target/macos-derived-data/Build/Products/Debug/Seyal.app
```

`open` of that bundle shows the glue harness, not panes/composer/Blocks:

```sh
open target/macos-derived-data/Build/Products/Debug/Seyal.app
```

`--ui-shell-preview` is removed. Do not treat leftover XCTest history as product authority.

The native CI path validates several distinct boundaries rather than conflating them:

- `--smoke-test` validates deterministic AppKit/Metal glue construction;
- `--renderer-self-test` validates deterministic permanent-renderer/input behavior;
- `--renderer-live-self-test` connects to a real separately started Runtime and validates Candidate-D through the Metal preparation path, including alternate screen;
- `--pass8-native-metadata-self-test` connects through the production Rust client/FFI/Swift bridge and validates real Runtime-owned Pass 8 metadata;
- `make ui-test` executes native-glue XCTest/XCUI. Product-shell XCUI is not a headed-product gate.
