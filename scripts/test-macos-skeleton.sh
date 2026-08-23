#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "[seyal macOS test] skipped: native AppKit/Metal skeleton is macOS-only."
  exit 0
fi

fail() {
  printf '[seyal macOS test] ERROR: %s\n' "$*" >&2
  exit 1
}

PROJECT="macos/Seyal/Seyal.xcodeproj"
SOURCES="macos/Seyal/Sources"

[[ -f "$PROJECT/project.pbxproj" ]] || fail "missing native Xcode project"
[[ -f "$PROJECT/xcshareddata/xcschemes/Seyal.xcscheme" ]] || fail "missing shared Seyal Xcode scheme"
[[ -f "macos/Seyal/Info.plist" ]] || fail "missing native app Info.plist"
[[ -d "$SOURCES" ]] || fail "missing native Swift sources"

if find macos/Seyal -type f \( -name '*.m' -o -name '*.mm' -o -name '*.cc' -o -name '*.cpp' \) -print -quit | grep -q .; then
  fail "Issue #10 must use Swift only; Objective-C/Objective-C++/C++ sources are not justified"
fi

find "$SOURCES" -type f -name '*.swift' -print -quit | grep -q . || fail "native host has no Swift source"
grep -R -q '^import AppKit$' "$SOURCES" || fail "Swift native host must use AppKit"
grep -R -q '^import Metal$' "$SOURCES" || fail "Swift native host must use Metal"
grep -R -q '^import QuartzCore$' "$SOURCES" || fail "Swift native host must use QuartzCore/CAMetalLayer"
grep -R -q 'CAMetalLayer' "$SOURCES" || fail "native surface must establish the permanent CAMetalLayer direction"
grep -R -q 'MTLCreateSystemDefaultDevice' "$SOURCES" || fail "native surface must acquire a real Metal device"

if grep -R -E -q '(^|[^A-Za-z])(SwiftUI|NSTextView)([^A-Za-z]|$)' "$SOURCES"; then
  fail "temporary SwiftUI/NSTextView terminal surfaces are forbidden"
fi

bash scripts/build-macos.sh

APP="${ROOT}/target/macos-derived-data/Build/Products/Debug/Seyal.app"
BINARY="${APP}/Contents/MacOS/Seyal"
[[ -d "$APP" ]] || fail "xcodebuild did not produce Seyal.app"
[[ -x "$BINARY" ]] || fail "Seyal.app executable is missing"

"$BINARY" --smoke-test

echo "[seyal macOS test] Swift + AppKit + Metal skeleton acceptance passed."
