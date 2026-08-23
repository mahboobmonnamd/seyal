#!/usr/bin/env python3
from __future__ import annotations

import platform
import subprocess
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "target/benchmarks/harness-smoke.environment.toml"


def git_sha() -> str:
    return subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()


def hardware() -> str:
    if platform.system() == "Darwin":
        try:
            model = subprocess.check_output(["sysctl", "-n", "hw.model"], text=True).strip()
            chip = subprocess.check_output(["sysctl", "-n", "machdep.cpu.brand_string"], text=True).strip()
            return f"{model}; {chip}"
        except (subprocess.CalledProcessError, FileNotFoundError):
            pass
    return f"{platform.machine()}; {platform.processor() or 'unknown-processor'}"


def toml_string(value: str) -> str:
    escaped = value.replace("\\", "\\\\").replace('"', '\\"').replace("\n", " ")
    return f'"{escaped}"'


def main() -> None:
    with (ROOT / "benches/environment-fields.toml").open("rb") as handle:
        schema = tomllib.load(handle)

    record = {
        "record_kind": "harness-smoke",
        "commit_sha": git_sha(),
        "os": platform.platform(),
        "hardware": hardware(),
        "build_mode": "not-a-performance-run",
        "terminal_rows": 0,
        "terminal_cols": 0,
        "font": "not-applicable",
        "display_scale": 0.0,
        "shell": "not-applicable",
        "workload": "harness-smoke",
        "run_count": 1,
        "percentile_method": "not-applicable",
        "performance_claim": False,
    }

    missing = [field for field in schema.get("required", []) if field not in record]
    if missing:
        raise SystemExit(f"benchmark smoke record is missing fields: {', '.join(missing)}")
    if record["performance_claim"]:
        raise SystemExit("benchmark harness smoke must never claim performance")

    OUT.parent.mkdir(parents=True, exist_ok=True)
    lines: list[str] = []
    for key, value in record.items():
        if isinstance(value, bool):
            encoded = "true" if value else "false"
        elif isinstance(value, (int, float)):
            encoded = str(value)
        else:
            encoded = toml_string(value)
        lines.append(f"{key} = {encoded}")
    OUT.write_text("\n".join(lines) + "\n", encoding="utf-8")

    with OUT.open("rb") as handle:
        round_trip = tomllib.load(handle)
    if set(schema.get("required", [])) - set(round_trip):
        raise SystemExit("benchmark environment record failed round-trip validation")

    print(f"[seyal benchmark smoke] recorded non-performance environment metadata: {OUT.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
