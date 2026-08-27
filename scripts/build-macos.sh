#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "[seyal macOS build] skipped: Seyal.app builds only on macOS."
  exit 0
fi

bash scripts/check-macos-toolchain.sh

CONFIGURATION="${SEYAL_MACOS_CONFIGURATION:-Debug}"
case "$CONFIGURATION" in
  Debug|Release) ;;
  *)
    echo "[seyal macOS build] unsupported configuration: $CONFIGURATION" >&2
    exit 64
    ;;
esac

channel="$(sed -nE 's/^[[:space:]]*channel[[:space:]]*=[[:space:]]*"([^"]+)".*/\1/p' rust-toolchain.toml | head -n1)"
[[ -n "$channel" ]] || { echo "rust-toolchain.toml does not declare a Rust channel" >&2; exit 1; }

cargo_args=(build -p seyal-client --locked)
if [[ "$CONFIGURATION" == "Release" ]]; then
  cargo_args+=(--release)
fi
rustup run "$channel" cargo "${cargo_args[@]}"

DERIVED_DATA="${ROOT}/target/macos-derived-data"

xcodebuild \
  -project macos/Seyal/Seyal.xcodeproj \
  -scheme Seyal \
  -configuration "$CONFIGURATION" \
  -derivedDataPath "$DERIVED_DATA" \
  CODE_SIGNING_ALLOWED=NO \
  build

printf '[seyal macOS build] built %s\n' "${DERIVED_DATA}/Build/Products/${CONFIGURATION}/Seyal.app"
