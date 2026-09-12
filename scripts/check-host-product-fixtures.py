#!/usr/bin/env python3
"""Fail if the thin host reconstructs Swift product fixtures."""

from __future__ import annotations

import os
import pathlib
import sys

DEFAULT_ROOT = pathlib.Path(__file__).resolve().parents[1]
ROOT = pathlib.Path(os.environ.get("SEYAL_VALIDATION_ROOT", DEFAULT_ROOT)).resolve()

HOST_FILES = [
    ROOT / "macos/Seyal/Sources/AppDelegate.swift",
    ROOT / "macos/Seyal/Sources/Main.swift",
    ROOT / "macos/Seyal/Sources/SeyalThinHostView.swift",
    ROOT / "macos/Seyal/Sources/SeyalProductBridge.swift",
]

FORBIDDEN = (
    "SeyalShellState",
    "SeyalShellPreviewFactory",
    "SeyalShellProductionFactory",
    "SeyalShellState.makePreview",
    "SeyalShellState.makeProduction",
    "SeyalShellView",
)


def main() -> int:
    errors: list[str] = []
    for path in HOST_FILES:
        rel = path.relative_to(ROOT)
        if not path.is_file():
            errors.append(f"missing thin-host source: {rel}")
            continue
        text = path.read_text(encoding="utf-8")
        for token in FORBIDDEN:
            if token in text:
                errors.append(f"{rel} must not mention {token}")
    if errors:
        print("Host product-fixture policy failed:", file=sys.stderr)
        for error in errors:
            print(f"  - {error}", file=sys.stderr)
        return 1
    print("Host product-fixture policy passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
