#!/usr/bin/env python3
from __future__ import annotations

import os
import sys
from pathlib import Path
import tomllib

ROOT = Path(os.environ.get("SEYAL_VALIDATION_ROOT", Path(__file__).resolve().parents[1])).resolve()
RULES = {
    "seyal-terminal": {"seyal-workspace", "seyal-exec", "seyal-runtime", "seyal-render"},
    "seyal-exec": {"seyal-render", "seyal-runtime", "seyal-workspace"},
    "seyal-runtime": {"seyal-render", "seyal-workspace"},
    "seyal-workspace": {"seyal-exec", "seyal-runtime", "seyal-render"},
    "seyal-render": {"seyal-terminal", "seyal-exec", "seyal-runtime", "seyal-workspace"},
}

errors: list[str] = []
crates = ROOT / "crates"
if crates.exists():
    for manifest in crates.glob("*/Cargo.toml"):
        data = tomllib.loads(manifest.read_text(encoding="utf-8"))
        name = data.get("package", {}).get("name")
        if not name or name not in RULES:
            continue
        dependencies: set[str] = set()
        for section in ("dependencies", "build-dependencies"):
            dependencies.update(data.get(section, {}).keys())
        for target in data.get("target", {}).values():
            for section in ("dependencies", "build-dependencies"):
                dependencies.update(target.get(section, {}).keys())
        forbidden = sorted(dependencies & RULES[name])
        if forbidden:
            errors.append(f"{name} has forbidden dependencies: {', '.join(forbidden)}")

if errors:
    print("Architecture layering violations:", file=sys.stderr)
    for error in errors:
        print(f"  {error}", file=sys.stderr)
    raise SystemExit(1)

print("Repository layering validation passed (existing crates only).")
