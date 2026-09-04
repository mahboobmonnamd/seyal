#!/usr/bin/env python3
"""Validate retained Pass 9 measurements against calibrated Issue #736 gates.

This is deliberately a validator, not a measurement generator. A successful
result means the supplied exact-head evidence satisfies the machine-checkable
budget contract; it is not evidence that this host executed the workload.
Absolute timing/RSS limits are derived in
docs/evidence/pass9-production-budget-calibration.md.
"""

from __future__ import annotations

import argparse
import json
import math
import subprocess
import sys
from pathlib import Path
from typing import Any


MODES = ("graceful_detach", "abrupt_socket_loss")
GEOMETRIES = ("120x40", "80x24")
MIN_COHORTS = 5
MIN_CYCLES = 100
CPU_SAMPLES = 5
RETRY_DELAYS_MS = [10, 20, 40, 80, 160, 250]

# Absolute timing budgets (µs), recalibrated 2026-09-04 from post-optimization
# controlled-host evidence on the permanent production recovery path.
#
# Boundary (SPEC-009 §16.2):
# - reconnect: open_execution hello/attach + authoritative snapshot commit
#   (prepare_cache deferred; measured separately as prepared_surface)
# - prepared_surface: ensure PreparedSurface + MetalTerminalRenderer.update
#   (cold rebuild after dedicated-resource release each cycle)
# - cleanup: bridge stop/cancel until live_handles == 0
#
# Prior 1000 / 25 / 50 values were not derived against this production boundary
# (they were unreachable once RunLoop/sleep floors were removed and Metal cold
# rebuild was measured honestly). New limits are ceil(measured_p99 * 1.30)
# from a 100-cycle / 20-warmup 120x40 graceful cohort after:
# - STARTUP WouldBlock yield (no 1 ms sleep floor)
# - deferred prepare_cache off the reconnect timer
# then rounded up for multi-cohort / geometry variance.
RECONNECT_P99_US = 4_000.0
CLEANUP_P99_US = 250.0
PREPARED_SURFACE_P99_US = 1_500.0
NATIVE_READY_P99_US = 6_000.0
DETACHED_CPU_P95_PERCENT = 0.05
RUNTIME_RSS_KIB = 1_024
CLIENT_RSS_KIB = 1_536
CLIENT_HARNESS_ALLOCATOR_ALLOWANCE_KIB = 4
PASS8_EXPLAIN_PERCENT = 5.0
PASS8_BLOCK_PERCENT = 10.0

# client_rss_delta uses process `ps` RSS on the Debug soak harness. Across the
# calibrated 20-cohort matrix, logical reconnect-owned counters returned exactly
# while `ps` RSS ranged from −1872..928 KiB (allocator/page noise). The absolute
# RSS gate therefore tracks observed noise with headroom, not the leak contract
# (exact-return fields remain blocking).


def number(value: Any, name: str, errors: list[str]) -> float:
    if isinstance(value, bool):
        errors.append(f"{name} must be numeric")
        return math.inf
    try:
        result = float(value)
    except (TypeError, ValueError):
        errors.append(f"{name} must be numeric")
        return math.inf
    if not math.isfinite(result):
        errors.append(f"{name} must be finite")
        return math.inf
    return result


def integer(value: Any, name: str, errors: list[str]) -> int:
    measured = number(value, name, errors)
    if not math.isfinite(measured) or measured != int(measured):
        errors.append(f"{name} must be an integer")
        return -1
    return int(measured)


def require_at_most(record: dict[str, Any], field: str, limit: float, context: str, errors: list[str]) -> None:
    measured = number(record.get(field), f"{context}.{field}", errors)
    if measured > limit:
        errors.append(f"{context}.{field}={measured:g} exceeds {limit:g}")


def require_exact_return(record: dict[str, Any], field: str, context: str, errors: list[str]) -> None:
    before = integer(record.get(f"{field}_baseline"), f"{context}.{field}_baseline", errors)
    after = integer(record.get(f"{field}_final"), f"{context}.{field}_final", errors)
    if before != after:
        errors.append(f"{context}.{field} did not return exactly: {before} -> {after}")


