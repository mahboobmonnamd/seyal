#!/usr/bin/env python3
"""Reject portable product authority in the new thin macOS host."""

from __future__ import annotations

import os
import pathlib
import re
import sys

DEFAULT_ROOT = pathlib.Path(__file__).resolve().parents[1]
ROOT = pathlib.Path(os.environ.get("SEYAL_VALIDATION_ROOT", DEFAULT_ROOT)).resolve()
SOURCES = ROOT / "macos/Seyal/Sources"

# Leftover deprecated product-shell files may still mention these tokens.
# New host/glue files must not grow a second product model.
DEPRECATED_ALLOWLIST = {
    "SeyalShellModel.swift",
    "SeyalShellView.swift",
    "SeyalShellChrome.swift",
    "SeyalShellChromeControls.swift",
    "SeyalShellPaneLayout.swift",
    "SeyalShellTranscriptCoordinator.swift",
    "SeyalShellPreviewFactory.swift",
    "SeyalShellProductionFactory.swift",
    "SeyalLeftContextPressCoordinator.swift",
    "PaneComposerShellView.swift",
    "BlockView.swift",
    "CommandBlockBodyView.swift",
    "PanePresentationContract.swift",
    "RuntimeLifecycleRecoveryCoordinator.swift",
    # KEEP_NATIVE_GLUE that still mentions leftover product types; #883 must
    # not grow new product tokens here either, but renaming these files is
    # not a bypass because the new host files are always scanned.
    "RustDisplayBridge.swift",
    "TerminalInputSurface.swift",
}

PRODUCT_AUTHORITY = (
    "SeyalShellState",
    "SeyalShellPreviewFactory",
    "SeyalShellProductionFactory",
    "PanePresentationSession",
    "ComposerRequestCorrelation",
    "enum InspectorMode",
    "enum LeftPanelMode",
    "func appendCommand(",
)


def main() -> int:
    if not SOURCES.is_dir():
        print("Thin-Swift boundary failed: missing macos/Seyal/Sources", file=sys.stderr)
        return 1
    errors: list[str] = []
    required_host = {
        "SeyalThinHostView.swift",
        "SeyalProductBridge.swift",
        "AppDelegate.swift",
    }
    present = {path.name for path in SOURCES.glob("*.swift")}
    for name in required_host:
        if name not in present:
            errors.append(f"missing required thin-host source: {name}")
    for path in sorted(SOURCES.glob("*.swift")):
        if path.name in DEPRECATED_ALLOWLIST:
            continue
        text = path.read_text(encoding="utf-8")
        rel = path.relative_to(ROOT)
        for token in PRODUCT_AUTHORITY:
            if token in text:
                errors.append(f"{rel} introduces portable product authority token {token!r}")
        if re.search(r"enum\s+Workspace(Id|Mode|State)\b", text):
            errors.append(f"{rel} introduces a Swift Workspace product enum")
    if errors:
        print("Thin-Swift boundary failed:", file=sys.stderr)
        for error in errors:
            print(f"  - {error}", file=sys.stderr)
        return 1
    print("Thin-Swift boundary passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
