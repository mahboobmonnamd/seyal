#!/usr/bin/env python3
from __future__ import annotations

import os
import tempfile
import tomllib
from pathlib import Path

ROOT = Path(os.environ.get("SEYAL_VALIDATION_ROOT", Path(__file__).resolve().parents[1])).resolve()


def read_toml(path: Path) -> dict:
    with path.open("rb") as handle:
        return tomllib.load(handle)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(message)


def sorted_fixture_ids(manifest: dict) -> list[str]:
    fixtures = manifest.get("fixtures", [])
    require(isinstance(fixtures, list), "VT fixture manifest 'fixtures' must be a list")
    ids = [entry["id"] for entry in fixtures]
    require(len(ids) == len(set(ids)), "VT fixture ids must be unique")
    return sorted(ids)


def validate_fixture_harness() -> None:
    manifest_path = ROOT / "tests/fixtures/vt/manifest.toml"
    schema_path = ROOT / "tests/fixtures/vt/provenance.schema.toml"
    require(manifest_path.is_file(), "missing VT fixture manifest")
    require(schema_path.is_file(), "missing VT provenance schema")

    manifest = read_toml(manifest_path)
    schema = read_toml(schema_path)
    require(manifest.get("version") == 1, "unsupported VT fixture manifest version")
    require(schema.get("version") == 1, "unsupported VT provenance schema version")
    required_provenance = set(schema.get("required", []))
    require(len(required_provenance) >= 8, "VT provenance schema is incomplete")
    sorted_fixture_ids(manifest)

    allowed_classifications = {"supported", "tested-deferred", "unsupported-deferred"}
    for fixture in manifest.get("fixtures", []):
        fixture_id = fixture.get("id", "<missing-id>")
        require(
            not (required_provenance - set(fixture)),
            f"VT fixture {fixture_id} is missing provenance fields: "
            f"{sorted(required_provenance - set(fixture))}",
        )
        require(
            fixture.get("classification") in allowed_classifications,
            f"VT fixture {fixture_id} has invalid classification",
        )
        for field in ("input", "expected"):
            relative = fixture.get(field)
            require(isinstance(relative, str) and relative, f"VT fixture {fixture_id} has no {field} path")
            path = (ROOT / relative).resolve()
            require(path.is_relative_to(ROOT), f"VT fixture {fixture_id} {field} escapes repository root")
            require(path.is_file(), f"VT fixture {fixture_id} is missing {field} file: {relative}")
        require(
            isinstance(fixture.get("cols"), int)
            and fixture["cols"] > 0
            and isinstance(fixture.get("rows"), int)
            and fixture["rows"] > 0,
            f"VT fixture {fixture_id} must declare positive cols/rows",
        )

    # Exercise deterministic loading independently of the retained manifest.
    with tempfile.TemporaryDirectory() as tmp:
        synthetic = Path(tmp) / "manifest.toml"
        synthetic.write_text(
            'version = 1\n[[fixtures]]\nid = "z"\n[[fixtures]]\nid = "a"\n',
            encoding="utf-8",
        )
        require(
            sorted_fixture_ids(read_toml(synthetic)) == ["a", "z"],
            "fixture ordering is not deterministic",
        )


def validate_fuzz_registry() -> None:
    registry_path = ROOT / "fuzz/targets.toml"
    require(registry_path.is_file(), "missing fuzz target registry")
    registry = read_toml(registry_path)
    require(registry.get("version") == 1, "unsupported fuzz registry version")

    targets = registry.get("target", [])
    expected = {
        "vt-byte-parser",
        "parser-state-mutation",
        "local-binary-protocol-decode",
        "shared-projection-validation",
        "reconnect-resync-state-machine",
    }
    names = {target.get("name") for target in targets}
    require(names == expected, f"fuzz registry mismatch: expected {sorted(expected)}, got {sorted(names)}")

    for target in targets:
        require(
            target.get("status") in {"pending-production-surface", "active"},
            f"invalid fuzz status for {target.get('name')}",
        )
        corpus = ROOT / target["corpus"]
        require(corpus.is_dir(), f"missing fuzz corpus directory: {target['corpus']}")
        seeds = sorted(path for path in corpus.iterdir() if path.is_file())
        require(seeds, f"fuzz corpus has no retained seed: {target['name']}")
        if target["status"] == "active":
            require((ROOT / target["adapter"]).is_file(), f"active fuzz target has no adapter: {target['name']}")


def validate_benchmark_contract() -> None:
    schema_path = ROOT / "benches/environment-fields.toml"
    require(schema_path.is_file(), "missing benchmark environment field contract")
    schema = read_toml(schema_path)
    require(schema.get("version") == 1, "unsupported benchmark environment schema version")
    required = set(schema.get("required", []))
    for key in {
        "commit_sha",
        "os",
        "hardware",
        "build_mode",
        "workload",
        "run_count",
        "percentile_method",
        "performance_claim",
    }:
        require(key in required, f"benchmark metadata is missing required field: {key}")


def main() -> None:
    require((ROOT / "tests/integration/README.md").is_file(), "missing integration-test harness location")
    validate_fixture_harness()
    validate_fuzz_registry()
    validate_benchmark_contract()
    print("[seyal harness test] fixture/fuzz/benchmark harness contracts passed.")


if __name__ == "__main__":
    main()