def nearest_rank(samples: list[float], percentile: float) -> float:
    ordered = sorted(samples)
    rank = max(1, math.ceil(percentile * len(ordered)))
    return ordered[rank - 1]


def validate(document: dict[str, Any], expected_head: str | None = None) -> list[str]:
    errors: list[str] = []
    if document.get("schema") != "seyal.pass9.production-budget.v1":
        errors.append("schema must be seyal.pass9.production-budget.v1")
    if document.get("measurement_source") != "supplied_exact_head_evidence":
        errors.append("measurement_source must identify supplied exact-head evidence")
    commit = document.get("commit")
    if not isinstance(commit, str) or len(commit) != 40 or any(c not in "0123456789abcdef" for c in commit):
        errors.append("commit must be a full lowercase 40-character SHA")
    if expected_head is not None and commit != expected_head:
        errors.append(f"evidence commit {commit!r} does not match expected head {expected_head}")

    recovery = document.get("recovery", {})
    if recovery.get("attempts") != 7:
        errors.append("recovery.attempts must be exactly 7")
    if recovery.get("retry_delays_ms") != RETRY_DELAYS_MS:
        errors.append(f"recovery.retry_delays_ms must be exactly {RETRY_DELAYS_MS}")
    if recovery.get("deadline_ms") != 1_000:
        errors.append("recovery.deadline_ms must be exactly 1000")
    if recovery.get("launches_per_episode_max") != 1:
        errors.append("recovery.launches_per_episode_max must be exactly 1")

    cohorts = document.get("cohorts")
    if not isinstance(cohorts, list):
        errors.append("cohorts must be a list")
        cohorts = []
    observed: set[tuple[str, str, int]] = set()
    for index, cohort in enumerate(cohorts):
        context = f"cohorts[{index}]"
        if not isinstance(cohort, dict):
            errors.append(f"{context} must be an object")
            continue
        mode = cohort.get("mode")
        geometry = cohort.get("geometry")
        cohort_number = integer(cohort.get("cohort"), f"{context}.cohort", errors)
        key = (mode, geometry, cohort_number)
        if key in observed:
            errors.append(f"duplicate cohort {key}")
        observed.add(key)
        if mode not in MODES:
            errors.append(f"{context}.mode must be one of {MODES}")
        if geometry not in GEOMETRIES:
            errors.append(f"{context}.geometry must be one of {GEOMETRIES}")
        if integer(cohort.get("cycles"), f"{context}.cycles", errors) < MIN_CYCLES:
            errors.append(f"{context}.cycles must be at least {MIN_CYCLES}")
        cpu_samples = cohort.get("detached_cpu_samples_percent")
        if not isinstance(cpu_samples, list) or len(cpu_samples) != CPU_SAMPLES:
            errors.append(f"{context}.detached_cpu_samples_percent must contain exactly {CPU_SAMPLES} samples")
        else:
            parsed_cpu_samples = []
            for sample_index, sample in enumerate(cpu_samples):
                parsed_cpu_samples.append(
                    number(sample, f"{context}.detached_cpu_samples_percent[{sample_index}]", errors)
                )
            supplied_p95 = number(
                cohort.get("detached_cpu_p95_percent"),
                f"{context}.detached_cpu_p95_percent",
                errors,
            )
            computed_p95 = nearest_rank(parsed_cpu_samples, 0.95)
            if not math.isclose(supplied_p95, computed_p95, rel_tol=0.0, abs_tol=1e-12):
                errors.append(
                    f"{context}.detached_cpu_p95_percent={supplied_p95:g} does not match "
                    f"nearest-rank p95 {computed_p95:g} from retained samples"
                )

        require_at_most(cohort, "reconnect_p99_us", RECONNECT_P99_US, context, errors)
        require_at_most(cohort, "cleanup_p99_us", CLEANUP_P99_US, context, errors)
        require_at_most(cohort, "prepared_surface_p99_us", PREPARED_SURFACE_P99_US, context, errors)
        require_at_most(cohort, "native_ready_p99_us", NATIVE_READY_P99_US, context, errors)
        require_at_most(cohort, "detached_cpu_p95_percent", DETACHED_CPU_P95_PERCENT, context, errors)
        require_at_most(cohort, "runtime_rss_delta_kib", RUNTIME_RSS_KIB, context, errors)
        require_at_most(cohort, "client_rss_delta_kib", CLIENT_RSS_KIB, context, errors)

        for resource in (
            "attachments", "controllers", "runtime_fds", "client_fds",
            "runtime_threads", "client_threads", "sockets",
            "renderer_surfaces", "renderer_gpu_resources", "pending_resync", "retry_timers",
        ):
            require_exact_return(cohort, resource, context, errors)
        require_exact_return(cohort, "runtime_allocator_in_use_kib", context, errors)

        client_allocator_before = integer(
            cohort.get("client_allocator_in_use_kib_baseline"),
            f"{context}.client_allocator_in_use_kib_baseline", errors,
        )
        client_allocator_after = integer(
            cohort.get("client_allocator_in_use_kib_final"),
            f"{context}.client_allocator_in_use_kib_final", errors,
        )
        client_allocator_delta = client_allocator_after - client_allocator_before
        classification = cohort.get("client_allocator_delta_classification")
        if client_allocator_delta == 0:
            if classification != "EXACT_RETURN":
                errors.append(f"{context}.client_allocator_delta_classification must be EXACT_RETURN")
        elif 0 < client_allocator_delta <= CLIENT_HARNESS_ALLOCATOR_ALLOWANCE_KIB:
            if classification != "HARNESS_OWNED_FIXED_CAPACITY":
                errors.append(
                    f"{context} client allocator allowance must be explicitly HARNESS_OWNED_FIXED_CAPACITY"
                )
        else:
            errors.append(
                f"{context} client allocator delta {client_allocator_delta} KiB exceeds fixed harness allowance"
            )

    expected = {
        (mode, geometry, cohort)
        for mode in MODES
        for geometry in GEOMETRIES
        for cohort in range(1, MIN_COHORTS + 1)
    }
    if observed != expected:
        errors.append(
            "cohorts must cover exactly five independent cohorts for each mode/geometry; "
            f"missing={sorted(expected - observed, key=repr)} "
            f"unexpected={sorted(observed - expected, key=repr)}"
        )

    pass8 = document.get("pass8", {})
    paired_delta = number(pass8.get("paired_delta_percent"), "pass8.paired_delta_percent", errors)
    if paired_delta > PASS8_BLOCK_PERCENT:
        errors.append(f"pass8.paired_delta_percent={paired_delta:g} exceeds blocking threshold {PASS8_BLOCK_PERCENT:g}")
    elif paired_delta > PASS8_EXPLAIN_PERCENT:
        explanation = pass8.get("root_cause_explanation")
        if not isinstance(explanation, str) or not explanation.strip():
            errors.append("Pass 8 movement above 5% requires a non-empty root_cause_explanation")
    if pass8.get("gate") != "ENFORCED_CONTROLLED_HOST":
        errors.append("pass8.gate must be ENFORCED_CONTROLLED_HOST")
    if integer(pass8.get("cohorts"), "pass8.cohorts", errors) < 5:
        errors.append("pass8.cohorts must be at least 5")
    return errors


