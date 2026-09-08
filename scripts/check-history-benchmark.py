#!/usr/bin/env python3
"""Validate the machine-readable shape of the #819 history benchmark."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

WORKLOADS = {"ascii", "styled", "cjk", "emoji-combining"}
FULL_LINES = {10_000, 100_000, 1_000_000}
FULL_EXECUTIONS = {1, 10, 50, 100}
FULL_COLUMNS = {40, 48, 64, 80, 96, 132, 160}


def parse_cases(text: str) -> list[dict[str, str]]:
    cases: list[dict[str, str]] = []
    for line in text.splitlines():
        if not line.startswith("[seyal history benchmark] case "):
            continue
        fields = dict(
            field.split("=", 1)
            for field in line.removeprefix("[seyal history benchmark] case ").split()
            if "=" in field
        )
        cases.append(fields)
    return cases


def validate(text: str, require_full_matrix: bool = False) -> list[str]:
    errors: list[str] = []
    cases = parse_cases(text)
    if not cases:
        return ["missing machine-readable history benchmark cases"]

    required = {
        "workload",
        "lines",
        "executions",
        "columns",
        "commit",
        "resident_history_bytes",
        "derived_cache_bytes",
        "rss_before_kib",
        "rss_after_kib",
        "rss_delta_kib",
        "rss_available",
        "append_p50_ns",
        "append_p95_ns",
        "append_p99_ns",
        "reflow_p50_ns",
        "reflow_p95_ns",
        "reflow_p99_ns",
        "search_p50_ns",
        "search_p95_ns",
        "search_p99_ns",
        "anchor_p50_ns",
        "anchor_p95_ns",
        "anchor_p99_ns",
        "resolved_anchors",
        "allocation_calls",
        "allocated_bytes",
        "deallocated_bytes",
        "allocation_status",
        "samples",
        "percentile_method",
        "performance_claim",
        "evidence_scope",
    }
    for index, case in enumerate(cases, start=1):
        missing = sorted(required - case.keys())
        if missing:
            errors.append(f"case {index} missing fields: {', '.join(missing)}")
            continue
        if case["workload"] not in WORKLOADS:
            errors.append(f"case {index} has unknown workload {case['workload']!r}")
        for key in ("lines", "executions", "columns", "samples"):
            try:
                if int(case[key]) <= 0:
                    errors.append(f"case {index} has non-positive {key}")
            except ValueError:
                errors.append(f"case {index} has non-integer {key}")
        for key in (
            "resident_history_bytes",
            "derived_cache_bytes",
            "rss_before_kib",
            "rss_after_kib",
            "rss_delta_kib",
            "resolved_anchors",
            "append_p50_ns",
            "append_p95_ns",
            "append_p99_ns",
            "reflow_p50_ns",
            "reflow_p95_ns",
            "reflow_p99_ns",
            "search_p50_ns",
            "search_p95_ns",
            "search_p99_ns",
            "anchor_p50_ns",
            "anchor_p95_ns",
            "anchor_p99_ns",
        ):
            try:
                if int(case[key]) < 0:
                    errors.append(f"case {index} has negative {key}")
            except ValueError:
                errors.append(f"case {index} has non-integer {key}")
        if case["rss_available"] not in {"true", "false"}:
            errors.append(f"case {index} has invalid rss_available")
        if case["allocation_status"] not in {"not-instrumented", "measured"}:
            errors.append(f"case {index} has invalid allocation_status")
        if case["percentile_method"] != "nearest-rank":
            errors.append(f"case {index} must use nearest-rank percentiles")
        if case["performance_claim"] != "false":
            errors.append(f"case {index} must keep performance_claim=false")
        if case["evidence_scope"] != "TerminalState-comparative":
            errors.append(f"case {index} must identify TerminalState-comparative scope")

    if require_full_matrix and not errors:
        observed = {
            (
                case["workload"],
                int(case["lines"]),
                int(case["executions"]),
                int(case["columns"]),
            )
            for case in cases
        }
        expected = {
            (workload, lines, executions, columns)
            for workload in WORKLOADS
            for lines in FULL_LINES
            for executions in FULL_EXECUTIONS
            for columns in FULL_COLUMNS
        }
        missing = sorted(expected - observed)
        if missing:
            errors.append(f"full matrix missing {len(missing)} cases")
        if len(cases) != len(expected):
            errors.append(
                f"full matrix expected {len(expected)} unique cases, observed {len(cases)} lines"
            )
        if any(int(case["resolved_anchors"]) == 0 for case in cases):
            errors.append("full matrix contains a case without a resolved source anchor")

    return errors


def self_test() -> None:
    valid = (
        "[seyal history benchmark] case workload=ascii lines=4 executions=1 "
        "columns=80 commit=fixture resident_history_bytes=1 rss_before_kib=2 rss_after_kib=3 "
        "derived_cache_bytes=1 rss_delta_kib=1 rss_available=true "
        "append_p50_ns=1 append_p95_ns=1 append_p99_ns=1 "
        "reflow_p50_ns=1 reflow_p95_ns=1 reflow_p99_ns=1 search_p50_ns=1 "
        "search_p95_ns=1 search_p99_ns=1 anchor_p50_ns=1 anchor_p95_ns=1 "
        "anchor_p99_ns=1 resolved_anchors=1 allocation_calls=1 allocated_bytes=1 "
        "deallocated_bytes=1 "
        "allocation_status=measured "
        "samples=1 percentile_method=nearest-rank performance_claim=false "
        "evidence_scope=TerminalState-comparative\n"
    )
    if validate(valid):
        raise SystemExit("valid benchmark fixture was rejected")
    if not validate(valid.replace("performance_claim=false", "performance_claim=true")):
        raise SystemExit("invalid performance claim was accepted")
    if not validate(valid, require_full_matrix=True):
        raise SystemExit("incomplete matrix was accepted")
    print("[seyal history benchmark contract] self-test passed.")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("log", nargs="?", type=Path)
    parser.add_argument("--require-full-matrix", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return
    if args.log is None:
        parser.error("a benchmark log is required unless --self-test is used")
    errors = validate(args.log.read_text(encoding="utf-8"), args.require_full_matrix)
    if errors:
        print("History benchmark contract violations:", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        raise SystemExit(1)
    print(f"[seyal history benchmark contract] {len(parse_cases(args.log.read_text(encoding='utf-8')))} case(s) passed.")


if __name__ == "__main__":
    main()
