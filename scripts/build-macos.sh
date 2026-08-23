#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "[seyal macOS build] skipped: Seyal.app builds only on macOS."
  exit 0
fi

bash scripts/check-macos-toolchain.sh

DERIVED_DATA="${ROOT}/target/macos-derived-data"

xcodebuild \
  -project macos/Seyal/Seyal.xcodeproj \
  -scheme Seyal \
  -configuration Debug \
  -derivedDataPath "$DERIVED_DATA" \
  CODE_SIGNING_ALLOWED=NO \
  build

printf '[seyal macOS build] built %s\n' "${DERIVED_DATA}/Build/Products/Debug/Seyal.app"
