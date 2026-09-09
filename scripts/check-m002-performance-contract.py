#!/usr/bin/env python3
from __future__ import annotations

import os
from pathlib import Path

ROOT = Path(os.environ.get("SEYAL_VALIDATION_ROOT", Path(__file__).resolve().parents[1])).resolve()
CONTRACT = ROOT / "docs/evidence/M002-PERFORMANCE-CONTRACT-V1.md"

REQUIRED = (
    "Status: proposed contract for Issue #673",
    "exact production SHA",
    "baseline SHA",
    "nearest-rank",
    "`CI`",
    "`SYNTHETIC`",
    "`NATIVE_HEADED`",
    "`PHYSICAL_ARM64`",
    "unknown",
    "not-instrumented",
    "1/10/50/100",
    "10k/100k/1M",
)


def main() -> None:
    if not CONTRACT.is_file():
        raise SystemExit(f"missing M002 performance contract: {CONTRACT.relative_to(ROOT)}")
    text = CONTRACT.read_text(encoding="utf-8")
    missing = [token for token in REQUIRED if token not in text]
    if missing:
        raise SystemExit("M002 performance contract missing: " + ", ".join(repr(token) for token in missing))
    if "performance gate passed" in text.lower() or "performance_claim=true" in text:
        raise SystemExit("M002 performance contract must not claim a gate passed")
    print("M002 performance contract shape passed.")


if __name__ == "__main__":
    main()
