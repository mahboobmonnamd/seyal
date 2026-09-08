#!/usr/bin/env python3
"""Regression test for the #819 benchmark output contract."""

from __future__ import annotations

import os
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
VALIDATOR = ROOT / "scripts/check-history-benchmark.py"


def main() -> None:
    env = os.environ.copy()
    env.update(
        {
            "SEYAL_HISTORY_BENCH_LINES": "4",
            "SEYAL_HISTORY_BENCH_EXECUTIONS": "1",
            "SEYAL_HISTORY_BENCH_COLUMNS": "80",
            "SEYAL_HISTORY_BENCH_WORKLOADS": "ascii",
            "SEYAL_HISTORY_BENCH_SAMPLES": "2",
        }
    )
    with tempfile.NamedTemporaryFile(mode="w+", suffix=".log") as log:
        result = subprocess.run(
            [
                "cargo",
                "bench",
                "-p",
                "seyal-terminal",
                "--bench",
                "history_reflow",
                "--locked",
                "--",
                "--quiet",
            ],
            cwd=ROOT,
            env=env,
            stdout=log,
            stderr=subprocess.STDOUT,
            check=False,
            text=True,
        )
        if result.returncode != 0:
            raise SystemExit(f"history benchmark failed with status {result.returncode}")
        log.flush()
        output = Path(log.name).read_text(encoding="utf-8")
        if "append_observations=2 append_samples_per_execution=2" not in output:
            raise SystemExit("append benchmark did not collect two observations for the execution")
        check = subprocess.run(
            [sys.executable, str(VALIDATOR), log.name],
            cwd=ROOT,
            check=False,
            text=True,
        )
        if check.returncode != 0:
            raise SystemExit("history benchmark output contract failed")
    print("[seyal history benchmark contract] integration test passed.")


if __name__ == "__main__":
    main()
