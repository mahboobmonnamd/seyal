#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "[seyal macOS UI test] skipped: native XCTest/XCUIAutomation requires macOS."
  exit 0
fi

bash scripts/check-macos-toolchain.sh

DERIVED_DATA="$ROOT/target/macos-ui-tests"
RESULT_BUNDLE="$ROOT/target/macos-ui-tests.xcresult"
rm -rf "$DERIVED_DATA" "$RESULT_BUNDLE"

xcodebuild \
  -project macos/Seyal/Seyal.xcodeproj \
  -scheme Seyal \
  -configuration Debug \
  -destination 'platform=macOS' \
  -derivedDataPath "$DERIVED_DATA" \
  -resultBundlePath "$RESULT_BUNDLE" \
  ARCHS=arm64 \
  ONLY_ACTIVE_ARCH=YES \
  CODE_SIGNING_ALLOWED=NO \
  CODE_SIGNING_REQUIRED=NO \
  SWIFT_ACTIVE_COMPILATION_CONDITIONS='DEBUG $(inherited)' \
  test

echo "[seyal macOS UI test] XCTest component + XCUIAutomation E2E passed."
