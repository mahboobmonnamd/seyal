#!/usr/bin/env python3
from __future__ import annotations

import re
import sys
from pathlib import Path

REQUIRED_LATENCY_BOUNDARIES = {
    "controlled_native_callback_to_client_admission",
    "client_admission_to_socket_complete",
    "runtime_frame_admission_to_pty_write",
    "controlled_native_callback_to_pty_write",
    "resize_120x40",
    "resize_512x256",
}
REQUIRED_PERCENTILES = ("p50_us", "p95_us", "p99_us", "max_us")


def validate(text: str) -> list[str]:
    errors: list[str] = []
    lines = [line.strip() for line in text.splitlines() if line.strip()]

    host = next((line for line in lines if line.startswith("pass7_host ")), None)
    if host is None:
        errors.append("missing pass7_host metadata")
    else:
        for token in (
            "macos_version=",
            "macos_build=",
            "hardware=",
            "arch=",
            "rust=",
            "build_mode=release",
            "commit=",
            "repetitions=120",
            "percentile_method=nearest_rank",
            "performance_claim=false",
        ):
            if token not in host:
                errors.append(f"pass7_host missing {token}")

    observed: set[str] = set()
    for line in lines:
        if not line.startswith("pass7_latency "):
            continue
        match = re.search(r"(?:^| )boundary=([^ ]+)", line)
        if match is None:
            errors.append("pass7_latency line missing boundary")
            continue
        boundary = match.group(1)
        observed.add(boundary)
        if "classification=MEASURED" not in line:
            errors.append(f"{boundary} is not classified MEASURED")
        if "sample_count=120" not in line:
            errors.append(f"{boundary} does not contain 120 measured samples")
        if "performance_claim=false" not in line:
            errors.append(f"{boundary} missing performance_claim=false")
        for percentile in REQUIRED_PERCENTILES:
            match_value = re.search(rf"(?:^| ){percentile}=([0-9]+(?:\.[0-9]+)?)", line)
            if match_value is None:
                errors.append(f"{boundary} missing numeric {percentile}")

    missing = sorted(REQUIRED_LATENCY_BOUNDARIES - observed)
    if missing:
        errors.append(f"missing Pass 7 latency boundaries: {', '.join(missing)}")

    input_resources = next(
        (line for line in lines if line.startswith("pass7_input_resources ")), None
    )
    if input_resources is None:
        errors.append("missing pass7_input_resources")
    else:
        for token in (
            "client_queue_high_water_bytes=",
            "runtime_queue_high_water_bytes=",
            "rss_baseline_kib=",
            "rss_populated_kib=",
            "rss_measured_kib=",
            "measurement_phase=post_input_workload",
            "incremental_post_workload_rss_kib=",
            "cpu_percent_sample=",
            "native_boundary_classification=CONTROLLED_FFI_EQUIVALENT_APPKIT_EVENT_NOT_CLAIMED",
            "performance_claim=false",
        ):
            if token not in input_resources:
                errors.append(f"pass7_input_resources missing {token}")

    idle = next((line for line in lines if line.startswith("pass7_idle_resource ")), None)
    if idle is None:
        errors.append("missing pass7_idle_resource")
    else:
        for token in (
            "idle_window_ms=500",
            "rss_idle_kib=",
            "incremental_idle_rss_kib=",
            "cpu_percent_sample=",
            "client_wants_write=false",
            "performance_claim=false",
        ):
            if token not in idle:
                errors.append(f"pass7_idle_resource missing {token}")

    for geometry in ("120x40", "512x256"):
        if not any(
            line.startswith("pass7_resize_resources ") and f"geometry={geometry}" in line
            for line in lines
        ):
            errors.append(f"missing resize resource evidence for {geometry}")

    # The Rust harness is intentionally not allowed to masquerade as AppKit
    # event evidence. A separate native measurement must replace/supplement this
    # classification before Pass 7 can claim the native-event target.
    if any(
        "native_boundary_classification=APPKIT_EVENT" in line
        for line in lines
        if line.startswith("pass7_input_resources ")
    ):
        errors.append("Rust harness must not self-classify as AppKit event evidence")

    return errors