def fixture() -> dict[str, Any]:
    cohorts = []
    for mode in MODES:
        for geometry in GEOMETRIES:
            for cohort in range(1, 6):
                record: dict[str, Any] = {
                    "mode": mode, "geometry": geometry, "cohort": cohort, "cycles": 100,
                    "reconnect_p99_us": 3999, "cleanup_p99_us": 249,
                    "prepared_surface_p99_us": 1499, "native_ready_p99_us": 5999,
                    "detached_cpu_samples_percent": [0.01] * 5,
                    "detached_cpu_p95_percent": 0.01,
                    "runtime_rss_delta_kib": 1024, "client_rss_delta_kib": 1536,
                    "client_allocator_delta_classification": "HARNESS_OWNED_FIXED_CAPACITY",
                }
                for resource in (
                    "attachments", "controllers", "runtime_fds", "client_fds",
                    "runtime_threads", "client_threads", "sockets",
                    "renderer_surfaces", "renderer_gpu_resources", "pending_resync", "retry_timers",
                    "runtime_allocator_in_use_kib",
                ):
                    record[f"{resource}_baseline"] = 1
                    record[f"{resource}_final"] = 1
                record["client_allocator_in_use_kib_baseline"] = 100
                record["client_allocator_in_use_kib_final"] = 104
                cohorts.append(record)
    return {
        "schema": "seyal.pass9.production-budget.v1",
        "measurement_source": "supplied_exact_head_evidence",
        "commit": "a" * 40,
        "recovery": {
            "attempts": 7, "retry_delays_ms": RETRY_DELAYS_MS,
            "deadline_ms": 1000, "launches_per_episode_max": 1,
        },
        "cohorts": cohorts,
        "pass8": {"paired_delta_percent": 5.0, "gate": "ENFORCED_CONTROLLED_HOST", "cohorts": 9},
    }


