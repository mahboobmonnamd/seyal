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

# KEEP_NATIVE_GLUE only. Rejected product-shell sources are not a supported
# headed product path (#890). #883 writes the replacement host.
for required in \
  AppDelegate.swift \
  Main.swift \
  MetalSurfaceView.swift \
  MetalTerminalRenderer.swift \
  GlyphAtlas.swift \
  TerminalSurfaceHostView.swift \
  TerminalInputSurface.swift \
  RustDisplayBridge.swift \
  BundledRuntimeLauncher.swift \
  SeyalAccessibilityAnnouncement.swift; do
  [[ -f "$SOURCES/$required" ]] || fail "missing native glue source: $required"
done

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

if grep -R -E -q '(^|[^A-Za-z])SwiftUI([^A-Za-z]|$)' "$SOURCES"; then
  fail "temporary SwiftUI terminal surfaces are forbidden"
fi

# NSTextView is correct for the Pane-local multiline composer editor. It must
# never become a terminal/Block rendering surface.
for forbidden_text_surface in \
  "$SOURCES/MetalSurfaceView.swift" \
  "$SOURCES/TerminalSurfaceHostView.swift" \
  "$SOURCES/BlockView.swift"; do
  [[ -f "$forbidden_text_surface" ]] || continue
  if grep -E -q '(^|[^A-Za-z])NSTextView([^A-Za-z]|$)' "$forbidden_text_surface"; then
    fail "NSTextView is forbidden in terminal/Block rendering surfaces: $forbidden_text_surface"
  fi
done
if [[ -f "$SOURCES/BlockView.swift" ]] && grep -q 'NSScrollView' "$SOURCES/BlockView.swift"; then
  fail "BlockView must not own nested output scrolling; the Pane transcript is the single normal-scroll owner"
fi

grep -q 'func applyPresentationMode' "$SOURCES/TerminalInputSurface.swift" \
  || fail "interactive Metal surface must apply the Flow/Raw/TUI presentation contract"
grep -q 'func inspectFlowPaint' "$SOURCES/MetalTerminalRenderer.swift" \
  || fail "Flow paint detection must inspect Metal instances/pixels, not XCUI glyph text"
grep -q 'flow-paint=' "$SOURCES/MetalSurfaceView.swift" \
  || fail "terminal-surface accessibility must publish flow-paint for XCUI"
grep -q 'TerminalSurfaceHostView' "$PROJECT/project.pbxproj" \
  || fail "permanent Metal terminal-surface host is missing from the native target"
if grep -q 'SeyalShellProductionFactory.make' "$SOURCES/AppDelegate.swift"; then
  fail "rejected SeyalShellProductionFactory must not be the supported launch path"
fi
if grep -q 'SeyalShellPreviewFactory.make' "$SOURCES/AppDelegate.swift"; then
  fail "rejected SeyalShellPreviewFactory must not be the supported launch path"
fi
if grep -q 'window.contentView = SeyalShellView' "$SOURCES/AppDelegate.swift"; then
  fail "rejected SeyalShellView must not be the supported launch path"
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
RUNTIME_DIR="$(getconf DARWIN_USER_TEMP_DIR)/seyal-runtime"
RUNTIME_SOCKET="${RUNTIME_DIR}/control.sock"

runtime_pid=""
cleanup_runtime() {
  if [[ -n "$runtime_pid" ]] && kill -0 "$runtime_pid" 2>/dev/null; then
    kill "$runtime_pid" 2>/dev/null || true
    wait "$runtime_pid" 2>/dev/null || true
  fi
  # Runtime performs bounded graceful cleanup of its canonical listener. Do
  # not start the next fixture while the old listener still owns the socket;
  # otherwise the new fixture reports AlreadyRunning nondeterministically.
  # Runtime may finish its child before the listener cleanup completes. Keep
  # the singleton boundary closed until the endpoint is actually gone.
  for _ in $(seq 1 200); do
    [[ ! -S "$RUNTIME_SOCKET" ]] && break
    sleep 0.025
  done
  runtime_pid=""
}
trap cleanup_runtime EXIT

run_pass8_native_metadata_case() {
  cleanup_runtime
  "$RUNTIME" /bin/sh -c "sleep 3" &
  runtime_pid=$!

  # Wait until the fixture Runtime owns the canonical endpoint before asking
  # the client to discover it. Without this barrier, a legitimate endpoint
  # absence can cause the production one-shot bundled-helper recovery path to
  # win the singleton race and make the fixture Runtime report AlreadyRunning.
  local ready=0
  for _ in $(seq 1 40); do
    if [[ -S "$RUNTIME_SOCKET" ]]; then
      ready=1
      break
    fi
    if ! kill -0 "$runtime_pid" 2>/dev/null; then
      break
    fi
    sleep 0.025
  done
  [[ "$ready" == "1" ]] || fail "fixture Runtime did not bind its canonical endpoint"

  local passed=0
  for _ in $(seq 1 20); do
    if "$BINARY" --pass8-native-metadata-self-test; then
      passed=1
      break
    fi
    if ! kill -0 "$runtime_pid" 2>/dev/null; then
      break
    fi
    sleep 0.05
  done
  [[ "$passed" == "1" ]] || fail "Pass 8 real Runtime-to-Swift metadata seam failed"
  cleanup_runtime
}

run_live_renderer_case() {
  local command="$1"
  shift
  cleanup_runtime
  "$RUNTIME" /bin/sh -c "$command" &
  runtime_pid=$!

  local ready=0
  for _ in $(seq 1 40); do
    if [[ -S "$RUNTIME_SOCKET" ]]; then
      ready=1
      break
    fi
    if ! kill -0 "$runtime_pid" 2>/dev/null; then
      break
    fi
    sleep 0.025
  done
  [[ "$ready" == "1" ]] || fail "live fixture Runtime did not bind its canonical endpoint"

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
  # The child can exit before Runtime has removed its listener. Reuse the
  # same teardown path while ownership is still tracked so the next fixture
  # cannot race a draining singleton.
  cleanup_runtime
}

run_pass8_native_metadata_case
run_live_renderer_case "printf 'SEYAL-LIVE'; sleep 1"
run_live_renderer_case "printf '\033[?1049hALT-LIVE'; sleep 1" --expect-alternate

trap - EXIT
cleanup_runtime

echo "[seyal macOS test] Pass 8 real Runtime-to-Swift metadata acceptance passed."
echo "[seyal macOS test] AppKit + Candidate-D + permanent Metal renderer acceptance passed."
echo "[seyal macOS test] Swift + AppKit + Metal + UI shell scaffold acceptance passed."
