#!/usr/bin/env python3
from __future__ import annotations

import re
import sys
from pathlib import Path

PERCENTILES = ("p50_us", "p95_us", "p99_us", "max_us")


def validate(text: str) -> list[str]:
    errors: list[str] = []
    line = next(
        (
            candidate.strip()
            for candidate in text.splitlines()
            if candidate.strip().startswith("pass7_native_input ")
        ),
        None,
    )
    if line is None:
        return ["missing pass7_native_input evidence line"]

    for token in (
        "boundary=synthetic_NSEvent_to_production_keyDown_return",
        "classification=MEASURED",
        "sample_count=120",
        "appkit_event_boundary=true",
        "production_keyDown_route=true",
        "synthetic_event=true",
        "physical_keyboard=false",
        "performance_claim=false",
    ):
        if token not in line:
            errors.append(f"native input evidence missing {token}")

    for name in PERCENTILES:
        match = re.search(rf"(?:^| ){name}=([0-9]+(?:\.[0-9]+)?)", line)
        if match is None:
            errors.append(f"native input evidence missing numeric {name}")

    return errors


def self_test() -> int:
    good = "pass7_native_input boundary=synthetic_NSEvent_to_production_keyDown_return classification=MEASURED sample_count=120 p50_us=1.0 p95_us=2.0 p99_us=3.0 max_us=4.0 appkit_event_boundary=true production_keyDown_route=true synthetic_event=true physical_keyboard=false performance_claim=false"
    assert not validate(good), validate(good)
    assert validate(good.replace("sample_count=120", "sample_count=12"))
    assert validate(good.replace("synthetic_event=true", "synthetic_event=false"))
    assert validate(good.replace("p99_us=3.0", ""))
    print("[seyal Pass-7 native input benchmark] self-test passed.")
    return 0


def main() -> int:
    if len(sys.argv) == 2 and sys.argv[1] == "--self-test":
        return self_test()
    if len(sys.argv) != 2:
        print(f"usage: {Path(sys.argv[0]).name} [--self-test|<benchmark-log>]", file=sys.stderr)
        return 64
    errors = validate(Path(sys.argv[1]).read_text(encoding="utf-8", errors="replace"))
    if errors:
        print("Pass 7 native input benchmark validation failed:", file=sys.stderr)
        for error in errors:
            print(f"  {error}", file=sys.stderr)
        return 1
    print("[seyal Pass-7 native input benchmark] measured evidence shape passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
