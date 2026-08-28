#!/usr/bin/env python3
from __future__ import annotations

import re
import sys
from pathlib import Path

REQUIRED_CASES = {
    "commit_1b",
    "commit_16kib",
    "commit_64kib",
    "reject_65537b",
    "key_repeat_arrow_up",
    "input_under_sustained_output",
    "alternate_screen_input_resize",
}
REQUIRED_PERCENTILES = ("p50_us", "p95_us", "p99_us", "max_us")


def validate(text: str) -> list[str]:
    errors: list[str] = []
    lines = [line.strip() for line in text.splitlines() if line.strip()]
    host = next((line for line in lines if line.startswith("pass7_matrix_host ")), None)
    if host is None:
        errors.append("missing pass7_matrix_host")
    else:
        for token in (
            "classification=MEASURED",
            "repetitions=120",
            "architecture=production_client_UDS_Runtime_PTY",
            "performance_claim=false",
        ):
            if token not in host:
                errors.append(f"pass7_matrix_host missing {token}")

    observed: set[str] = set()
    for line in lines:
        if not line.startswith("pass7_matrix case="):
            continue
        match = re.search(r"(?:^| )case=([^ ]+)", line)
        if match is None:
            errors.append("matrix case line missing case name")
            continue
        case = match.group(1)
        observed.add(case)
        if "classification=MEASURED" not in line:
            errors.append(f"{case} is not classified MEASURED")
        if "sample_count=120" not in line:
            errors.append(f"{case} does not contain 120 samples")
        if "performance_claim=false" not in line:
            errors.append(f"{case} missing performance_claim=false")
        for percentile in REQUIRED_PERCENTILES:
            if re.search(rf"(?:^| ){percentile}=([0-9]+(?:\.[0-9]+)?)", line) is None:
                errors.append(f"{case} missing numeric {percentile}")

    missing = sorted(REQUIRED_CASES - observed)
    if missing:
        errors.append(f"missing Pass 7 matrix cases: {', '.join(missing)}")

    commit_expectations = {
        "commit_1b": "committed_bytes=1",
        "commit_16kib": "committed_bytes=16384",
        "commit_64kib": "committed_bytes=65536",
    }
    for case, token in commit_expectations.items():
        line = next((line for line in lines if f"case={case} " in line), None)
        if line is not None:
            for required in (token, "final_pty_completion=true"):
                if required not in line:
                    errors.append(f"{case} missing {required}")

    rejected = next((line for line in lines if "case=reject_65537b " in line), None)
    if rejected is not None:
        for token in ("atomic_rejection=true", "pty_write_bytes=0", "client_queue_bytes=0"):
            if token not in rejected:
                errors.append(f"reject_65537b missing {token}")

    repeat = next((line for line in lines if "case=key_repeat_arrow_up " in line), None)
    if repeat is not None:
        for token in ("keys_per_burst=64", "encoded_bytes_per_burst=192"):
            if token not in repeat:
                errors.append(f"key_repeat_arrow_up missing {token}")

    output = next(
        (line for line in lines if "case=input_under_sustained_output " in line), None
    )
    if output is not None and "output_progress_observed=true" not in output:
        errors.append("input_under_sustained_output missing output_progress_observed=true")

    alternate = next(
        (line for line in lines if "case=alternate_screen_input_resize " in line), None
    )
    if alternate is not None:
        for token in ("alternate_screen=true", "final_geometry=100x30"):
            if token not in alternate:
                errors.append(f"alternate_screen_input_resize missing {token}")

    remaining = next(
        (line for line in lines if line.startswith("pass7_matrix_remaining ")), None
    )
    if remaining is None:
        errors.append("missing explicit remaining-gap classification")
    else:
        for token in (
            "persistent_runtime_resize_failure_and_true_AppKit_event_boundary",
            "classification=NOT_CLAIMED",
            "performance_claim=false",
        ):
            if token not in remaining:
                errors.append(f"remaining-gap line missing {token}")

    return errors


def self_test() -> int:
    good = """
pass7_matrix_host classification=MEASURED repetitions=120 key_repeat_burst=64 architecture=production_client_UDS_Runtime_PTY performance_claim=false
pass7_matrix case=commit_1b classification=MEASURED sample_count=120 committed_bytes=1 p50_us=1.0 p95_us=2.0 p99_us=3.0 max_us=4.0 client_queue_high_water_bytes=1 runtime_queue_high_water_bytes=1 final_pty_completion=true performance_claim=false
pass7_matrix case=commit_16kib classification=MEASURED sample_count=120 committed_bytes=16384 p50_us=1.0 p95_us=2.0 p99_us=3.0 max_us=4.0 client_queue_high_water_bytes=1 runtime_queue_high_water_bytes=1 final_pty_completion=true performance_claim=false
pass7_matrix case=commit_64kib classification=MEASURED sample_count=120 committed_bytes=65536 p50_us=1.0 p95_us=2.0 p99_us=3.0 max_us=4.0 client_queue_high_water_bytes=1 runtime_queue_high_water_bytes=1 final_pty_completion=true performance_claim=false
pass7_matrix case=reject_65537b classification=MEASURED sample_count=120 p50_us=1.0 p95_us=2.0 p99_us=3.0 max_us=4.0 atomic_rejection=true pty_write_bytes=0 client_queue_bytes=0 performance_claim=false
pass7_matrix case=key_repeat_arrow_up classification=MEASURED sample_count=120 keys_per_burst=64 encoded_bytes_per_burst=192 p50_us=1.0 p95_us=2.0 p99_us=3.0 max_us=4.0 runtime_queue_high_water_bytes=1 performance_claim=false
pass7_matrix case=input_under_sustained_output classification=MEASURED sample_count=120 p50_us=1.0 p95_us=2.0 p99_us=3.0 max_us=4.0 output_progress_observed=true performance_claim=false
pass7_matrix case=alternate_screen_input_resize classification=MEASURED sample_count=120 p50_us=1.0 p95_us=2.0 p99_us=3.0 max_us=4.0 alternate_screen=true final_geometry=100x30 performance_claim=false
pass7_matrix_remaining validation=persistent_runtime_resize_failure_and_true_AppKit_event_boundary classification=NOT_CLAIMED performance_claim=false
"""
    assert not validate(good), validate(good)
    assert validate(good.replace("case=commit_64kib", "case=missing", 1))
    assert validate(good.replace("final_pty_completion=true", "", 1))
    assert validate(good.replace("classification=NOT_CLAIMED", "classification=MEASURED"))
    print("[seyal Pass-7 validation matrix] self-test passed.")
    return 0


def main() -> int:
    if len(sys.argv) == 2 and sys.argv[1] == "--self-test":
        return self_test()
    if len(sys.argv) != 2:
        print(f"usage: {Path(sys.argv[0]).name} [--self-test|<benchmark-log>]", file=sys.stderr)
        return 64
    errors = validate(Path(sys.argv[1]).read_text(encoding="utf-8", errors="replace"))
    if errors:
        print("Pass 7 validation matrix failed:", file=sys.stderr)
        for error in errors:
            print(f"  {error}", file=sys.stderr)
        return 1
    print("[seyal Pass-7 validation matrix] measured evidence shape passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
