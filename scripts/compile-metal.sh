#!/usr/bin/env bash
set -euo pipefail

: "${SRCROOT:?Xcode must provide SRCROOT}"
: "${DERIVED_FILE_DIR:?Xcode must provide DERIVED_FILE_DIR}"
: "${TARGET_BUILD_DIR:?Xcode must provide TARGET_BUILD_DIR}"
: "${UNLOCALIZED_RESOURCES_FOLDER_PATH:?Xcode must provide UNLOCALIZED_RESOURCES_FOLDER_PATH}"

# Xcode exports TOOLCHAINS and SDKROOT for its own tool wrappers. On macOS
# versions where Metal is mounted as a cryptex, those variables force xcrun
# back to XcodeDefault/usr/bin/metal, which can be present but unable to see
# the installed Metal component. Resolve the installed component directly.
unset TOOLCHAINS SDKROOT || true

metal="$(xcrun --find metal)"
metal_dir="$(cd "$(dirname "$metal")" && pwd)"
metallib="$metal_dir/metallib"
if [[ ! -x "$metallib" ]]; then
    echo "Metal library linker is unavailable beside xcrun metal: $metallib" >&2
    exit 1
fi

source_file="$SRCROOT/Sources/TerminalShaders.metal"
module_cache="$DERIVED_FILE_DIR/metal-module-cache"
intermediate="$DERIVED_FILE_DIR/TerminalShaders.air"
bundle_resources="$TARGET_BUILD_DIR/$UNLOCALIZED_RESOURCES_FOLDER_PATH"

mkdir -p "$module_cache" "$bundle_resources"
"$metal" \
    -fmodules-cache-path="$module_cache" \
    -c "$source_file" \
    -o "$intermediate"
"$metallib" "$intermediate" -o "$bundle_resources/default.metallib"
