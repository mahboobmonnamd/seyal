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

## Next

See **What is available now?** before relying on a feature described in product plans or architecture documents.
