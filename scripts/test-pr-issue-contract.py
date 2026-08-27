#!/usr/bin/env python3
from __future__ import annotations

import os
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
VALIDATOR = ROOT / "scripts/check-pr-issue-contract.py"
ENV_BODY = "SEYAL_PR_BODY"


def run(body: str) -> subprocess.CompletedProcess[str]:
    env = os.environ.copy()
    env[ENV_BODY] = body
    return subprocess.run(
        ["python3", str(VALIDATOR)],
        cwd=ROOT,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
    )


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(f"[seyal PR issue contract self-test] ERROR: {message}")


def expect_ok(body: str) -> None:
    result = run(body)
    require(result.returncode == 0, f"valid fixture failed:\n{result.stdout}")


def expect_fail(body: str, expected: str) -> None:
    result = run(body)
    require(result.returncode != 0, "invalid fixture unexpectedly passed")
    require(expected in result.stdout, f"expected {expected!r}; output was:\n{result.stdout}")


def main() -> None:
    expect_ok(
        """## Issue

Owning Issue: #704

Closes #704

## Goal
Complete the owning Issue.
"""
    )
    expect_ok(
        """## Issue

Owning Issue: #704

Part of #704

## Goal
Partial work; documentation may mention `Closes #999` as an example without activating it.
"""
    )

    expect_fail("## Goal\nNo issue section.\n", "missing required '## Issue' section")
    expect_fail(
        """## Issue
Owning Issue: #704
Closes #705
## Goal
Mismatch.
""",
        "Issue relationship targets #705, but owning Issue is #704",
    )
    expect_fail(
        """## Issue
Owning Issue: #704
Closes #704
Refs #704
## Goal
Ambiguous relationship.
""",
        "expected exactly one closing/non-closing Issue relationship",
    )
    expect_fail(
        """## Issue
Owning Issue: #704
Refs #704
## Goal
This also closes #704 accidentally.
""",
        "cannot coexist with active GitHub closing keyword",
    )
    expect_fail(
        """## Issue
Owning Issue: #704
Closes #704
## Goal
This also fixes #700 accidentally.
""",
        "a closing PR must contain exactly one active GitHub closing keyword",
    )

    print("[seyal PR issue contract self-test] valid and adversarial fixtures passed.")


if __name__ == "__main__":
    main()
