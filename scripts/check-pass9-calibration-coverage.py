#!/usr/bin/env python3
from __future__ import annotations

import math
import re
import sys
from pathlib import Path

MODES = ("graceful_detach", "abrupt_socket_loss")
GEOMETRIES = ("120x40", "80x24")
COHORTS = range(1, 6)
PERCENTILES = ("p50_us", "p95_us", "p99_us", "max_us")


def field(line: str, name: str) -> str | None:
    match = re.search(rf"(?:^| ){re.escape(name)}=([^ ]+)", line)
    return match.group(1) if match else None


def require_number(errors: list[str], line: str, name: str, context: str) -> None:
    value = field(line, name)
    if value is None:
        errors.append(f"{context} missing {name}")
        return
    try:
        if not math.isfinite(float(value)):
            raise ValueError
    except ValueError:
        errors.append(f"{context} has non-finite {name}={value}")


def expected_keys() -> set[tuple[str, str, int]]:
    return {(mode, geometry, cohort) for mode in MODES for geometry in GEOMETRIES for cohort in COHORTS}


def line_key(line: str, context: str, errors: list[str]) -> tuple[str, str, int] | None:
    mode, geometry, cohort = field(line, "mode"), field(line, "geometry"), field(line, "cohort")
    if mode is None or geometry is None or cohort is None:
        errors.append(f"{context} missing mode, geometry, or cohort")
        return None
    try:
        return (mode, geometry, int(cohort))
    except ValueError:
        errors.append(f"{context} has invalid cohort={cohort}")
        return None


def validate_lifecycle(text: str) -> list[str]:
    errors: list[str] = []
    lines = [line.strip() for line in text.splitlines() if line.strip()]
    if any(" SKIPPED " in line or "PLATFORM_LIMITED" in line for line in lines):
        errors.append("controlled lifecycle log contains skipped or platform-limited output")

    hosts = [line for line in lines if line.startswith("pass9_calibration_host ")]
    if len(hosts) != 1:
        errors.append(f"expected exactly one pass9_calibration_host, found {len(hosts)}")
    elif any(token not in hosts[0] for token in (
        "macos_version=", "macos_build=", "model=", "hardware=", "arch=", "rust=",
        "build_mode=release", "commit=", "master_baseline=", "pass8_baseline=", "performance_claim=false",
    )):
        errors.append("pass9_calibration_host is missing required controlled-host metadata")

    starts = [line for line in lines if line.startswith("pass9_preimplementation_calibration architecture=")]
    if len(starts) != 1:
        errors.append(f"expected exactly one calibration start record, found {len(starts)}")
    elif any(token not in starts[0] for token in (
        "warmup_cycles=20", "measured_cycles=100", "cohorts=5", "geometries=120x40,80x24",
        "percentile_method=nearest_rank", "rss_samples=5", "performance_claim=false",
    )):
        errors.append("calibration start record is incomplete")

    cohort_lines = [line for line in lines if line.startswith("pass9_calibration_cohort ")]
    observed: set[tuple[str, str, int]] = set()
    for line in cohort_lines:
        key = line_key(line, "cohort record", errors)
        if key is None:
            continue
        if key in observed:
            errors.append(f"duplicate cohort record {key}")
        observed.add(key)
        for token in (
            "sample_count=100", "attachment_controller_fd_thread_return_each_cycle=true",
            "client_socket_closed_each_cycle=true", "performance_claim=false",
        ):
            if token not in line:
                errors.append(f"cohort {key} missing {token}")
        for name in ("reconnect_p50_us", "reconnect_p95_us", "reconnect_p99_us", "reconnect_max_us",
                     "renderer_p50_us", "renderer_p95_us", "renderer_p99_us", "renderer_max_us",
                     "cleanup_p50_us", "cleanup_p95_us", "cleanup_p99_us", "cleanup_max_us",
                     "idle_runtime_cpu_percent"):
            require_number(errors, line, name, f"cohort {key}")
        for resource in ("runtime_fds", "runtime_threads", "client_fds", "client_threads"):
            before, after = field(line, f"{resource}_baseline"), field(line, f"{resource}_final")
            if before is None or after is None:
                errors.append(f"cohort {key} missing {resource} baseline/final")
            elif before != after:
                errors.append(f"cohort {key} leaked {resource}: {before} -> {after}")

    missing = expected_keys() - observed
    unexpected = observed - expected_keys()
    if missing:
        errors.append(f"missing lifecycle cohorts: {sorted(missing)}")
    if unexpected:
        errors.append(f"unexpected lifecycle cohorts: {sorted(unexpected)}")

    rss_lines = [line for line in lines if line.startswith("pass9_calibration_rss_samples ")]
    rss_observed: set[tuple[str, str, int]] = set()
    for line in rss_lines:
        key = line_key(line, "RSS sample record", errors)
        if key is None:
            continue
        if key in rss_observed:
            errors.append(f"duplicate RSS sample record {key}")
        rss_observed.add(key)
        if "sample_count=100" not in line or "performance_claim=false" not in line:
            errors.append(f"RSS sample record {key} has incomplete metadata")
        for name in ("runtime_rss_kib", "client_rss_kib"):
            samples = field(line, name)
            if samples is None:
                errors.append(f"RSS sample record {key} missing {name}")
                continue
            values = samples.split(",")
            if len(values) != 100 or any(not value.isdigit() for value in values):
                errors.append(f"RSS sample record {key} must retain 100 integer {name} samples")
        for name in ("runtime_slope_kib_per_cycle", "client_slope_kib_per_cycle"):
            require_number(errors, line, name, f"RSS sample record {key}")
    if rss_observed != expected_keys():
        errors.append("RSS sample records do not cover exactly every lifecycle cohort")

    summaries = [line for line in lines if line.startswith("pass9_calibration_summary ")]
    summary_keys: set[tuple[str, str]] = set()
    for line in summaries:
        mode, geometry = field(line, "mode"), field(line, "geometry")
        if mode is None or geometry is None:
            errors.append("summary missing mode or geometry")
            continue
        key = (mode, geometry)
        if key in summary_keys:
            errors.append(f"duplicate summary {key}")
        summary_keys.add(key)
        for token in ("cohorts=5", "cycles_per_cohort=100", "performance_claim=false"):
            if token not in line:
                errors.append(f"summary {key} missing {token}")
        for name in ("median_cohort_p99_us", "median_renderer_cohort_p99_us", "median_cleanup_cohort_p99_us"):
            require_number(errors, line, name, f"summary {key}")
    expected_summaries = {(mode, geometry) for mode in MODES for geometry in GEOMETRIES}
    if summary_keys != expected_summaries:
        errors.append("summary records do not cover every mode/geometry pair")
    return errors