def self_test() -> int:
    good = """
pass7_host macos_version=26.5.2 macos_build=25F84 hardware=\"Apple M5 Pro\" arch=aarch64 rust=\"rustc 1.98.0\" build_mode=release commit=abc repetitions=120 percentile_method=nearest_rank performance_claim=false
pass7_latency boundary=controlled_native_callback_to_client_admission classification=MEASURED sample_count=120 p50_us=1.0 p95_us=2.0 p99_us=3.0 max_us=4.0 performance_claim=false
pass7_latency boundary=client_admission_to_socket_complete classification=MEASURED sample_count=120 p50_us=1.0 p95_us=2.0 p99_us=3.0 max_us=4.0 performance_claim=false
pass7_latency boundary=runtime_frame_admission_to_pty_write classification=MEASURED sample_count=120 p50_us=1.0 p95_us=2.0 p99_us=3.0 max_us=4.0 performance_claim=false
pass7_latency boundary=controlled_native_callback_to_pty_write classification=MEASURED sample_count=120 p50_us=1.0 p95_us=2.0 p99_us=3.0 max_us=4.0 performance_claim=false
pass7_latency boundary=resize_120x40 classification=MEASURED sample_count=120 p50_us=1.0 p95_us=2.0 p99_us=3.0 max_us=4.0 performance_claim=false
pass7_latency boundary=resize_512x256 classification=MEASURED sample_count=120 p50_us=1.0 p95_us=2.0 p99_us=3.0 max_us=4.0 performance_claim=false
pass7_input_resources classification=MEASURED measurement_phase=post_input_workload client_queue_high_water_bytes=41 runtime_queue_high_water_bytes=1 rss_baseline_kib=100 rss_populated_kib=200 rss_measured_kib=210 incremental_post_workload_rss_kib=110 cpu_percent_sample=0.1 native_boundary_classification=CONTROLLED_FFI_EQUIVALENT_APPKIT_EVENT_NOT_CLAIMED performance_claim=false
pass7_resize_resources case=resize_120x40 geometry=120x40 classification=MEASURED measurement_phase=post_resize_workload client_queue_high_water_bytes=56 runtime_queue_high_water_bytes=0 rss_baseline_kib=100 rss_populated_kib=200 rss_measured_kib=210 incremental_post_resize_rss_kib=110 cpu_percent_sample=0.1 performance_claim=false
pass7_resize_resources case=resize_512x256 geometry=512x256 classification=MEASURED measurement_phase=post_resize_workload client_queue_high_water_bytes=56 runtime_queue_high_water_bytes=0 rss_baseline_kib=100 rss_populated_kib=200 rss_measured_kib=210 incremental_post_resize_rss_kib=110 cpu_percent_sample=0.1 performance_claim=false
pass7_idle_resource classification=MEASURED idle_window_ms=500 rss_baseline_kib=100 rss_populated_kib=200 rss_idle_kib=205 incremental_idle_rss_kib=105 cpu_percent_sample=0.0 threads_baseline=1 threads_idle=2 fds_baseline=4 fds_idle=8 client_wants_write=false performance_claim=false
"""
    assert not validate(good), validate(good)
    assert validate(good.replace("p99_us=3.0", "", 1))
    assert validate(good.replace("repetitions=120", "repetitions=12"))
    assert validate(good.replace("pass7_latency boundary=resize_512x256", "pass7_latency boundary=missing_max"))
    assert validate(good.replace("CONTROLLED_FFI_EQUIVALENT_APPKIT_EVENT_NOT_CLAIMED", "APPKIT_EVENT"))
    print("[seyal Pass-7 benchmark coverage] self-test passed.")
    return 0


def main() -> int:
    if len(sys.argv) == 2 and sys.argv[1] == "--self-test":
        return self_test()
    if len(sys.argv) != 2:
        print(f"usage: {Path(sys.argv[0]).name} [--self-test|<benchmark-log>]", file=sys.stderr)
        return 64
    text = Path(sys.argv[1]).read_text(encoding="utf-8", errors="replace")
    errors = validate(text)
    if errors:
        print("Pass 7 benchmark coverage validation failed:", file=sys.stderr)
        for error in errors:
            print(f"  {error}", file=sys.stderr)
        return 1
    print("[seyal Pass-7 benchmark coverage] measured evidence shape passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
