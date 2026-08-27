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

# M001 builds the native app for the active host architecture. The Rust static
# library and Xcode target must agree; asking Xcode for an additional slice
# would require a separately cross-compiled Rust archive and a deliberate
# universal-binary packaging contract, which is outside Pass 6.
MACOS_ARCH="${SEYAL_MACOS_ARCH:-$(uname -m)}"
case "$MACOS_ARCH" in
  arm64|x86_64) ;;
  *)
    echo "[seyal macOS build] unsupported architecture: $MACOS_ARCH" >&2
    exit 64
    ;;
esac

channel="$(sed -nE 's/^[[:space:]]*channel[[:space:]]*=[[:space:]]*"([^"]+)".*/\1/p' rust-toolchain.toml | head -n1)"
[[ -n "$channel" ]] || { echo "rust-toolchain.toml does not declare a Rust channel" >&2; exit 1; }

rust_host_arch="$(rustup run "$channel" rustc -vV | sed -nE 's/^host: ([^-]+)-.*/\1/p')"
[[ "$rust_host_arch" == "$MACOS_ARCH" ]] || {
  echo "[seyal macOS build] Rust host architecture $rust_host_arch does not match requested Xcode architecture $MACOS_ARCH" >&2
  exit 1
}

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
  ARCHS="$MACOS_ARCH" \
  ONLY_ACTIVE_ARCH=YES \
  CODE_SIGNING_ALLOWED=NO \
  build

printf '[seyal macOS build] built %s (%s)\n' "${DERIVED_DATA}/Build/Products/${CONFIGURATION}/Seyal.app" "$MACOS_ARCH"
