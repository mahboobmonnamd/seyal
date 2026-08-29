# Seyal macOS host

This directory owns the native macOS application boundary for Seyal.

M001 establishes the permanent **Swift + AppKit + Metal** application direction:

- `NSApplication` / `NSWindow` lifecycle;
- a custom terminal `NSView` backed directly by `CAMetalLayer`;
- acquisition of the system `MTLDevice`;
- native Xcode application target and `.app` bundle;
- deterministic native smoke and live Runtime validation modes for CI.

## Current M001 production boundary

The macOS host is no longer the pre-Pass-6 scaffold. Through Pass 7.1 it contains the permanent Metal renderer, the Candidate-D local Runtime client, native terminal input/resize handling, the Pane-owned transcript/composer shell, and trusted-shell command Block presentation. Pass 8 adds only a minimal read-only execution-Block metadata seam; it does not create another terminal, transcript, or renderer authority.

The production ownership boundary remains:

```text
seyal-runtime / TerminalExecution
  -> PTY bytes
  -> canonical VT / TerminalState
  -> Candidate-D display + metadata projection
  -> seyal-client disposable caches
  -> Swift/AppKit/Metal presentation
```

The native application is a **client of the separate per-user Seyal Runtime**. It does not create a second Runtime, PTY, VT parser, grid, or transcript authority inside AppKit. Therefore launching `Seyal.app` with no running Runtime can legitimately show that the Runtime connection is unavailable. Live terminal validation must start `seyal-runtime` first; the canonical macOS test harness does this automatically for real Candidate-D/Metal and Pass 8 Runtime-to-Swift metadata checks.

No Objective-C or Objective-C++ source is justified for this boundary. If a future issue finds a concrete API/interoperability requirement that Swift plus a coarse C-compatible Rust boundary cannot satisfy, that evidence must be reviewed before introducing another native language.

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

Launch the normal app path with:

```sh
open target/macos-derived-data/Build/Products/Debug/Seyal.app
```

For explicit design/decomposition review, the Debug bundle supports a fixture-only shell preview:

```sh
target/macos-derived-data/Build/Products/Debug/Seyal.app/Contents/MacOS/Seyal --ui-shell-preview
```

The preview data is deterministic and presentation-only. It is never Runtime, PTY, VT, grid, Block-history, or execution authority. The app stamps its Xcode build configuration into the bundle and honors the preview flag only when that configuration is `Debug`; Release builds ignore the preview flag and environment opt-in.

The native CI path validates several distinct boundaries rather than conflating them:

- `--smoke-test` validates deterministic AppKit/Metal/UI-shell construction;
- `--renderer-self-test` validates deterministic permanent-renderer/input behavior;
- `--renderer-live-self-test` connects to a real separately started Runtime and validates Candidate-D through the Metal preparation path, including alternate screen;
- `--pass8-native-metadata-self-test` connects through the production Rust client/FFI/Swift bridge and validates real Runtime-owned Pass 8 metadata;
- `make ui-test` executes the XCTest/XCUIAutomation suite through `scripts/test-macos-ui.sh`.

These test modes do not move terminal authority into the GUI and do not substitute preview fixtures for Runtime-owned state.
