# Seyal macOS host

This directory owns the native macOS application boundary for Seyal.

M001 Issue #10 establishes the permanent **Swift + AppKit + Metal** application direction:

- `NSApplication` / `NSWindow` lifecycle;
- a custom terminal `NSView` backed directly by `CAMetalLayer`;
- acquisition of the system `MTLDevice`;
- native Xcode application target and `.app` bundle;
- deterministic native smoke mode for CI.

The pre-Pass-6 UI shell scaffold adds native presentation structure without changing terminal authority or starting live renderer/runtime integration. It includes design tokens, the Core Terminal shell regions, intrinsic Blocks, a Pane-owned transcript scroll surface, a Pane-scoped composer presentation seam, contextual panels, attention presentation, and a `TerminalSurfaceHostView` around the permanent Metal surface.

The shell scaffold intentionally does **not** implement terminal rendering, shaping/glyph caches, VT state, PTY/runtime ownership, live Runtime attachment, native input, IME/accessibility behavior, shell integration, or final product actions. Those remain governed by M001 pass ordering and the accepted architecture.

No Objective-C or Objective-C++ source is justified for this boundary. If a future issue finds a concrete API/interoperability requirement that Swift plus a coarse C-compatible Rust boundary cannot satisfy, that evidence must be reviewed before introducing another native language.

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

Launch the normal M001 app path with:

```sh
open target/macos-derived-data/Build/Products/Debug/Seyal.app
```

The normal path remains the minimal Metal surface before Pass 6.

For explicit design/decomposition review, the Debug bundle supports a fixture-only shell preview:

```sh
target/macos-derived-data/Build/Products/Debug/Seyal.app/Contents/MacOS/Seyal --ui-shell-preview
```

The preview data is deterministic and presentation-only. It is never Runtime, PTY, VT, grid, Block-history, or execution authority and is not compiled into Release behavior.

The CI smoke path runs the bundle executable with `--smoke-test`; it validates Metal availability and deterministic construction of the native UI shell without starting the AppKit event loop.
