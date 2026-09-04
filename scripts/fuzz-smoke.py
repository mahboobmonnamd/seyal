#!/usr/bin/env python3
from __future__ import annotations

import re
import subprocess
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def fail(message: str) -> None:
    raise SystemExit(message)


def cargo_fuzz_bins(cargo_toml: Path) -> set[str]:
    text = cargo_toml.read_text(encoding="utf-8")
    bins: set[str] = set()
    in_bin = False
    for line in text.splitlines():
        stripped = line.strip()
        if stripped == "[[bin]]":
            in_bin = True
            continue
        if stripped.startswith("["):
            in_bin = False
            continue
        if in_bin:
            match = re.match(r'name\s*=\s*"([^"]+)"\s*$', stripped)
            if match:
                bins.add(match.group(1))
    return bins


def main() -> None:
    registry_path = ROOT / "fuzz/targets.toml"
    with registry_path.open("rb") as handle:
        registry = tomllib.load(handle)

    if registry.get("version") != 1:
        fail("unsupported fuzz registry version")

    fuzz_cargo = ROOT / "fuzz/Cargo.toml"
    if not fuzz_cargo.is_file():
        fail("missing fuzz/Cargo.toml")
    known_bins = cargo_fuzz_bins(fuzz_cargo)

    active = 0
    pending = 0
    comparator = 0
    required_bins: set[str] = set()

    for target in registry.get("target", []):
        name = target["name"]
        corpus = ROOT / target["corpus"]
        seeds = sorted(path for path in corpus.iterdir() if path.is_file()) if corpus.is_dir() else []
        if not seeds:
            fail(f"fuzz target {name} has no smoke corpus")

        status = target["status"]
        if status == "pending-production-surface":
            pending += 1
            print(
                f"[seyal fuzz smoke] pending {name}: corpus validated; production adapter not yet allowed"
            )
            continue
        if status == "non-production-comparator":
            adapter = ROOT / target["adapter"]
            if not adapter.is_file():
                fail(
                    f"non-production comparator {name} is missing adapter: {target['adapter']}"
                )
            if "libfuzzer" in target:
                fail(
                    f"non-production comparator {name} must not claim a production libfuzzer campaign"
                )
            comparator += 1
            print(
                f"[seyal fuzz smoke] comparator {name}: corpus present; not production §6.9 coverage"
            )
            continue
        if status != "active":
            fail(f"fuzz target {name} has invalid status: {status}")

        adapter = ROOT / target["adapter"]
        if not adapter.is_file():
            fail(f"active fuzz target {name} is missing adapter: {target['adapter']}")

        libfuzzer = target.get("libfuzzer")
        if not libfuzzer:
            fail(f"active fuzz target {name} is missing required libfuzzer binary mapping")
        if libfuzzer not in known_bins:
            fail(
                f"active fuzz target {name} maps to unknown libfuzzer bin {libfuzzer!r}; "
                f"known={sorted(known_bins)}"
            )
        required_bins.add(libfuzzer)

        campaign_corpus = target.get("campaign_corpus", target["corpus"])
        campaign_dir = ROOT / campaign_corpus
        campaign_seeds = (
            sorted(path for path in campaign_dir.iterdir() if path.is_file())
            if campaign_dir.is_dir()
            else []
        )
        if not campaign_seeds:
            fail(f"active fuzz target {name} has empty campaign corpus: {campaign_corpus}")

        # All registry adapters are checked-in POSIX shell entry points. Invoke
        # them through bash so a Contents-API-created fuzz adapter cannot become
        # a false CI failure merely because its executable mode was not
        # preserved by the remote write path.
        for seed in seeds:
            subprocess.run(["bash", str(adapter), str(seed)], cwd=ROOT, check=True)
        active += 1
        print(
            f"[seyal fuzz smoke] active {name}: {len(seeds)} retained seed(s) passed; "
            f"libfuzzer={libfuzzer}"
        )

    for decision in registry.get("surface_decision", []):
        owner_pass = decision["owner_pass"]
        verdict = decision["decision"]
        proof = ROOT / decision["proof"]
        if verdict not in {"N/A", "covered"}:
            fail(f"surface_decision owner_pass={owner_pass} has invalid decision: {verdict}")
        if not proof.is_file():
            fail(
                f"surface_decision owner_pass={owner_pass} proof missing: {decision['proof']}"
            )
        print(
            f"[seyal fuzz smoke] surface_decision pass {owner_pass}: {verdict} "
            f"(proof={decision['proof']})"
        )

    orphan_bins = sorted(known_bins - required_bins)
    if orphan_bins:
        fail(
            "libFuzzer binaries exist without an active registry row: "
            + ", ".join(orphan_bins)
        )

    print(
        f"[seyal fuzz smoke] registry valid: {active} active, {pending} pending, "
        f"{comparator} non-production comparator(s); campaign parity ok."
    )


if __name__ == "__main__":
    main()
