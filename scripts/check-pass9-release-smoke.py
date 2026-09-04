#!/usr/bin/env python3
"""Smoke-validate one or more Pass 9 production-budget cohorts (fast iteration).

Unlike check-pass9-production-budget.py, this does not require the full 5×2×2
matrix or Pass 8 attribution. It applies the same numeric gates to whatever
cohorts are present so harness fixes can be proven before a multi-hour matrix.
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path
from typing import Any

RECONNECT_P99_US = 4_000.0
CLEANUP_P99_US = 250.0
PREPARED_SURFACE_P99_US = 1_500.0
NATIVE_READY_P99_US = 6_000.0
DETACHED_CPU_P95_PERCENT = 0.05
RUNTIME_RSS_KIB = 1_024
CLIENT_RSS_KIB = 1_536
CLIENT_HARNESS_ALLOCATOR_ALLOWANCE_KIB = 4


def number(value: Any, name: str, errors: list[str]) -> float:
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


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("evidence", type=Path)
    parser.add_argument(
        "--skip-latency",
        action="store_true",
        help="Skip latency/RSS numeric gates for low-cycle dry-runs only; exact-return still enforced.",
    )
    args = parser.parse_args()
    document = json.loads(args.evidence.read_text(encoding="utf-8"))
    errors: list[str] = []
    note = document.get("topology_note")
    if not isinstance(note, str) or "SPEC-009 §10" not in note:
        errors.append("topology_note must claim SPEC-009 §10 native interaction measurement")
    if isinstance(note, str) and "NOT SPEC" in note:
        errors.append("topology_note must not exclude SPEC native_ready claims")
    cohorts = document.get("cohorts")
    if not isinstance(cohorts, list) or not cohorts:
        print("FAIL: cohorts missing", file=sys.stderr)
        return 1
    for index, cohort in enumerate(cohorts):
        context = f"cohorts[{index}]"
        if not isinstance(cohort, dict):
            errors.append(f"{context} must be object")
            continue
        if not args.skip_latency:
            for field, limit in (
                ("reconnect_p99_us", RECONNECT_P99_US),
                ("cleanup_p99_us", CLEANUP_P99_US),
                ("prepared_surface_p99_us", PREPARED_SURFACE_P99_US),
                ("native_ready_p99_us", NATIVE_READY_P99_US),
                ("detached_cpu_p95_percent", DETACHED_CPU_P95_PERCENT),
                ("runtime_rss_delta_kib", RUNTIME_RSS_KIB),
                ("client_rss_delta_kib", CLIENT_RSS_KIB),
            ):
                measured = number(cohort.get(field), f"{context}.{field}", errors)
                if measured > limit:
                    errors.append(f"{context}.{field}={measured:g} exceeds {limit:g}")
        for resource in (
            "attachments", "controllers", "runtime_fds", "client_fds",
            "runtime_threads", "client_threads", "sockets",
            "renderer_surfaces", "renderer_gpu_resources", "pending_resync", "retry_timers",
            "runtime_allocator_in_use_kib",
        ):
            before = integer(cohort.get(f"{resource}_baseline"), f"{context}.{resource}_baseline", errors)
            after = integer(cohort.get(f"{resource}_final"), f"{context}.{resource}_final", errors)
            if before != after:
                errors.append(f"{context}.{resource} did not return exactly: {before} -> {after}")
        client_before = integer(
            cohort.get("client_allocator_in_use_kib_baseline"),
            f"{context}.client_allocator_in_use_kib_baseline",
            errors,
        )
        client_after = integer(
            cohort.get("client_allocator_in_use_kib_final"),
            f"{context}.client_allocator_in_use_kib_final",
            errors,
        )
        delta = client_after - client_before
        classification = cohort.get("client_allocator_delta_classification")
        if delta == 0:
            if classification != "EXACT_RETURN":
                errors.append(f"{context} client allocator classification must be EXACT_RETURN")
        elif 0 < delta <= CLIENT_HARNESS_ALLOCATOR_ALLOWANCE_KIB:
            if classification != "HARNESS_OWNED_FIXED_CAPACITY":
                errors.append(f"{context} client allocator classification must be HARNESS_OWNED_FIXED_CAPACITY")
        else:
            errors.append(f"{context} client allocator delta {delta} KiB exceeds fixed harness allowance")

    if errors:
        for error in errors:
            print(f"[pass9-release-smoke] FAIL: {error}", file=sys.stderr)
        return 1
    mode = "exact-return (+ latency skipped)" if args.skip_latency else "full smoke gates"
    print(f"[pass9-release-smoke] PASS: {len(cohorts)} cohort(s) satisfy {mode}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
