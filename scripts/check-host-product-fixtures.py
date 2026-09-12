#!/usr/bin/env python3
"""Fail if native host or tests reconstruct Swift portable product fixtures."""

from __future__ import annotations

import os
import pathlib
import re
import sys

DEFAULT_ROOT = pathlib.Path(__file__).resolve().parents[1]
ROOT = pathlib.Path(os.environ.get("SEYAL_VALIDATION_ROOT", DEFAULT_ROOT)).resolve()
SOURCES = ROOT / "macos" / "Seyal" / "Sources"
TESTS = ROOT / "macos" / "Seyal" / "Tests"

FORBIDDEN_TOKENS = (
    "SeyalShellState",
    "SeyalShellView",
    "SeyalShellPreviewFactory",
    "SeyalShellProductionFactory",
    "SeyalShellModel",
    "PanePresentationSession",
)

TEST_DEFINITIONS = re.compile(
    r"\b(?:struct|class|enum|func)\s+(?:SeyalShell\w*|makePreview|makeProduction|InspectorMode|LeftPanelMode)\b"
)


def swift_files(root: pathlib.Path) -> list[pathlib.Path]:
    if not root.is_dir():
        return []
    return sorted(path for path in root.rglob("*.swift") if path.is_file())


def main() -> int:
    errors: list[str] = []
    if not SOURCES.is_dir():
        errors.append("missing macos/Seyal/Sources")
    for path in swift_files(SOURCES):
        text = path.read_text(encoding="utf-8")
        rel = path.relative_to(ROOT)
        for token in FORBIDDEN_TOKENS:
            if token in text:
                errors.append(f"{rel} reconstructs portable product fixture token {token!r}")
    for path in swift_files(TESTS):
        text = path.read_text(encoding="utf-8")
        rel = path.relative_to(ROOT)
        if TEST_DEFINITIONS.search(text):
            errors.append(f"{rel} defines a Swift product fixture reducer")
    if errors:
        print("Host product-fixture policy failed:", file=sys.stderr)
        for error in errors:
            print(f"  - {error}", file=sys.stderr)
        return 1
    print("Host product-fixture policy passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
