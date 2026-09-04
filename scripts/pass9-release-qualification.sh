#!/usr/bin/env bash
# Pass 9 release-qualification orchestrator for Issue #736.
# Reuses the production recovery soak path (same topology as merge-acceptance)
# across the SPEC-009 §16 matrix: 5 cohorts × 2 modes × 2 geometries, with a
# fresh Runtime process per cohort. Emits seyal.pass9.production-budget.v1.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

CYCLES="${SEYAL_PASS9_CYCLES:-100}"
WARMUP="${SEYAL_PASS9_WARMUP:-20}"
OUT_DIR="${SEYAL_PASS9_OUT_DIR:-$ROOT/docs/evidence}"
COMMIT="${SEYAL_PASS9_EXPECTED_HEAD:-$(git rev-parse HEAD)}"
DRY_RUN="${SEYAL_PASS9_DRY_RUN:-0}"
GEOMETRIES="${SEYAL_PASS9_GEOMETRIES:-120x40 80x24}"
MODES="${SEYAL_PASS9_MODES:-graceful_detach abrupt_socket_loss}"
COHORTS="${SEYAL_PASS9_COHORTS:-1 2 3 4 5}"
PASS8_DELTA="${SEYAL_PASS9_PASS8_DELTA_PERCENT:-}"
PASS8_EXPLAIN="${SEYAL_PASS9_PASS8_EXPLANATION:-}"

if [[ "$DRY_RUN" == "1" ]]; then
  CYCLES="${SEYAL_PASS9_CYCLES:-2}"
  WARMUP="${SEYAL_PASS9_WARMUP:-1}"
  COHORTS="${SEYAL_PASS9_COHORTS:-1}"
  GEOMETRIES="${SEYAL_PASS9_GEOMETRIES:-120x40}"
  MODES="${SEYAL_PASS9_MODES:-graceful_detach}"
  echo "[pass9-release-qualification] DRY_RUN=1 cycles=$CYCLES warmup=$WARMUP (validator skipped)"
fi

mkdir -p "$OUT_DIR"
PARTIAL_DIR="$OUT_DIR/pass9-release-partials-${COMMIT:0:12}"
rm -rf "$PARTIAL_DIR"
mkdir -p "$PARTIAL_DIR"
ARTIFACT="$OUT_DIR/pass9-release-qualification-${COMMIT:0:12}.json"
REPORT="$OUT_DIR/pass9-release-qualification-${COMMIT:0:12}.md"
PACKAGING="$OUT_DIR/pass9-release-packaging-${COMMIT:0:12}.md"

echo "[pass9-release-qualification] building Debug app + Runtime helper"
unset CARGO_TARGET_DIR
export CARGO_TARGET_DIR="$ROOT/target"
bash scripts/build-macos.sh

APP="$ROOT/target/macos-derived-data/Build/Products/Debug/Seyal.app"
BIN="$APP/Contents/MacOS/Seyal"
RUNTIME_HELPER="$APP/Contents/Helpers/seyal-runtime"
[[ -x "$BIN" ]] || { echo "missing app binary: $BIN" >&2; exit 1; }
[[ -x "$RUNTIME_HELPER" ]] || { echo "missing bundled runtime: $RUNTIME_HELPER" >&2; exit 1; }

echo "[pass9-release-qualification] retaining packaging inspection"
{
  echo "# Pass 9 release packaging inspection"
  echo
  echo "- **Commit:** \`$COMMIT\`"
  echo "- **Helper path:** \`Seyal.app/Contents/Helpers/seyal-runtime\`"
  echo "- **Direct no-shell launch:** exercised by this orchestrator"
  echo
  echo '## codesign -dv --verbose=4 (helper)'
  echo '```'
  codesign -dv --verbose=4 "$RUNTIME_HELPER" 2>&1 || true
  echo '```'
  echo
  echo '## codesign --display --entitlements - (helper)'
  echo '```'
  codesign --display --entitlements :- "$RUNTIME_HELPER" 2>&1 || true
  echo '```'
  echo
  echo '## codesign --verify --strict --deep (app)'
  echo '```'
  codesign --verify --strict --deep "$APP" 2>&1 || true
  echo '```'
} >"$PACKAGING"

echo "[pass9-release-qualification] proving Release trust rules reject ad-hoc helpers"
if [[ "$DRY_RUN" == "1" ]]; then
  echo "[pass9-release-qualification] DRY_RUN skips XCTest trust step (use full run for Release trust proof)"
else
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
fi

PARTIALS=()
for geometry in $GEOMETRIES; do
  for mode in $MODES; do
    for cohort in $COHORTS; do
      RUNTIME_LOG="$(mktemp -t seyal-pass9-rq-runtime)"
      "$RUNTIME_HELPER" /bin/zsh >"$RUNTIME_LOG" 2>&1 &
      RUNTIME_PID=$!
      cleanup_runtime() {
        if kill -0 "$RUNTIME_PID" 2>/dev/null; then
          kill "$RUNTIME_PID" 2>/dev/null || true
          wait "$RUNTIME_PID" 2>/dev/null || true
        fi
        rm -f "$RUNTIME_LOG"
      }
      trap cleanup_runtime EXIT
      sleep 0.25

      PARTIAL="$PARTIAL_DIR/${mode}-${geometry}-c${cohort}.json"
      echo "[pass9-release-qualification] cohort mode=$mode geometry=$geometry cohort=$cohort"
      SEYAL_PASS9_RUNTIME_PID="$RUNTIME_PID" \
      SEYAL_PASS9_EXPECTED_HEAD="$COMMIT" \
        "$BIN" \
        --pass9-release-qualification \
        --cycles="$CYCLES" \
        --warmup="$WARMUP" \
        --geometry="$geometry" \
        --mode="$mode" \
        --cohort="$cohort" \
        --commit="$COMMIT" \
        --output="$PARTIAL"
      PARTIALS+=("$PARTIAL")
      cleanup_runtime
      trap - EXIT
    done
  done
