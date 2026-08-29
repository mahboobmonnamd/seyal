#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "[seyal macOS UI test] skipped: native XCTest/XCUIAutomation requires macOS."
  exit 0
fi

bash scripts/check-macos-toolchain.sh

# The Pass 8 native application test starts the real, separately owned Runtime
# binary and then executes Seyal.app against it. Build the fixture through the
# pinned repository toolchain so `make ui-test` is self-contained.
channel="$(sed -nE 's/^[[:space:]]*channel[[:space:]]*=[[:space:]]*"([^"]+)".*/\1/p' rust-toolchain.toml | head -n1)"
[[ -n "$channel" ]] || { echo "rust-toolchain.toml does not declare a Rust channel" >&2; exit 1; }
rustup run "$channel" cargo build -p seyal-runtime --bin seyal-runtime --locked

DERIVED_DATA="$ROOT/target/macos-ui-tests"
RESULT_BUNDLE="$ROOT/target/macos-ui-tests.xcresult"
rm -rf "$DERIVED_DATA" "$RESULT_BUNDLE"

xcodebuild \
  -project macos/Seyal/Seyal.xcodeproj \
  -scheme Seyal \
  -configuration Debug \
  -destination 'platform=macOS' \
  -derivedDataPath "$DERIVED_DATA" \
  ARCHS=arm64 \
  ONLY_ACTIVE_ARCH=YES \
  CODE_SIGNING_ALLOWED=NO \
  CODE_SIGNING_REQUIRED=NO \
  SWIFT_ACTIVE_COMPILATION_CONDITIONS='DEBUG $(inherited)' \
  build-for-testing

# Xcode's no-signing mode is required for reproducible local Rust linking, but
# macOS will refuse to load its generated XCTest/XCUI runner as an unsigned
# bundle. Sign only the disposable DerivedData products ad hoc, then execute
# the already-built test plan. This does not alter production signing.
PRODUCTS="$DERIVED_DATA/Build/Products/Debug"
codesign --force --deep --sign - "$PRODUCTS/Seyal.app"
codesign --force --deep --sign - "$PRODUCTS/SeyalUITests-Runner.app"
codesign --verify --deep --strict "$PRODUCTS/Seyal.app"
codesign --verify --deep --strict "$PRODUCTS/SeyalUITests-Runner.app"

xcodebuild \
  -project macos/Seyal/Seyal.xcodeproj \
  -scheme Seyal \
  -configuration Debug \
  -destination 'platform=macOS' \
  -derivedDataPath "$DERIVED_DATA" \
  ARCHS=arm64 \
  ONLY_ACTIVE_ARCH=YES \
  -resultBundlePath "$RESULT_BUNDLE" \
  test-without-building

echo "[seyal macOS UI test] XCTest component + XCUIAutomation E2E passed."
