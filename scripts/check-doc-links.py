#!/usr/bin/env python3
from __future__ import annotations

import os
import re
import sys
from pathlib import Path
from urllib.parse import unquote

ROOT = Path(os.environ.get("SEYAL_VALIDATION_ROOT", Path(__file__).resolve().parents[1])).resolve()
DOCS_CONTENT_ROOT = ROOT / "site" / "src" / "content" / "docs"
DOCS_PUBLIC_ROOT = ROOT / "site" / "public"
LINK = re.compile(r"\[[^\]]*\]\(([^)]+)\)")
SKIP_PREFIXES = ("http://", "https://", "mailto:", "#")
errors: list[str] = []


def markdown_files() -> list[Path]:
    files: list[Path] = []
    for pattern in ("*.md", "*.mdx"):
        files.extend(ROOT.rglob(pattern))
    return sorted(set(files))


def resolve_target(md: Path, target: str) -> Path:
    if target.startswith("/"):
        # Starlight serves `site/public/*` from the documentation site root.
        # A docs page such as `/images/foo.svg` therefore maps to
        # `site/public/images/foo.svg`, not the repository root.
        try:
            md.relative_to(DOCS_CONTENT_ROOT)
        except ValueError:
            return ROOT / target.lstrip("/")
        return DOCS_PUBLIC_ROOT / target.lstrip("/")
    return md.parent / target


for md in markdown_files():
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
        resolved = resolve_target(md, target)
        if not resolved.exists():
            errors.append(f"{md.relative_to(ROOT)} -> {raw}")

if errors:
    print("Broken local Markdown links:", file=sys.stderr)
    for error in errors:
        print(f"  {error}", file=sys.stderr)
    raise SystemExit(1)

print("Local Markdown/MDX link validation passed.")
