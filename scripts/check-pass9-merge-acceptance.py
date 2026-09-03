#!/usr/bin/env python3
"""Validate Pass 9 merge-acceptance evidence for Issue #735.

This is the lighter merge-critical schema. It does not replace the full
five-cohort `seyal.pass9.production-budget.v1` release-qualification gate.
"""

from __future__ import annotations

import argparse
import copy
import json
import math
import sys
from pathlib import Path
from typing import Any

MODES = ("graceful_detach", "abrupt_socket_loss")
MIN_CYCLES = 100
RETRY_DELAYS_MS = [10, 20, 40, 80, 160, 250]
RUNTIME_RSS_KIB = 1_024
CLIENT_RSS_KIB = 768
RECONNECT_P99_US = 50_000.0  # merge-safety soft gate for full native recovery path

EXACT_RETURN_FIELDS = (
    "attachments",
    "controllers",
    "live_handles",
    "pending_handles",
    "sockets",
    "renderer_surfaces",
    "renderer_gpu_resources",
    "retry_timers",
)
CLIENT_FD_ALLOWANCE = 32


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


def require_exact_return(record: dict[str, Any], field: str, context: str, errors: list[str]) -> None:
    before = integer(record.get(f"{field}_baseline"), f"{context}.{field}_baseline", errors)
    after = integer(record.get(f"{field}_final"), f"{context}.{field}_final", errors)
    if before != after:
        errors.append(f"{context}.{field} did not return exactly: {before} -> {after}")


def validate(document: dict[str, Any], expected_head: str | None = None) -> list[str]:
    errors: list[str] = []
    if document.get("schema") != "seyal.pass9.merge-acceptance.v1":
        errors.append("schema must be seyal.pass9.merge-acceptance.v1")
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
    if not isinstance(cohorts, list) or len(cohorts) != 2:
        errors.append("cohorts must contain exactly two records (graceful + abrupt)")
        return errors

    seen: set[str] = set()
    for index, cohort in enumerate(cohorts):
        context = f"cohorts[{index}]"
        if not isinstance(cohort, dict):
            errors.append(f"{context} must be an object")
            continue
        mode = cohort.get("mode")
        if mode not in MODES:
            errors.append(f"{context}.mode must be one of {MODES}")
        else:
            if mode in seen:
                errors.append(f"duplicate mode {mode}")
            seen.add(mode)
        if integer(cohort.get("cohort"), f"{context}.cohort", errors) != 1:
            errors.append(f"{context}.cohort must be 1 for merge-acceptance")
        if integer(cohort.get("cycles"), f"{context}.cycles", errors) < MIN_CYCLES:
            errors.append(f"{context}.cycles must be at least {MIN_CYCLES}")
        if integer(cohort.get("failures"), f"{context}.failures", errors) != 0:
            errors.append(f"{context}.failures must be 0")
        continuity = cohort.get("continuity", {})
        if not isinstance(continuity, dict):
            errors.append(f"{context}.continuity must be an object")
        else:
            for key in ("runtime_id", "execution_id"):
                value = continuity.get(key)
                if not isinstance(value, str) or not value or value == "none":
                    errors.append(f"{context}.continuity.{key} must be a retained identity")
            if continuity.get("attachment_ids_unique") is not True:
                errors.append(f"{context}.continuity.attachment_ids_unique must be true")
        for field in EXACT_RETURN_FIELDS:
            require_exact_return(cohort, field, context, errors)
        client_before = integer(cohort.get("client_fds_baseline"), f"{context}.client_fds_baseline", errors)
        client_after = integer(cohort.get("client_fds_final"), f"{context}.client_fds_final", errors)
        if client_after - client_before > CLIENT_FD_ALLOWANCE:
            errors.append(
                f"{context}.client_fds grew by {client_after - client_before} "
                f"(allowance {CLIENT_FD_ALLOWANCE})"
            )
        runtime_delta = number(cohort.get("runtime_rss_delta_kib"), f"{context}.runtime_rss_delta_kib", errors)
        client_delta = number(cohort.get("client_rss_delta_kib"), f"{context}.client_rss_delta_kib", errors)
        if runtime_delta > RUNTIME_RSS_KIB:
            errors.append(f"{context}.runtime_rss_delta_kib={runtime_delta:g} exceeds {RUNTIME_RSS_KIB}")
        if client_delta > CLIENT_RSS_KIB:
            errors.append(f"{context}.client_rss_delta_kib={client_delta:g} exceeds {CLIENT_RSS_KIB}")
        reconnect = number(cohort.get("reconnect_p99_us"), f"{context}.reconnect_p99_us", errors)
        if reconnect > RECONNECT_P99_US:
            errors.append(f"{context}.reconnect_p99_us={reconnect:g} exceeds merge soft gate {RECONNECT_P99_US:g}")

    if seen != set(MODES):
        errors.append(f"cohorts must cover both modes; observed={sorted(seen)}")
    return errors


