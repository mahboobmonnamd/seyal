#!/usr/bin/env python3
from __future__ import annotations

import os
from pathlib import Path
import tomllib
import argparse
import re

ROOT = Path(os.environ.get("SEYAL_VALIDATION_ROOT", Path(__file__).resolve().parents[1])).resolve()
CONTRACT = ROOT / "docs/evidence/M002-PERFORMANCE-CONTRACT-V1.md"
SCHEMA = ROOT / "docs/evidence/M002-PERFORMANCE-CONTRACT-V1.toml"

REQUIRED = ("Status: proposed contract for Issue #673", "exact production SHA", "baseline SHA", "nearest-rank")
CLASSES = {"CI", "SYNTHETIC", "NATIVE_HEADED", "PHYSICAL_ARM64"}
REQUIRED_GATES = {
    "history_active_reflow_ms", "history_sealed_segment_reflow_ms", "input_visible_proxy",
    "pty_to_terminal_state", "damage_to_client_cache", "high_output_responsiveness",
    "resource_scaling", "startup", "idle_cpu", "renderer_prepare_submission", "teardown_recovery",
}


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
    comparison = schema.get("comparison", {})
    if any(comparison.get(key) is not True for key in ("baseline_required", "exact_head_required", "raw_cohorts_required")):
        raise SystemExit("M002 performance comparison policy is incomplete")
    if not isinstance(comparison.get("noise_policy"), str) or not comparison["noise_policy"].strip():
        raise SystemExit("M002 performance noise policy is missing")
    if not isinstance(comparison.get("regression_rule"), str) or not comparison["regression_rule"].strip():
        raise SystemExit("M002 performance regression rule is missing")
    classes = set(schema.get("evidence_classes", {}))
    if classes != CLASSES:
        raise SystemExit(f"M002 performance schema evidence classes mismatch: {sorted(classes)}")
    for name, evidence_class in schema["evidence_classes"].items():
        if not evidence_class.get("establishes") or not evidence_class.get("cannot_establish"):
            raise SystemExit(f"M002 evidence class {name} has incomplete semantics")
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
    gates = schema.get("gates", {})
    if set(gates) != REQUIRED_GATES:
        raise SystemExit("M002 performance schema gate set is incomplete")
    for name, gate in gates.items():
        if gate.get("evidence_class") not in CLASSES or not gate.get("boundary") or not gate.get("unit"):
            raise SystemExit(f"M002 performance gate {name} is missing boundary, unit, or evidence class")
        if not isinstance(gate.get("relative_regression_percent"), (int, float)) or gate["relative_regression_percent"] < 0:
            raise SystemExit(f"M002 performance gate {name} has invalid relative allowance")
    for name, gate in gates.items():
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
        "contract_schema", "contract_version", "production_sha", "harness_sha", "baseline_sha", "build_mode",
        "os_version", "toolchain", "hardware", "display", "power_thermal_state", "workload_hash", "topology",
        "evidence_class", "gate", "metric", "boundary", "unit", "percentile_method", "sample_count",
        "cohort_count", "environment_status", "platform_limit_reason", "comparator", "p50", "p95", "p99",
        "baseline_p50", "baseline_p95", "baseline_p99", "relative_regression_percent", "raw_log", "raw_cohorts",
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
    gate = schema.get("gates", {}).get(record["gate"])
    if gate is None:
        raise SystemExit("M002 performance result names an unknown gate")
    if gate.get("status", "accepted") != "accepted":
        raise SystemExit("M002 performance result cannot evaluate a proposed gate")
    for field in ("evidence_class", "boundary", "unit"):
        expected = gate.get(field)
        if expected is not None and record[field] != expected:
            raise SystemExit(f"M002 performance result {field} does not match gate contract")
    if record["metric"] != record["gate"]:
        raise SystemExit("M002 performance result metric does not match gate")
    if record["environment_status"] not in schema["result_schema"]["environment_statuses"]:
        raise SystemExit("M002 performance result has invalid environment status")
    if record["comparator"] not in schema["result_schema"]["comparators"]:
        raise SystemExit("M002 performance result has invalid comparator")
    if record["percentile_method"] != schema["percentile_method"]:
        raise SystemExit("M002 performance result percentile method mismatch")
    if record["cohort_count"] != schema["cohorts"] or record["sample_count"] != schema["cohorts"] * schema["samples_per_cohort"]:
        raise SystemExit("M002 performance result does not satisfy the cohort policy")
    for field in ("production_sha", "harness_sha", "baseline_sha", "build_mode", "os_version", "toolchain", "hardware", "display", "power_thermal_state", "workload_hash", "topology", "raw_log", "raw_cohorts"):
        if not isinstance(record[field], str) or not record[field].strip():
            raise SystemExit(f"M002 performance result {field} must be non-empty")
    for field in ("production_sha", "harness_sha", "baseline_sha"):
        if not re.fullmatch(r"[0-9a-fA-F]{40}", record[field]):
            raise SystemExit(f"M002 performance result {field} must be a full commit SHA")
    values = [record[key] for key in ("p50", "p95", "p99")]
    baseline = [record[key] for key in ("baseline_p50", "baseline_p95", "baseline_p99")]
    if any(not isinstance(value, (int, float)) or value < 0 for value in values + baseline):
        raise SystemExit("M002 performance result percentiles and baselines must be non-negative numbers")
    if not values[0] <= values[1] <= values[2]:
        raise SystemExit("M002 performance result percentiles must be ordered p50 <= p95 <= p99")
    if not baseline[0] <= baseline[1] <= baseline[2]:
        raise SystemExit("M002 performance baseline percentiles must be ordered p50 <= p95 <= p99")
    reason = record["platform_limit_reason"]
    if record["environment_status"] == "PLATFORM_LIMITED" and (not isinstance(reason, str) or not reason.strip()):
        raise SystemExit("M002 platform-limited results require a reason")
    if record["environment_status"] == "VALID" and reason:
        raise SystemExit("valid M002 performance results cannot carry a platform-limit reason")
    if record["environment_status"] == "PLATFORM_LIMITED":
        status = "PLATFORM_LIMITED"
    else:
        allowed = gate["relative_regression_percent"]
        if record["relative_regression_percent"] != allowed:
            raise SystemExit("M002 performance result relative allowance does not match gate contract")
        ceilings = [gate.get(key) for key in ("p50", "p95", "p99")]
        absolute_ok = all(limit is not None and value <= limit for value, limit in zip(values, ceilings))
        relative_ok = all(value <= base * (1 + allowed / 100) for value, base in zip(values, baseline))
        status = "PASS" if absolute_ok and relative_ok else "FAIL"
    claimed = record.get("status")
    if claimed is not None and claimed != status:
        raise SystemExit(f"M002 performance result status mismatch: claimed {claimed}, evaluated {status}")
    print(f"M002 performance result: {status} metric={record['metric']} samples={record['sample_count']} cohorts={record['cohort_count']}")
    return status


if __name__ == "__main__":
    main()
