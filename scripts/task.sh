#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

cmd="${1:-}"
channel="$(sed -nE 's/^[[:space:]]*channel[[:space:]]*=[[:space:]]*"([^"]+)".*/\1/p' rust-toolchain.toml 2>/dev/null | head -n1 || true)"

cargo_pinned() {
  [[ -n "$channel" ]] || { echo "rust-toolchain.toml does not declare a Rust channel" >&2; exit 1; }
  rustup run "$channel" cargo "$@"
}

case "$cmd" in
  bootstrap)
    bash scripts/bootstrap-dev.sh
    ;;
  bootstrap-agents)
    bash scripts/bootstrap-agents.sh
    ;;
  build)
    bash scripts/check-toolchain.sh
    if [[ -f Cargo.toml ]]; then
      cargo_pinned build --workspace --locked
    else
      echo "[seyal task] build: no Rust workspace exists yet; Issue #9 owns workspace scaffolding. Nothing to build."
    fi
    ;;
  test)
    bash scripts/test-tooling.sh
    if [[ -f Cargo.toml ]]; then
      bash scripts/check-toolchain.sh
      cargo_pinned test --workspace --locked
    fi
    ;;
  check)
    bash scripts/check-toolchain.sh
    bash -n scripts/*.sh
    bash scripts/validate-governance.sh
    python3 scripts/check-doc-links.py
    python3 scripts/check-layering.py
    bash scripts/test-tooling.sh
    if [[ -f Cargo.toml ]]; then
      cargo_pinned fmt --all -- --check
      cargo_pinned clippy --workspace --all-targets --all-features -- -D warnings
      cargo_pinned test --workspace --locked
    fi
    ;;
  bench)
    bash scripts/check-toolchain.sh
    if [[ -f Cargo.toml ]]; then
      cargo_pinned bench --workspace --locked
    else
      echo "[seyal task] bench: no benchmarkable production surface exists yet; no performance result is claimed."
    fi
    ;;
  *)
    echo "usage: $0 {bootstrap|bootstrap-agents|build|test|check|bench}" >&2
    exit 64
    ;;
esac