def validate_native(text: str) -> list[str]:
    errors: list[str] = []
    lines = [line.strip() for line in text.splitlines() if line.strip()]
    starts = [line for line in lines if line.startswith("pass9_renderer_calibration ")]
    if len(starts) != 1:
        errors.append(f"expected exactly one native calibration start record, found {len(starts)}")
    elif any(token not in starts[0] for token in ("warmup_cycles=20", "measured_cycles=100", "cohorts=5", "performance_claim=false")):
        errors.append("native calibration start record is incomplete")
    observed: set[tuple[str, int]] = set()
    for line in lines:
        if not line.startswith("pass9_renderer_cohort "):
            continue
        geometry, cohort = field(line, "geometry"), field(line, "cohort")
        if geometry is None or cohort is None:
            errors.append("native cohort missing geometry or cohort")
            continue
        try:
            key = (geometry, int(cohort))
        except ValueError:
            errors.append(f"native cohort has invalid cohort={cohort}")
            continue
        if key in observed:
            errors.append(f"duplicate native cohort {key}")
        observed.add(key)
        for token in ("sample_count=100", "resource_return_every_cycle=true", "performance_claim=false"):
            if token not in line:
                errors.append(f"native cohort {key} missing {token}")
        for name in ("update_p50_us", "update_p95_us", "update_p99_us", "update_max_us",
                     "release_p50_us", "release_p95_us", "release_p99_us", "release_max_us", "max_dedicated_gpu_bytes"):
            require_number(errors, line, name, f"native cohort {key}")
    expected = {(geometry, cohort) for geometry in GEOMETRIES for cohort in COHORTS}
    if observed != expected:
        errors.append("native cohort records do not cover both geometries and all cohorts")
    return errors