done

PASS8_ARGS=()
if [[ -n "$PASS8_DELTA" ]]; then
  PASS8_ARGS+=(--pass8-delta-percent "$PASS8_DELTA")
  if [[ -n "$PASS8_EXPLAIN" ]]; then
    PASS8_ARGS+=(--pass8-explanation "$PASS8_EXPLAIN")
  fi
elif [[ "$DRY_RUN" == "1" ]]; then
  # Dry-run tooling path: record a zero paired delta so the merge artifact is
  # schema-complete. Full runs collect real Pass 8 attribution below.
  PASS8_ARGS+=(--pass8-delta-percent 0 --pass8-cohorts 5)
else
  echo "[pass9-release-qualification] collecting Pass 8 paired attribution (pass7_input_resize)"
  PASS8_LOG="$(mktemp -t seyal-pass9-pass8)"
  set +e
  cargo bench -p seyal-client --bench pass7_input_resize --features benchmark-instrumentation --locked \
    >"$PASS8_LOG" 2>&1
  PASS8_STATUS=$?
  set -e
  if [[ "$PASS8_STATUS" -ne 0 ]]; then
    echo "[pass9-release-qualification] Pass 8 bench failed; see $PASS8_LOG" >&2
    tail -50 "$PASS8_LOG" >&2 || true
    exit "$PASS8_STATUS"
  fi
  PASS8_DELTA_PARSED="$(
    python3 - <<'PY' "$PASS8_LOG"
import re, sys
text = open(sys.argv[1], encoding="utf-8", errors="replace").read()
matches = re.findall(r"paired_delta_median_percent=([-+0-9.]+)", text)
if not matches:
    raise SystemExit("pass8 paired_delta_median_percent not found")
print(matches[-1])
PY
  )"
  PASS8_ARGS+=(--pass8-delta-percent "$PASS8_DELTA_PARSED" --pass8-cohorts 5)
  if python3 -c "import sys; sys.exit(0 if abs(float('$PASS8_DELTA_PARSED')) > 5 else 1)"; then
    PASS8_ARGS+=(--pass8-explanation "Measured pass7_input_resize paired_delta_median_percent=${PASS8_DELTA_PARSED} on exact head ${COMMIT}")
  fi
  cp "$PASS8_LOG" "$OUT_DIR/pass9-pass8-attribution-${COMMIT:0:12}.log"
  rm -f "$PASS8_LOG"
fi

python3 scripts/merge-pass9-release-qualification.py \
  --commit "$COMMIT" \
  --output "$ARTIFACT" \
  "${PASS8_ARGS[@]}" \
  "${PARTIALS[@]}"

if [[ "$DRY_RUN" != "1" || "${SEYAL_PASS9_FORCE_VALIDATE:-0}" == "1" ]]; then
  python3 scripts/check-pass9-production-budget.py --expected-head "$COMMIT" "$ARTIFACT"
fi

INPUT_AX="$OUT_DIR/pass9-input-accessibility-${COMMIT:0:12}.json"
INPUT_AX_MD="$OUT_DIR/pass9-input-accessibility-${COMMIT:0:12}.md"
if [[ "$DRY_RUN" == "1" ]]; then
  echo "[pass9-release-qualification] DRY_RUN skips input/accessibility Track C"
else
  echo "[pass9-release-qualification] running input/accessibility Track C"
  "$BIN" --pass9-input-accessibility-qualification --output "$INPUT_AX"
  {
    echo "# Pass 9 input / accessibility qualification"
    echo
    echo "- **Issue:** #736"
    echo "- **Exact production head:** \`$COMMIT\`"
    echo "- **Artifact:** \`$(basename "$INPUT_AX")\`"
    echo "- **Surface:** production \`InteractiveMetalSurfaceView\` as \`NSTextInputClient\`"
    echo "- **VoiceOver:** discoverability via accessibility role/label/value (system VoiceOver audio not enabled)"
    echo
    echo '```json'
    cat "$INPUT_AX"
    echo '```'
  } >"$INPUT_AX_MD"
  echo "[pass9-release-qualification] retained $INPUT_AX"
  echo "[pass9-release-qualification] retained $INPUT_AX_MD"
fi

cat >"$REPORT" <<EOF
# Pass 9 release-qualification evidence

- **Issue:** #736
- **Exact production head under test:** \`$COMMIT\`
- **Artifact:** \`$(basename "$ARTIFACT")\`
- **Packaging:** \`$(basename "$PACKAGING")\`
- **Input/accessibility:** \`$(basename "$INPUT_AX")\` (skipped on dry-run)
- **Modes:** $MODES
- **Geometries:** $GEOMETRIES
- **Cohorts:** $COHORTS
- **Cycles:** $CYCLES each after $WARMUP warmups
- **Topology:** Debug \`RustDisplayBridge\` + \`RuntimeLifecycleRecoveryCoordinator\` + \`MetalTerminalRenderer\` prepare/release (same boundary as merge-acceptance; not full AppKit present)
- **Abrupt fault:** \`socket_shutdown_owned_disconnect\`
- **Fresh Runtime:** one Runtime helper process per cohort
- **Validator:** \`python3 scripts/check-pass9-production-budget.py --expected-head $COMMIT $ARTIFACT\`
- **Dry run:** $DRY_RUN

Independent reviews remain required. This report does not self-certify release qualification.
EOF

echo "[pass9-release-qualification] retained $ARTIFACT"
echo "[pass9-release-qualification] retained $REPORT"
echo "[pass9-release-qualification] retained $PACKAGING"
