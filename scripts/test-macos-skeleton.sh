#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "[seyal macOS test] skipped: native AppKit/Metal renderer is macOS-only."
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
[[ -f "$SOURCES/TerminalShaders.metal" ]] || fail "missing permanent terminal Metal shaders"
[[ -f "$SOURCES/SeyalBridge.h" ]] || fail "missing coarse Rust/native bridge header"

if find macos/Seyal -type f \( -name '*.m' -o -name '*.mm' -o -name '*.cc' -o -name '*.cpp' \) -print -quit | grep -q .; then
  fail "native host must remain Swift-only unless a later ADR justifies another language"
fi

find "$SOURCES" -type f -name '*.swift' -print -quit | grep -q . || fail "native host has no Swift source"
grep -R -q '^import AppKit$' "$SOURCES" || fail "Swift native host must use AppKit"
grep -R -q '^import Metal$' "$SOURCES" || fail "Swift native host must use Metal"
grep -R -q '^import QuartzCore$' "$SOURCES" || fail "Swift native host must use QuartzCore/CAMetalLayer"
grep -R -q 'CAMetalLayer' "$SOURCES" || fail "native surface must use CAMetalLayer"
grep -R -q 'makeCommandQueue' "$SOURCES" || fail "Pass 6 requires a real Metal command queue"
grep -R -q 'makeRenderPipelineState' "$SOURCES" || fail "Pass 6 requires a real Metal pipeline"
grep -R -q 'nextDrawable' "$SOURCES" || fail "production renderer must acquire CAMetalLayer drawables"
grep -R -q 'commandBuffer.present' "$SOURCES" || fail "production renderer must present Metal drawables"
grep -R -q 'DispatchSource.makeReadSource' "$SOURCES" || fail "Candidate-D client must be readiness-driven, not polled"

if grep -R -E -q '(^|[^A-Za-z])(SwiftUI|NSTextView)([^A-Za-z]|$)' "$SOURCES"; then
  fail "temporary SwiftUI/NSTextView terminal surfaces are forbidden"
fi

bash scripts/build-macos.sh

APP="${ROOT}/target/macos-derived-data/Build/Products/Debug/Seyal.app"
BINARY="${APP}/Contents/MacOS/Seyal"
[[ -d "$APP" ]] || fail "xcodebuild did not produce Seyal.app"
[[ -x "$BINARY" ]] || fail "Seyal.app executable is missing"

"$BINARY" --smoke-test
"$BINARY" --renderer-self-test

channel="$(sed -nE 's/^[[:space:]]*channel[[:space:]]*=[[:space:]]*"([^"]+)".*/\1/p' rust-toolchain.toml | head -n1)"
[[ -n "$channel" ]] || fail "rust-toolchain.toml does not declare a Rust channel"
rustup run "$channel" cargo build -p seyal-runtime --bin seyal-runtime --locked
RUNTIME="${ROOT}/target/debug/seyal-runtime"
[[ -x "$RUNTIME" ]] || fail "seyal-runtime fixture executable is missing"

runtime_pid=""
cleanup_runtime() {
  if [[ -n "$runtime_pid" ]] && kill -0 "$runtime_pid" 2>/dev/null; then
    kill "$runtime_pid" 2>/dev/null || true
    wait "$runtime_pid" 2>/dev/null || true
  fi
  runtime_pid=""
}
trap cleanup_runtime EXIT

run_live_renderer_case() {
  local command="$1"
  shift
  cleanup_runtime
  "$RUNTIME" /bin/sh -c "$command" &
  runtime_pid=$!

  local passed=0
  for _ in $(seq 1 20); do
    if "$BINARY" --renderer-live-self-test "$@"; then
      passed=1
      break
    fi
    if ! kill -0 "$runtime_pid" 2>/dev/null; then
      break
    fi
    sleep 0.05
  done
  [[ "$passed" == "1" ]] || fail "live Candidate-D to Metal case failed: $command"
  wait "$runtime_pid" || true
  runtime_pid=""
}

run_live_renderer_case "printf 'SEYAL-LIVE'; sleep 1"
run_live_renderer_case "printf '\033[?1049hALT-LIVE'; sleep 1" --expect-alternate

trap - EXIT
cleanup_runtime

echo "[seyal macOS test] AppKit + Candidate-D + permanent Metal renderer acceptance passed."