def fixture() -> dict[str, Any]:
    cohorts = []
    for mode in MODES:
        cohorts.append(
            {
                "mode": mode,
                "geometry": "120x40",
                "cohort": 1,
                "cycles": 100,
                "warmup_cycles": 5,
                "continuity": {
                    "runtime_id": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    "execution_id": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                    "attachment_ids_unique": True,
                },
                "attachments_baseline": 0,
                "attachments_final": 0,
                "controllers_baseline": 0,
                "controllers_final": 0,
                "live_handles_baseline": 0,
                "live_handles_final": 0,
                "pending_handles_baseline": 0,
                "pending_handles_final": 0,
                "client_fds_baseline": 12,
                "client_fds_final": 12,
                "sockets_baseline": 0,
                "sockets_final": 0,
                "renderer_surfaces_baseline": 0,
                "renderer_surfaces_final": 0,
                "renderer_gpu_resources_baseline": 0,
                "renderer_gpu_resources_final": 0,
                "retry_timers_baseline": 0,
                "retry_timers_final": 0,
                "runtime_rss_kib_baseline_median": 20_000,
                "runtime_rss_kib_final_median": 20_010,
                "client_rss_kib_baseline_median": 40_000,
                "client_rss_kib_final_median": 40_020,
                "runtime_rss_delta_kib": 10,
                "client_rss_delta_kib": 20,
                "reconnect_p99_us": 900.0,
                "failures": 0,
            }
        )
    return {
        "schema": "seyal.pass9.merge-acceptance.v1",
        "measurement_source": "supplied_exact_head_evidence",
        "commit": "a" * 40,
        "recovery": {
            "attempts": 7,
            "retry_delays_ms": RETRY_DELAYS_MS,
            "deadline_ms": 1_000,
            "launches_per_episode_max": 1,
        },
        "cohorts": cohorts,
    }


def self_test() -> None:
    ok = validate(fixture(), expected_head="a" * 40)
    if ok:
        raise SystemExit(f"expected fixture to pass; errors={ok}")
    broken = copy.deepcopy(fixture())
    broken["cohorts"][0]["attachments_final"] = 1
    errors = validate(broken)
    if not any("attachments did not return exactly" in error for error in errors):
        raise SystemExit(f"expected resource-return failure; errors={errors}")
    print("[seyal Pass 9 merge-acceptance] validator self-test passed.")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("artifact", nargs="?", type=Path)
    parser.add_argument("--expected-head")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return
    if args.artifact is None:
        parser.error("artifact path is required unless --self-test is set")
    document = json.loads(args.artifact.read_text())
    errors = validate(document, expected_head=args.expected_head)
    if errors:
        for error in errors:
            print(f"[seyal Pass 9 merge-acceptance] ERROR: {error}", file=sys.stderr)
        raise SystemExit(1)
    print(
        f"[seyal Pass 9 merge-acceptance] OK commit={document.get('commit')} "
        f"cohorts={len(document.get('cohorts', []))}"
    )


if __name__ == "__main__":
    main()
