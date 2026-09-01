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

for required in \
  SeyalDesignTokens.swift \
  SeyalShellModel.swift \
  BlockView.swift \
  PaneComposerShellView.swift \
  TerminalSurfaceHostView.swift \
  SeyalShellView.swift \
  SeyalShellPreviewFactory.swift; do
  [[ -f "$SOURCES/$required" ]] || fail "missing native UI shell source: $required"
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
  if grep -E -q '(^|[^A-Za-z])NSTextView([^A-Za-z]|$)' "$forbidden_text_surface"; then
    fail "NSTextView is forbidden in terminal/Block rendering surfaces: $forbidden_text_surface"
  fi
done
grep -q 'NSTextView' "$SOURCES/PaneComposerShellView.swift" \
  || fail "Pane composer preview must exercise a real multiline native editor"

if grep -q 'NSScrollView' "$SOURCES/BlockView.swift"; then
  fail "BlockView must not own nested output scrolling; the Pane transcript is the single normal-scroll owner"
fi

grep -q 'private func makeTranscript(paneID: String) -> PaneTranscriptView' "$SOURCES/SeyalShellView.swift" \
  || fail "UI shell must keep Pane-owned transcript scrolling explicit"
grep -q 'final class PaneTranscriptView: NSScrollView' "$SOURCES/CommandBlockBodyView.swift" \
  || fail "Pane transcript must remain the single normal-scroll owner"
grep -q 'NSSegmentedControl' "$SOURCES/SeyalShellView.swift" \
  || fail "compact Workspaces/Tabs switcher is missing from the frozen left-panel model"
grep -q 'toggle-left-sidebar' "$SOURCES/SeyalShellView.swift" \
  || fail "left context panel must have a functional hide/reopen control"
grep -q 'toggle-inspector' "$SOURCES/SeyalShellView.swift" \
  || fail "Inspector must have a functional hide/reopen control"
grep -Fq 'private func makeInspectorRail() -> NSView' "$SOURCES/SeyalShellView.swift" \
  || fail "frozen Inspector vertical mode rail builder is missing"
grep -Fq 'setAccessibilityIdentifier("inspector-mode.\(mode.rawValue)")' "$SOURCES/SeyalShellView.swift" \
  || fail "Inspector rail modes must expose deterministic dynamic accessibility identifiers"
grep -q 'pane.split.' "$SOURCES/SeyalShellView.swift" \
  || fail "Pane-local split control is missing"
grep -q 'pane.close.' "$SOURCES/SeyalShellView.swift" \
  || fail "Pane-local close control is missing"
grep -q 'inspector.trailingAnchor.constraint(equalTo: trailingAnchor)' "$SOURCES/SeyalShellView.swift" \
  || fail "Inspector must remain pinned to the shell trailing edge"
grep -q 'NSMenuItem' "$SOURCES/SeyalShellPreviewFactory.swift" \
  || fail "native shell navigation shortcuts must be discoverable AppKit menu commands"
grep -q 'keyEquivalentModifierMask' "$SOURCES/SeyalShellPreviewFactory.swift" \
  || fail "native shell navigation shortcuts must use AppKit key equivalents"
grep -q 'closeFocusedContext' "$SOURCES/SeyalShellPreviewFactory.swift" \
  || fail "Command-W must route through hierarchical Pane/Tab/Window close semantics"
grep -q 'static func closeTarget' "$SOURCES/SeyalShellPreviewFactory.swift" \
  || fail "hierarchical close target policy must remain explicit and testable"
grep -q 'SeyalShortcutHintOverlay' "$SOURCES/SeyalShellPreviewFactory.swift" \
  || fail "Command-hold shortcut hint overlay is missing"
grep -q 'intentionalHoldDelay: TimeInterval = 0.30' "$SOURCES/SeyalShellPreviewFactory.swift" \
  || fail "shortcut hints must require the intentional 300 ms Command-only hold"
grep -q 'addLocalMonitorForEvents(matching: .flagsChanged)' "$SOURCES/SeyalShellPreviewFactory.swift" \
  || fail "shortcut hint monitor must observe modifier transitions"
grep -q 'addLocalMonitorForEvents(matching: .keyDown)' "$SOURCES/SeyalShellPreviewFactory.swift" \
  || fail "shortcut hints must cancel when another key is pressed"
if grep -q 'override func keyDown' "$SOURCES/SeyalShellPreviewFactory.swift"; then
  fail "shell navigation shortcuts must not override raw keyDown handling"
fi
grep -q 'TerminalSurfaceHostView' "$PROJECT/project.pbxproj" \
  || fail "permanent Metal terminal-surface host is missing from the native target"
grep -q -- '--ui-shell-preview' "$SOURCES/AppDelegate.swift" \
  || fail "UI shell preview must remain behind an explicit launch path"
grep -q 'buildConfiguration == "Debug"' "$SOURCES/AppDelegate.swift" \
  || fail "UI shell preview must be runtime-gated to Debug builds before Pass 6"
grep -q '#if DEBUG' "$SOURCES/SeyalShellPreviewFactory.swift" \
  || fail "preview fixtures must remain compiled only in Debug builds before Pass 6"
grep -q 'SWIFT_ACTIVE_COMPILATION_CONDITIONS='"'"'DEBUG $(inherited)'"'"'' scripts/build-macos.sh \
  || fail "canonical Debug build must compile the preview-only shell fixtures"
grep -q 'SWIFT_ACTIVE_COMPILATION_CONDITIONS='"'"'DEBUG $(inherited)'"'"'' scripts/test-macos-ui.sh \
  || fail "native UI tests must exercise the same Debug preview compilation path"

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

run_pass8_native_metadata_case
run_live_renderer_case "printf 'SEYAL-LIVE'; sleep 1"
run_live_renderer_case "printf '\033[?1049hALT-LIVE'; sleep 1" --expect-alternate

trap - EXIT
cleanup_runtime

echo "[seyal macOS test] Pass 8 real Runtime-to-Swift metadata acceptance passed."
echo "[seyal macOS test] AppKit + Candidate-D + permanent Metal renderer acceptance passed."
echo "[seyal macOS test] Swift + AppKit + Metal + UI shell scaffold acceptance passed."
