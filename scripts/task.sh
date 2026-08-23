#!/usr/bin/env bash
set -euo pipefail

cmd="${1:-}"

case "$cmd" in
  bootstrap)
    echo "Governance bootstrap complete. M001 Pass 1 must add deterministic Rust/macOS toolchain bootstrap here when production scaffolding is created."
    ;;
  build)
    if [[ -f Cargo.toml ]]; then
      cargo build --workspace
    else
      echo "No production Rust workspace exists yet. M001 Pass 1 owns creation of the minimal justified workspace." >&2
      exit 2
    fi
    ;;
  test)
    if [[ -f Cargo.toml ]]; then
      cargo test --workspace
    else
      echo "No production test workspace exists yet. Run 'make governance-check' for the pre-M001 repository system." >&2
      exit 2
    fi
    ;;
  check)
    bash scripts/validate-governance.sh
    if [[ -f Cargo.toml ]]; then
      cargo fmt --all -- --check
      cargo clippy --workspace --all-targets --all-features -- -D warnings
      cargo test --workspace
    fi
    ;;
  bench)
    if [[ -f Cargo.toml ]]; then
      cargo bench --workspace
    else
      echo "No benchmark harness exists yet. M001 Pass 1 owns the first reproducible benchmark skeleton." >&2
      exit 2
    fi
    ;;
  *)
    echo "usage: $0 {bootstrap|build|test|check|bench}" >&2
    exit 64
    ;;
esac
