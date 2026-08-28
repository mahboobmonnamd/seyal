#!/usr/bin/env python3
import os
import pathlib
import subprocess
import sys

DEFAULT_ROOT = pathlib.Path(__file__).resolve().parents[1]
ROOT = pathlib.Path(os.environ.get("SEYAL_VALIDATION_ROOT", DEFAULT_ROOT)).resolve()

REQUIRED = [
    ROOT / "macos/Seyal/Tests/SeyalTests/SeyalShellComponentTests.swift",
    ROOT / "macos/Seyal/Tests/SeyalUITests/SeyalShellUITests.swift",
    ROOT / "macos/Seyal/Seyal.xcodeproj/xcshareddata/xcschemes/Seyal.xcscheme",
    ROOT / "scripts/test-macos-ui.sh",
]

missing = [str(path.relative_to(ROOT)) for path in REQUIRED if not path.exists()]
if missing:
    print("UI test policy failed: missing required native test assets:", file=sys.stderr)
    for path in missing:
        print(f"  - {path}", file=sys.stderr)
    sys.exit(1)

project = (ROOT / "macos/Seyal/Seyal.xcodeproj/project.pbxproj").read_text()
for target in ("SeyalTests", "SeyalUITests"):
    if target not in project:
        print(f"UI test policy failed: Xcode project is missing {target}", file=sys.stderr)
        sys.exit(1)

scheme = REQUIRED[2].read_text()
for target in ("SeyalTests.xctest", "SeyalUITests.xctest"):
    if target not in scheme:
        print(f"UI test policy failed: shared scheme does not execute {target}", file=sys.stderr)
        sys.exit(1)

base_ref = os.environ.get("GITHUB_BASE_REF", "").strip()
if not base_ref or "SEYAL_VALIDATION_ROOT" in os.environ:
    print("UI test policy passed (repository assets validated; PR diff enforcement not requested).")
    sys.exit(0)

try:
    merge_base = subprocess.check_output(
        ["git", "merge-base", f"origin/{base_ref}", "HEAD"],
        cwd=ROOT,
        text=True,
    ).strip()
    changed = subprocess.check_output(
        ["git", "diff", "--name-only", f"{merge_base}...HEAD"],
        cwd=ROOT,
        text=True,
    ).splitlines()
except subprocess.CalledProcessError as error:
    print(f"UI test policy failed while determining PR diff: {error}", file=sys.stderr)
    sys.exit(1)

ui_sources = [
    path for path in changed
    if path.startswith("macos/Seyal/Sources/") and path.endswith(".swift")
]
if not ui_sources:
    print("UI test policy passed (no native UI source changes).")
    sys.exit(0)

unit_changes = [
    path for path in changed
    if path.startswith("macos/Seyal/Tests/SeyalTests/") and path.endswith(".swift")
]
if not unit_changes:
    print("UI test policy failed: native UI source changed without XCTest component coverage in the same PR.", file=sys.stderr)
    sys.exit(1)

material_ui_sources = [
    path for path in ui_sources
    if path.endswith("View.swift")
    or path.endswith("AppDelegate.swift")
    or path.endswith("Main.swift")
]
ui_test_changes = [
    path for path in changed
    if path.startswith("macos/Seyal/Tests/SeyalUITests/") and path.endswith(".swift")
]
if material_ui_sources and not ui_test_changes:
    print("UI test policy failed: visible/interactive macOS UI changed without XCUIAutomation coverage in the same PR.", file=sys.stderr)
    sys.exit(1)

print(
    f"UI test policy passed ({len(ui_sources)} UI source change(s), "
    f"{len(unit_changes)} XCTest change(s), {len(ui_test_changes)} XCUI change(s))."
)