def lifecycle_fixture() -> str:
    lines = [
        "pass9_preimplementation_calibration architecture=separate_client_cohort_process_plus_fresh_Runtime_worker_process warmup_cycles=20 measured_cycles=100 cohorts=5 geometries=120x40,80x24 percentile_method=nearest_rank rss_samples=5 performance_claim=false",
        "pass9_calibration_host macos_version=26.5 macos_build=25F84 model=Mac17,9 hardware=AppleM5Pro arch=aarch64 rust=rustc1.98 build_mode=release commit=abc master_baseline=def pass8_baseline=ghi performance_claim=false",
    ]
    samples = ",".join(["1"] * 100)
    for mode, geometry, cohort in sorted(expected_keys()):
        lines.append(
            f"pass9_calibration_cohort mode={mode} geometry={geometry} cohort={cohort} sample_count=100 "
            "reconnect_p50_us=1 reconnect_p95_us=2 reconnect_p99_us=3 reconnect_max_us=4 "
            "renderer_p50_us=1 renderer_p95_us=2 renderer_p99_us=3 renderer_max_us=4 "
            "cleanup_p50_us=1 cleanup_p95_us=2 cleanup_p99_us=3 cleanup_max_us=4 idle_runtime_cpu_percent=0 "
            "runtime_fds_baseline=1 runtime_fds_final=1 runtime_threads_baseline=1 runtime_threads_final=1 "
            "client_fds_baseline=1 client_fds_final=1 client_threads_baseline=1 client_threads_final=1 "
            "attachment_controller_fd_thread_return_each_cycle=true client_socket_closed_each_cycle=true performance_claim=false"
        )
        lines.append(
            f"pass9_calibration_rss_samples mode={mode} geometry={geometry} cohort={cohort} sample_count=100 "
            f"runtime_rss_kib={samples} runtime_slope_kib_per_cycle=0 client_rss_kib={samples} client_slope_kib_per_cycle=0 performance_claim=false"
        )
    for mode in MODES:
        for geometry in GEOMETRIES:
            lines.append(
                f"pass9_calibration_summary mode={mode} geometry={geometry} median_cohort_p99_us=3 "
                "median_renderer_cohort_p99_us=3 median_cleanup_cohort_p99_us=3 cohorts=5 cycles_per_cohort=100 performance_claim=false"
            )
    return "\n".join(lines)


def native_fixture() -> str:
    lines = ["pass9_renderer_calibration warmup_cycles=20 measured_cycles=100 cohorts=5 performance_claim=false"]
    for geometry in GEOMETRIES:
        for cohort in COHORTS:
            lines.append(
                f"pass9_renderer_cohort geometry={geometry} cohort={cohort} sample_count=100 resource_return_every_cycle=true "
                "update_p50_us=1 update_p95_us=2 update_p99_us=3 update_max_us=4 release_p50_us=1 release_p95_us=2 release_p99_us=3 release_max_us=4 max_dedicated_gpu_bytes=1 performance_claim=false"
            )
    return "\n".join(lines)


def self_test() -> int:
    lifecycle = lifecycle_fixture()
    native = native_fixture()
    assert not validate_lifecycle(lifecycle), validate_lifecycle(lifecycle)
    assert not validate_native(native), validate_native(native)
    assert validate_lifecycle(lifecycle.replace("cohort=5", "cohort=6", 1))
    assert validate_lifecycle(lifecycle.replace("runtime_fds_final=1", "runtime_fds_final=2", 1))
    assert validate_lifecycle(lifecycle.replace("runtime_rss_kib=" + ",".join(["1"] * 100), "runtime_rss_kib=1", 1))
    assert validate_native(native.replace("resource_return_every_cycle=true", "resource_return_every_cycle=false", 1))
    print("[seyal Pass-9 calibration coverage] self-test passed.")
    return 0


def main() -> int:
    args = sys.argv[1:]
    if args == ["--self-test"]:
        return self_test()
    if len(args) == 1:
        errors = validate_lifecycle(Path(args[0]).read_text(encoding="utf-8", errors="replace"))
    elif len(args) == 2 and args[0] == "--native":
        errors = validate_native(Path(args[1]).read_text(encoding="utf-8", errors="replace"))
    else:
        print(f"usage: {Path(sys.argv[0]).name} [--self-test|--native <benchmark-log>|<benchmark-log>]", file=sys.stderr)
        return 64
    if errors:
        print("Pass 9 calibration coverage validation failed:", file=sys.stderr)
        for error in errors:
            print(f"  {error}", file=sys.stderr)
        return 1
    print("[seyal Pass-9 calibration coverage] measured evidence shape passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
