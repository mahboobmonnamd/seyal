#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "[seyal macOS toolchain] skipped: macOS-only native toolchain."
  exit 0
fi

fail() {
  printf '[seyal macOS toolchain] ERROR: %s\n' "$*" >&2
  exit 1
}

command -v xcode-select >/dev/null 2>&1 || fail "xcode-select is required"
command -v xcrun >/dev/null 2>&1 || fail "xcrun is required"
command -v xcodebuild >/dev/null 2>&1 || fail "full Xcode is required starting with Issue #10"

xcode-select -p >/dev/null 2>&1 || fail "no active Xcode developer directory"
xcodebuild -version >/dev/null 2>&1 || fail "full Xcode is required; select it with xcode-select before building Seyal.app"
xcrun --sdk macosx --show-sdk-path >/dev/null 2>&1 || fail "macOS SDK is unavailable from the active Xcode"
xcrun --find swiftc >/dev/null 2>&1 || fail "Swift compiler is unavailable from the active Xcode"
xcrun --find metal >/dev/null 2>&1 || fail "Metal shader toolchain is unavailable from the active Xcode"

printf '[seyal macOS toolchain] Swift, macOS SDK, Xcode build tools and Metal toolchain are ready.\n'
