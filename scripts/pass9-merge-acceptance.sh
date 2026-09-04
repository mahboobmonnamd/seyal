#!/usr/bin/env bash
# Reusable Pass 9 merge-acceptance orchestrator for Issue #735.
# Builds production topology artifacts, runs the native soak CLI against a live
# Runtime helper, validates the emitted JSON, and retains evidence paths.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

CYCLES="${SEYAL_PASS9_CYCLES:-100}"
WARMUP="${SEYAL_PASS9_WARMUP:-5}"
GEOMETRY="${SEYAL_PASS9_GEOMETRY:-120x40}"
OUT_DIR="${SEYAL_PASS9_OUT_DIR:-$ROOT/docs/evidence}"
COMMIT="${SEYAL_PASS9_EXPECTED_HEAD:-$(git rev-parse HEAD)}"

mkdir -p "$OUT_DIR"
ARTIFACT="$OUT_DIR/pass9-merge-acceptance-${COMMIT:0:12}.json"
REPORT="$OUT_DIR/pass9-merge-acceptance-${COMMIT:0:12}.md"

echo "[pass9-merge-acceptance] building Debug app + Runtime helper"
unset CARGO_TARGET_DIR
export CARGO_TARGET_DIR="$ROOT/target"
bash scripts/build-macos.sh

APP="$ROOT/target/macos-derived-data/Build/Products/Debug/Seyal.app"
BIN="$APP/Contents/MacOS/Seyal"
RUNTIME_HELPER="$APP/Contents/Helpers/seyal-runtime"
[[ -x "$BIN" ]] || { echo "missing app binary: $BIN" >&2; exit 1; }
[[ -x "$RUNTIME_HELPER" ]] || { echo "missing bundled runtime: $RUNTIME_HELPER" >&2; exit 1; }

echo "[pass9-merge-acceptance] proving Release trust rules reject ad-hoc helpers"
TRUST_DERIVED="$ROOT/target/macos-pass9-trust-tests"
rm -rf "$TRUST_DERIVED"
xcodebuild \
  -project macos/Seyal/Seyal.xcodeproj \
  -scheme Seyal \
  -configuration Debug \
  -destination 'platform=macOS' \
  -derivedDataPath "$TRUST_DERIVED" \
  ARCHS=arm64 \
  ONLY_ACTIVE_ARCH=YES \
  CODE_SIGNING_ALLOWED=NO \
  CODE_SIGNING_REQUIRED=NO \
  SWIFT_ACTIVE_COMPILATION_CONDITIONS='DEBUG $(inherited)' \
  build-for-testing
PRODUCTS="$TRUST_DERIVED/Build/Products/Debug"
codesign --force --deep --sign - "$PRODUCTS/Seyal.app"
codesign --force --deep --sign - "$PRODUCTS/SeyalUITests-Runner.app" 2>/dev/null || true
xcodebuild \
  -project macos/Seyal/Seyal.xcodeproj \
  -scheme Seyal \
  -configuration Debug \
  -destination 'platform=macOS' \
  -derivedDataPath "$TRUST_DERIVED" \
  ARCHS=arm64 \
  ONLY_ACTIVE_ARCH=YES \
  -only-testing:SeyalTests/SeyalShellComponentTests/testReleaseTrustRulesRejectAdHocHelpers \
  test-without-building

RUNTIME_LOG="$(mktemp -t seyal-pass9-runtime)"
"$RUNTIME_HELPER" /bin/zsh >"$RUNTIME_LOG" 2>&1 &
RUNTIME_PID=$!
cleanup() {
  if kill -0 "$RUNTIME_PID" 2>/dev/null; then
    kill "$RUNTIME_PID" 2>/dev/null || true
    wait "$RUNTIME_PID" 2>/dev/null || true
  fi
  rm -f "$RUNTIME_LOG"
}
trap cleanup EXIT

# Give the singleton listener a moment to bind before the soak opens.
sleep 0.25

echo "[pass9-merge-acceptance] running soak cycles=$CYCLES geometry=$GEOMETRY commit=$COMMIT"
SEYAL_PASS9_RUNTIME_PID="$RUNTIME_PID" \
SEYAL_PASS9_EXPECTED_HEAD="$COMMIT" \
  "$BIN" \
  --pass9-merge-acceptance \
  --cycles="$CYCLES" \
  --warmup="$WARMUP" \
  --geometry="$GEOMETRY" \
  --commit="$COMMIT" \
  --output="$ARTIFACT"

python3 scripts/check-pass9-merge-acceptance.py --expected-head "$COMMIT" "$ARTIFACT"

cat >"$REPORT" <<EOF
# Pass 9 merge-acceptance evidence

- **Issue:** #735
- **Implementation PR:** #734
- **Exact production head under test:** \`$COMMIT\`
- **Artifact:** \`$(basename "$ARTIFACT")\`
- **Modes:** graceful_detach, abrupt_socket_loss (\`socket_shutdown_owned_disconnect\` fault injection; not GUI-process death)
- **Cycles:** $CYCLES each after $WARMUP warmups
- **Geometry:** $GEOMETRY (applied via \`proposeGeometry\` + frame observe)
- **Topology:** Debug \`RustDisplayBridge\` + \`RuntimeLifecycleRecoveryCoordinator\` + \`MetalTerminalRenderer\` update/release path (same prepare boundary as \`MetalSurfaceView.consumeBridgeFrame\`; not full AppKit window/CAMetalLayer present)
- **Release trust:** \`testReleaseTrustRulesRejectAdHocHelpers\` executed by this orchestrator
- **Validator:** \`python3 scripts/check-pass9-merge-acceptance.py --expected-head $COMMIT $ARTIFACT\`

Independent implementation/architecture/security/performance/accessibility reviews remain required before merge of #734. This report is evidence only; it does not self-certify those gates. Merging acceptance PR #743 must not be treated as an automatic merge of #734.
EOF

echo "[pass9-merge-acceptance] retained $ARTIFACT"
echo "[pass9-merge-acceptance] retained $REPORT"
