# Seyal macOS host

This directory owns the native macOS application boundary for Seyal.

**M001.1 freeze:** the current AppKit **product** shell in this tree is **deprecated**. It is not product authority and is not an XCTest/XCUI oracle. Do not add Workspace/Tab/Pane, Blocks/composer, Flow/Raw/TUI, theme semantics, or recovery-policy features here. Portable product behavior is implemented in Rust (#879/#880/#740/#861/#881/#882). A **new** thin host is written from scratch (#883). Rejected product sources are removed from the supported path by #890 (that may temporarily leave `master` without a headed `Seyal.app`). Do not delete classified Metal/IME/accessibility glue until the ledger says so. See `docs/engineering/M001.1-SWIFT-OWNERSHIP-PARITY-MANIFEST.md`.

M001 established the permanent **Swift + AppKit + Metal** application direction:

- `NSApplication` / `NSWindow` lifecycle;
- a custom terminal `NSView` backed directly by `CAMetalLayer`;
- acquisition of the system `MTLDevice`;
- native Xcode application target and `.app` bundle;
- deterministic native smoke and live Runtime validation modes for CI.

## Current M001.1 supported native surface

The rejected Swift product shell is **not** a supported headed `Seyal.app`. The launch path is the new thin host (`SeyalThinHostView`) over Rust snapshots/actions, plus KEEP_NATIVE_GLUE Metal/IME/helper launch. Leftover preview/XCTest product models are deleted by #884. Qualification of the rebuilt app is #885.

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

The Debug bundle is a native-glue harness, not a headed production product:

```sh
open target/macos-derived-data/Build/Products/Debug/Seyal.app
```

Do not treat `--ui-shell-preview` or leftover `SeyalShellState` XCTest/XCUI as product authority.

The native CI path validates several distinct boundaries rather than conflating them:

- `--smoke-test` validates deterministic AppKit/Metal glue construction;
- `--renderer-self-test` validates deterministic permanent-renderer/input behavior;
- `--renderer-live-self-test` connects to a real separately started Runtime and validates Candidate-D through the Metal preparation path, including alternate screen;
- `--pass8-native-metadata-self-test` connects through the production Rust client/FFI/Swift bridge and validates real Runtime-owned Pass 8 metadata;
- `make ui-test` executes native-glue XCTest. Product-shell XCUI is skipped (#890) and is not a headed-product gate.

These test modes do not move terminal authority into the GUI and do not treat the deprecated Swift shell as product truth.
