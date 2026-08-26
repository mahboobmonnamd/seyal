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

pass5_failure_matrix() {
  cargo_pinned test -p seyal-runtime --locked --features test-fault-injection \
    --test local_ipc_failure_injection \
    --test runtime_adversarial
}

case "$cmd" in
  bootstrap)
    bash scripts/bootstrap-toolchain.sh
    ;;
  bootstrap-agents)
    bash scripts/bootstrap-dev.sh
    ;;
  build)
    bash scripts/check-toolchain.sh
    cargo_pinned build --workspace --locked
    bash scripts/build-macos.sh
    ;;
  test)
    bash scripts/test-tooling.sh
    python3 scripts/test-workspace.py
    python3 scripts/test-harnesses.py
    python3 scripts/fuzz-smoke.py
    bash scripts/check-toolchain.sh
    cargo_pinned test --workspace --locked
    pass5_failure_matrix
    bash scripts/test-macos-skeleton.sh
    ;;
  check)
    bash scripts/check-toolchain.sh
    bash -n scripts/*.sh
    bash scripts/validate-governance.sh
    python3 scripts/check-doc-links.py
    python3 scripts/check-layering.py
    python3 scripts/check-hot-path.py
    python3 scripts/check-benchmark-contract.py
    bash scripts/test-tooling.sh
    python3 scripts/test-workspace.py
    python3 scripts/test-harnesses.py
    python3 scripts/fuzz-smoke.py
    python3 scripts/test-ci-validators.py
    cargo_pinned fmt --all -- --check
    cargo_pinned clippy --workspace --all-targets --all-features -- -D warnings
    cargo_pinned test --workspace --locked
    pass5_failure_matrix
    bash scripts/test-macos-skeleton.sh
    ;;
  bench)
    bash scripts/check-toolchain.sh
    python3 scripts/check-benchmark-contract.py
    python3 scripts/benchmark-smoke.py
    if find crates -type f -path '*/benches/*.rs' -print -quit 2>/dev/null | grep -q .; then
      # Darwin Unix-domain sockets have a 104-byte sun_path limit. The production
      # benchmark deliberately uses temporary Runtime directories; keep the
      # benchmark root deterministic and short instead of inheriting a potentially
      # long hosted-runner TMPDIR. Production runtime discovery is unchanged.
      if [[ "$(uname -s)" == "Darwin" ]]; then
        export TMPDIR=/tmp
      fi
      cargo_pinned bench --workspace --locked
      cargo_pinned bench -p seyal-runtime --bench pass5_production_transport --features benchmark-instrumentation --locked
    else
      echo "[seyal task] bench: harness metadata recorder passed; no production benchmark target exists yet and no performance result is claimed."
    fi
    ;;
  *)
    echo "usage: $0 {bootstrap|bootstrap-agents|build|test|check|bench}" >&2
    exit 64
    ;;
esac
