# Seyal macOS host

This directory owns the native macOS application boundary for Seyal.

M001 Issue #10 establishes only a **Swift + AppKit + Metal** application skeleton:

- `NSApplication` / `NSWindow` lifecycle;
- one custom `NSView` backed directly by `CAMetalLayer`;
- acquisition of the system `MTLDevice`;
- native Xcode application target and `.app` bundle;
- deterministic native smoke mode for CI.

It intentionally does **not** implement terminal rendering, shaping/glyph caches, VT state, PTY/runtime ownership, Blocks, input handling, IME/accessibility behavior, or product UI.

No Objective-C or Objective-C++ source is justified for this boundary. If a future Issue finds a concrete API/interoperability requirement that Swift plus a coarse C-compatible Rust boundary cannot satisfy, that evidence must be reviewed before introducing another native language.

Build through the canonical repository interface:

```sh
make bootstrap
make build
make test
make check
```

On macOS the built app is located at:

```text
target/macos-derived-data/Build/Products/Debug/Seyal.app
```

Launch the non-terminal skeleton with:

```sh
open target/macos-derived-data/Build/Products/Debug/Seyal.app
```

The CI smoke path runs the bundle executable with `--smoke-test`; it validates that the process launches and Metal can supply a device/layer without starting the AppKit event loop.

The design-review branch also provides a preview-only harness that opens the approved PNG reference boards inside the app:

```sh
make design-preview
```

That harness is review scaffolding only; it does not change the production terminal/runtime architecture or claim that the component implementation is complete.
