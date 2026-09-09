#!/usr/bin/env python3
from __future__ import annotations

import os
from pathlib import Path
import tomllib
import argparse

ROOT = Path(os.environ.get("SEYAL_VALIDATION_ROOT", Path(__file__).resolve().parents[1])).resolve()
CONTRACT = ROOT / "docs/evidence/M002-PERFORMANCE-CONTRACT-V1.md"
SCHEMA = ROOT / "docs/evidence/M002-PERFORMANCE-CONTRACT-V1.toml"

REQUIRED = ("Status: proposed contract for Issue #673", "exact production SHA", "baseline SHA", "nearest-rank")
CLASSES = {"CI", "SYNTHETIC", "NATIVE_HEADED", "PHYSICAL_ARM64"}


def main() -> None:
    if not CONTRACT.is_file():
        raise SystemExit(f"missing M002 performance contract: {CONTRACT.relative_to(ROOT)}")
    if not SCHEMA.is_file():
        raise SystemExit(f"missing M002 performance schema: {SCHEMA.relative_to(ROOT)}")
    text = CONTRACT.read_text(encoding="utf-8")
    missing = [token for token in REQUIRED if token not in text]
    if missing:
        raise SystemExit("M002 performance contract missing: " + ", ".join(repr(token) for token in missing))
    try:
        schema = tomllib.loads(SCHEMA.read_text(encoding="utf-8"))
    except tomllib.TOMLDecodeError as error:
        raise SystemExit(f"invalid M002 performance schema: {error}") from error
    if schema.get("schema") != "seyal.m002.performance-contract" or schema.get("version") != 1:
        raise SystemExit("M002 performance schema has unsupported identity")
    if schema.get("status") != "proposed":
        raise SystemExit("M002 performance schema must remain proposed until accepted")
    if schema.get("percentile_method") != "nearest-rank":
        raise SystemExit("M002 performance schema must use nearest-rank percentiles")
    if schema.get("cohorts") != 5 or schema.get("warmups_per_cohort") != 20 or schema.get("samples_per_cohort") != 100:
        raise SystemExit("M002 performance schema has invalid cohort policy")
    if set(schema.get("missing_metric_values", [])) != {"unknown", "not-instrumented"}:
        raise SystemExit("M002 performance schema must preserve unknown and not-instrumented metrics")
    classes = set(schema.get("evidence_classes", {}))
    if classes != CLASSES:
        raise SystemExit(f"M002 performance schema evidence classes mismatch: {sorted(classes)}")
    caps = schema.get("resource_caps", {})
    expected_caps = {
        "sealed_payload_bytes": 16384,
        "mutable_tail_bytes": 32768,
        "history_per_execution_bytes": 33554432,
        "history_runtime_aggregate_bytes": 268435456,
        "cache_per_execution_bytes": 4194304,
        "cache_runtime_aggregate_bytes": 33554432,
    }
    if caps != expected_caps:
        raise SystemExit("M002 performance schema resource caps do not match #818/SPEC-010")
    for name, gate in schema.get("gates", {}).items():
        if gate.get("evidence_class") not in CLASSES:
            raise SystemExit(f"M002 performance gate {name} has an invalid evidence class")
        if gate.get("status", "accepted") == "accepted" and "source" not in gate:
            raise SystemExit(f"accepted M002 performance gate {name} is missing authority source")
    matrix = schema.get("matrix", {})
    if matrix.get("retained_content") != [10000, 100000, 1000000] or matrix.get("execution_populations") != [1, 10, 50, 100]:
        raise SystemExit("M002 performance matrix is incomplete")
    if "performance_claim=true" in text or "performance_claim=true" in SCHEMA.read_text(encoding="utf-8"):
        raise SystemExit("M002 performance contract must not claim a gate passed")
    result_schema = schema.get("result_schema", {})
    required_result_fields = set(result_schema.get("required", []))
    expected_result_fields = {
        "contract_schema", "contract_version", "production_sha", "harness_sha", "baseline_sha",
        "evidence_class", "metric", "boundary", "unit", "percentile_method", "sample_count",
        "cohort_count", "environment_status", "comparator", "p50", "p95", "p99", "ceiling_p50",
        "ceiling_p95", "ceiling_p99",
    }
    if required_result_fields != expected_result_fields:
        raise SystemExit("M002 performance result schema is incomplete")
    args = argparse.ArgumentParser(add_help=False)
    args.add_argument("--record")
    record_args, _ = args.parse_known_args()
    if record_args.record:
        evaluate_record(Path(record_args.record), schema)
    print("M002 performance contract shape passed.")


def evaluate_record(path: Path, schema: dict) -> str:
    try:
        record = tomllib.loads(path.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise SystemExit(f"invalid M002 performance result record: {error}") from error
    required = set(schema["result_schema"]["required"])
    missing = sorted(required - record.keys())
    if missing:
        raise SystemExit("M002 performance result missing: " + ", ".join(missing))
    if record["contract_schema"] != schema["schema"] or record["contract_version"] != schema["version"]:
        raise SystemExit("M002 performance result contract identity mismatch")
    if record["evidence_class"] not in CLASSES:
        raise SystemExit("M002 performance result has invalid evidence class")
    if record["environment_status"] not in schema["result_schema"]["environment_statuses"]:
        raise SystemExit("M002 performance result has invalid environment status")
    if record["comparator"] not in schema["result_schema"]["comparators"]:
        raise SystemExit("M002 performance result has invalid comparator")
    if record["percentile_method"] != schema["percentile_method"]:
        raise SystemExit("M002 performance result percentile method mismatch")
    if not isinstance(record["sample_count"], int) or record["sample_count"] <= 0:
        raise SystemExit("M002 performance result sample_count must be a positive integer")
    if not isinstance(record["cohort_count"], int) or record["cohort_count"] <= 0:
        raise SystemExit("M002 performance result cohort_count must be a positive integer")
    values = [record[key] for key in ("p50", "p95", "p99")]
    ceilings = [record[key] for key in ("ceiling_p50", "ceiling_p95", "ceiling_p99")]
    if any(not isinstance(value, (int, float)) or value < 0 for value in values + ceilings):
        raise SystemExit("M002 performance result percentiles and ceilings must be non-negative numbers")
    if not values[0] <= values[1] <= values[2]:
        raise SystemExit("M002 performance result percentiles must be ordered p50 <= p95 <= p99")
    if record["environment_status"] == "PLATFORM_LIMITED":
        status = "PLATFORM_LIMITED"
    else:
        status = "PASS" if all(value <= ceiling for value, ceiling in zip(values, ceilings)) else "FAIL"
    claimed = record.get("status")
    if claimed is not None and claimed != status:
        raise SystemExit(f"M002 performance result status mismatch: claimed {claimed}, evaluated {status}")
    print(f"M002 performance result: {status} metric={record['metric']} samples={record['sample_count']} cohorts={record['cohort_count']}")
    return status


if __name__ == "__main__":
    main()
