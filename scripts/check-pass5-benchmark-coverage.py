#!/usr/bin/env python3
from __future__ import annotations

import re
import sys
from pathlib import Path

RESULT_PREFIX = "pass5_production_result "
REQUIRED_SUSTAINED_FANOUT = {1, 2, 4, 8, 16}
MIN_SUSTAINED_SAMPLES_PER_VIEWER = 100


def fields(line: str) -> dict[str, str]:
    return dict(re.findall(r"([A-Za-z0-9_]+)=([^\s]+)", line))


def validate_lines(lines: list[str]) -> list[str]:
    errors: list[str] = []
    sustained_fanout: set[int] = set()
    saw_streaming = False

    for line in lines:
        if not line.startswith(RESULT_PREFIX):
            continue
        result = fields(line)
        if result.get("classification") != "MEASURED":
            continue
        workload = result.get("workload")
        if workload in {None, "interactive"}:
            continue

        saw_streaming = True
        try:
            sample_count = int(result["latency_sample_count"])
            display_batches = int(result["display_batches_received"])
            fanout = int(result["fanout_attached"])
        except (KeyError, ValueError) as error:
            errors.append(f"malformed streaming result: {error}: {line}")
            continue

        if sample_count != display_batches:
            errors.append(
                f"{workload} fanout={fanout}: latency samples {sample_count} != "
                f"client-visible display batches {display_batches}; percentile coverage is incomplete"
            )
        if sample_count == 0:
            errors.append(f"{workload} fanout={fanout}: no latency samples")

        if workload == "sustained_high_output_2s":
            sustained_fanout.add(fanout)
            minimum = fanout * MIN_SUSTAINED_SAMPLES_PER_VIEWER
            if sample_count < minimum:
                errors.append(
                    f"{workload} fanout={fanout}: {sample_count} samples < required {minimum} "
                    f"({MIN_SUSTAINED_SAMPLES_PER_VIEWER} per viewer)"
                )

    if not saw_streaming:
        errors.append("no measured streaming Pass-5 result lines found")
    missing = REQUIRED_SUSTAINED_FANOUT - sustained_fanout
    if missing:
        errors.append(f"missing measured sustained fanout cases: {sorted(missing)}")
    return errors


def self_test() -> None:
    good = [
        (
            "pass5_production_result workload=sustained_high_output_2s "
            f"classification=MEASURED fanout_attached={fanout} "
            f"latency_sample_count={fanout * 120} display_batches_received={fanout * 120}"
        )
        for fanout in sorted(REQUIRED_SUSTAINED_FANOUT)
    ]
    good.append(
        "pass5_production_result workload=normal_command classification=MEASURED "
        "fanout_attached=16 latency_sample_count=48 display_batches_received=48"
    )
    assert not validate_lines(good), validate_lines(good)

    undercovered = good.copy()
    undercovered[0] = (
        "pass5_production_result workload=sustained_high_output_2s classification=MEASURED "
        "fanout_attached=1 latency_sample_count=119 display_batches_received=120"
    )
    assert validate_lines(undercovered), "under-covered latency population was accepted"

    too_small = good.copy()
    too_small[0] = (
        "pass5_production_result workload=sustained_high_output_2s classification=MEASURED "
        "fanout_attached=1 latency_sample_count=99 display_batches_received=99"
    )
    assert validate_lines(too_small), "statistically weak sustained population was accepted"

    print("[seyal Pass-5 benchmark coverage] self-test passed.")


def main() -> None:
    if len(sys.argv) == 2 and sys.argv[1] == "--self-test":
        self_test()
        return
    if len(sys.argv) != 2:
        raise SystemExit("usage: check-pass5-benchmark-coverage.py <benchmark-log>|--self-test")

    path = Path(sys.argv[1])
    lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    errors = validate_lines(lines)
    if errors:
        print("Pass-5 benchmark latency coverage violations:")
        for error in errors:
            print(f"  {error}")
        raise SystemExit(1)
    print("[seyal Pass-5 benchmark coverage] all measured streaming display batches are latency-sampled; sustained sample population is sufficient.")


if __name__ == "__main__":
    main()
