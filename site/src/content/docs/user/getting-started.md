---
title: Getting Started
description: Build and run Seyal from the current source tree.
---

Seyal does not yet publish a stable end-user release. For now, the supported path is a source checkout used by contributors and early testers.

## Prerequisites

- macOS on Apple Silicon for the current native target.
- Rust toolchain required by the repository.
- Xcode / macOS development tools required by the native host when that milestone is active.
- Git.

## Bootstrap

From the repository root:

```sh
make bootstrap
```

Use the canonical repository commands rather than calling internal build scripts directly:

```sh
make build
make test
make check
make bench
```

Some commands may intentionally report that a later milestone has not created a production component yet. That is preferable to documentation pretending an unfinished surface is available.

## Appearance configuration (under development)

Seyal may read `~/.config/seyal/config.toml` or the path in `SEYAL_CONFIG` at launch. Only a small user-facing subset is accepted:

```toml
[ui]
appearance = "system" # system | light | dark
reduced-material = false
utility-opacity = 1.0
window-padding = 0

[ui.font]
family = ""
size = 12
fallbacks = ["SF Pro Text"]

[terminal]
padding = 8

[terminal.font]
family = "Menlo"
size = 14
fallbacks = ["SF Mono", "Menlo"]
```

Invalid keys or values are ignored or clamped; Seyal always starts from a complete snapshot. This is not a shipped settings product yet.

For a one-off override without editing a file, launch with `open --env` (shell `VAR=value open …` does not pass environment into the app):

```sh
open --env SEYAL_UI_APPEARANCE=light target/macos-derived-data/Build/Products/Debug/Seyal.app
```

Or run the binary directly so the shell environment is inherited.

## Next

See **What is available now?** before relying on a feature described in product plans or architecture documents.
