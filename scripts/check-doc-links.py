#!/usr/bin/env python3
from __future__ import annotations

import re
import sys
from pathlib import Path
from urllib.parse import unquote

ROOT = Path(__file__).resolve().parents[1]
LINK = re.compile(r"\[[^\]]*\]\(([^)]+)\)")
SKIP_PREFIXES = ("http://", "https://", "mailto:", "#")
errors: list[str] = []

for md in ROOT.rglob("*.md"):
    if ".git" in md.parts:
        continue
    text = md.read_text(encoding="utf-8")
    for raw in LINK.findall(text):
        target = raw.strip().split(maxsplit=1)[0].strip("<>")
        if not target or target.startswith(SKIP_PREFIXES):
            continue
        target = unquote(target.split("#", 1)[0])
        if not target:
            continue
        resolved = (ROOT / target.lstrip("/")) if target.startswith("/") else (md.parent / target)
        if not resolved.exists():
            errors.append(f"{md.relative_to(ROOT)} -> {raw}")

if errors:
    print("Broken local Markdown links:", file=sys.stderr)
    for error in errors:
        print(f"  {error}", file=sys.stderr)
    raise SystemExit(1)

print("Local Markdown link validation passed.")