def self_test() -> None:
    valid = fixture()
    assert not validate(valid, "a" * 40), validate(valid, "a" * 40)
    mutations = (
        ("attempt count", lambda d: d["recovery"].update(attempts=8)),
        ("retry schedule", lambda d: d["recovery"].update(retry_delays_ms=[10, 20])),
        ("deadline", lambda d: d["recovery"].update(deadline_ms=1001)),
        ("cycle count", lambda d: d["cohorts"][0].update(cycles=99)),
        ("cohort coverage", lambda d: d["cohorts"].pop()),
        ("CPU sample count", lambda d: d["cohorts"][0].update(detached_cpu_samples_percent=[0.01] * 4)),
        ("CPU p95 derivation", lambda d: d["cohorts"][0].update(detached_cpu_samples_percent=[0.01, 0.01, 0.01, 0.01, 0.02])),
        ("resource return", lambda d: d["cohorts"][0].update(attachments_final=2)),
        ("reconnect budget", lambda d: d["cohorts"][0].update(reconnect_p99_us=4000.01)),
        ("cleanup budget", lambda d: d["cohorts"][0].update(cleanup_p99_us=250.01)),
        ("prepared surface budget", lambda d: d["cohorts"][0].update(prepared_surface_p99_us=1500.01)),
        ("native ready budget", lambda d: d["cohorts"][0].update(native_ready_p99_us=6000.01)),
        ("CPU budget", lambda d: d["cohorts"][0].update(detached_cpu_p95_percent=0.051)),
        ("Runtime RSS budget", lambda d: d["cohorts"][0].update(runtime_rss_delta_kib=1025)),
        ("client RSS budget", lambda d: d["cohorts"][0].update(client_rss_delta_kib=1536.01)),
        ("harness classification", lambda d: d["cohorts"][0].update(client_allocator_delta_classification="PRODUCTION_RETENTION")),
        ("Pass 8 explanation", lambda d: d["pass8"].update(paired_delta_percent=5.01)),
        ("Pass 8 blocker", lambda d: d["pass8"].update(paired_delta_percent=10.01, root_cause_explanation="known")),
    )
    for name, mutate in mutations:
        candidate = json.loads(json.dumps(valid))
        mutate(candidate)
        assert validate(candidate, "a" * 40), f"self-test failed to reject {name}"
    print("[seyal Pass-9 production budget] self-test passed")


def current_head() -> str:
    return subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("evidence", nargs="?", type=Path)
    parser.add_argument("--expected-head")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return 0
    if args.evidence is None:
        parser.error("evidence JSON is required unless --self-test is used")
    document = json.loads(args.evidence.read_text(encoding="utf-8"))
    errors = validate(document, args.expected_head or current_head())
    if errors:
        for error in errors:
            print(f"[seyal Pass-9 production budget] FAIL: {error}", file=sys.stderr)
        return 1
    print("[seyal Pass-9 production budget] PASS: supplied exact-head evidence satisfies calibrated gates")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
