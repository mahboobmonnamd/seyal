#!/usr/bin/env python3
from __future__ import annotations

import subprocess
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def fail(message: str) -> None:
    raise SystemExit(message)


def main() -> None:
    registry_path = ROOT / "fuzz/targets.toml"
    with registry_path.open("rb") as handle:
        registry = tomllib.load(handle)

    if registry.get("version") != 1:
        fail("unsupported fuzz registry version")

    active = 0
    pending = 0
    for target in registry.get("target", []):
        name = target["name"]
        corpus = ROOT / target["corpus"]
        seeds = sorted(path for path in corpus.iterdir() if path.is_file()) if corpus.is_dir() else []
        if not seeds:
            fail(f"fuzz target {name} has no smoke corpus")

        status = target["status"]
        if status == "pending-production-surface":
            pending += 1
            print(f"[seyal fuzz smoke] pending {name}: corpus validated; production adapter not yet allowed")
            continue
        if status != "active":
            fail(f"fuzz target {name} has invalid status: {status}")

        adapter = ROOT / target["adapter"]
        if not adapter.is_file():
            fail(f"active fuzz target {name} is missing adapter: {target['adapter']}")

        for seed in seeds:
            subprocess.run([str(adapter), str(seed)], cwd=ROOT, check=True)
        active += 1
        print(f"[seyal fuzz smoke] active {name}: {len(seeds)} retained seed(s) passed")

    print(f"[seyal fuzz smoke] registry valid: {active} active, {pending} pending production target(s).")


if __name__ == "__main__":
    main()
