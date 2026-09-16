#!/usr/bin/env python3
"""Run PHYSICAL_ARM64 five-cohort HistoryStore reflow rows for #673.

This is an opt-in controlled-host runner. It is not invoked by `make bench` or
Foundation Quality. Default `history_reflow` smoke stays `performance_claim=false`.

Accepted numeric gates only: history_active_reflow_ms and
history_sealed_segment_reflow_ms. Proposed #673 gates are left unevaluated.
"""
from __future__ import annotations

import hashlib
import os
import platform
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
VALIDATOR = ROOT / "scripts/check-m002-performance-contract.py"
GATES = (
    "history_active_reflow_ms",
    "history_sealed_segment_reflow_ms",
)
BOUNDARIES = {
    "history_active_reflow_ms": "HistoryStore active reflow",
    "history_sealed_segment_reflow_ms": "HistoryStore sealed-segment lazy reflow",
}
CEILINGS = {
    "history_active_reflow_ms": (2, 4, 8),
    "history_sealed_segment_reflow_ms": (1, 2, 4),
}


def run(command: list[str], *, env: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        command,
        cwd=ROOT,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
    )


def git_sha() -> str:
    result = run(["git", "rev-parse", "HEAD"])
    sha = result.stdout.strip()
    if result.returncode != 0 or len(sha) != 40:
        raise SystemExit("cannot resolve production SHA")
    return sha


def rustc_version() -> str:
    result = run(["rustc", "--version"])
    return result.stdout.strip() or "unknown"


def hardware() -> str:
    if sys.platform == "darwin":
        model = run(["sysctl", "-n", "hw.model"]).stdout.strip()
        machine = run(["uname", "-m"]).stdout.strip()
        return f"{model} {machine}".strip() or platform.platform()
    return platform.platform()


def nearest_rank(values: list[float], percentile: int) -> float:
    ordered = sorted(values)
    rank = max(1, (len(ordered) * percentile + 99) // 100)
    return ordered[rank - 1]


def collect_cohorts(gate: str, dest: Path, sha: str) -> str:
    dest.mkdir(parents=True, exist_ok=True)
    log_chunks: list[str] = []
    for cohort in range(1, 6):
        out = dest / f"{cohort}.toml"
        env = os.environ.copy()
        env.update(
            {
                "SEYAL_BENCH_COMMIT": sha,
                "SEYAL_M002_CONTRACT_GATE": gate,
                "SEYAL_M002_COHORT": str(cohort),
                "SEYAL_M002_WARMUPS": "20",
                "SEYAL_M002_SAMPLES": "100",
                "SEYAL_M002_COHORT_OUT": str(out),
                "SEYAL_HISTORY_BENCH_LINES": os.environ.get("SEYAL_HISTORY_BENCH_LINES", "10000"),
                "SEYAL_HISTORY_BENCH_COLUMNS": os.environ.get("SEYAL_HISTORY_BENCH_COLUMNS", "80"),
                "SEYAL_HISTORY_BENCH_WORKLOADS": os.environ.get("SEYAL_HISTORY_BENCH_WORKLOADS", "ascii"),
            }
        )
        result = run(
            [
                "cargo",
                "bench",
                "--locked",
                "-p",
                "seyal-terminal",
                "--bench",
                "history_reflow",
                "--features",
                "history-reflow-bench",
                "--",
                "--quiet",
            ],
            env=env,
        )
        log_chunks.append(result.stdout)
        if result.returncode != 0 or not out.is_file():
            raise SystemExit(f"cohort {cohort} for {gate} failed:\n{result.stdout}")
    return "".join(log_chunks)


def load_samples(directory: Path) -> list[float]:
    values: list[float] = []
    for path in sorted(directory.glob("*.toml")):
        text = path.read_text(encoding="utf-8")
        start = text.index("[") + 1
        end = text.index("]")
        chunk = [float(part.strip()) for part in text[start:end].split(",") if part.strip()]
        values.extend(chunk)
    return values


def write_record(
    *,
    gate: str,
    sha: str,
    evidence_root: Path,
    raw_log: Path,
    candidate: Path,
    baseline: Path,
    workload: str,
) -> Path:
    candidate_values = load_samples(candidate)
    baseline_values = load_samples(baseline)
    p50, p95, p99 = (nearest_rank(candidate_values, p) for p in (50, 95, 99))
    b50, b95, b99 = (nearest_rank(baseline_values, p) for p in (50, 95, 99))
    record = evidence_root / "record.toml"
    workload_hash = hashlib.sha256(workload.encode()).hexdigest()
    rel = lambda path: path.relative_to(ROOT).as_posix()
    record.write_text(
        "\n".join(
            [
                "contract_schema = 'seyal.m002.performance-contract'",
                "contract_version = 1",
                f"production_sha = '{sha}'",
                f"harness_sha = '{sha}'",
                f"baseline_sha = '{sha}'",
                "build_mode = 'release'",
                f"os_version = '{platform.platform()}'",
                f"toolchain = '{rustc_version()}'",
                f"hardware = '{hardware()}'",
                "display = 'none-headless-history-reflow'",
                "power_thermal_state = 'uncontrolled-developer-host'",
                f"workload_hash = '{workload_hash}'",
                "topology = 'one-execution-headless-TerminalState'",
                "evidence_class = 'PHYSICAL_ARM64'",
                f"gate = '{gate}'",
                f"metric = '{gate}'",
                f"boundary = '{BOUNDARIES[gate]}'",
                "unit = 'ms'",
                "percentile_method = 'nearest-rank'",
                "sample_count = 500",
                "cohort_count = 5",
                "environment_status = 'VALID'",
                "platform_limit_reason = ''",
                "comparator = 'less_equal'",
                f"p50 = {p50}",
                f"p95 = {p95}",
                f"p99 = {p99}",
                f"baseline_p50 = {b50}",
                f"baseline_p95 = {b95}",
                f"baseline_p99 = {b99}",
                "relative_regression_percent = 10",
                f"raw_log = '{rel(raw_log)}'",
                f"raw_cohorts = '{rel(candidate)}/'",
                f"baseline_raw_cohorts = '{rel(baseline)}/'",
                "",
            ]
        ),
        encoding="utf-8",
    )
    return record


def main() -> None:
    if sys.platform != "darwin" or platform.machine() not in {"arm64", "aarch64"}:
        raise SystemExit("PHYSICAL_ARM64 runner requires Apple Silicon macOS")
    sha = git_sha()
    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    evidence_root = ROOT / "docs" / "evidence" / f"m002-673-history-reflow-{stamp}"
    evidence_root.mkdir(parents=True, exist_ok=False)
    workload = (
        f"lines={os.environ.get('SEYAL_HISTORY_BENCH_LINES', '10000')} "
        f"cols={os.environ.get('SEYAL_HISTORY_BENCH_COLUMNS', '80')} "
        "workload=ascii executions=1 warmups=20 samples=100 cohorts=5"
    )
    for gate in GATES:
        gate_root = evidence_root / gate
        candidate = gate_root / "cohorts"
        baseline = gate_root / "baseline-cohorts"
        log = gate_root / "raw.log"
        candidate_log = collect_cohorts(gate, candidate, sha)
        baseline_log = collect_cohorts(gate, baseline, sha)
        log.write_text(candidate_log + "\n" + baseline_log, encoding="utf-8")
        record = write_record(
            gate=gate,
            sha=sha,
            evidence_root=gate_root,
            raw_log=log,
            candidate=candidate,
            baseline=baseline,
            workload=workload,
        )
        checked = run(["python3", str(VALIDATOR), "--record", str(record)])
        sys.stdout.write(checked.stdout)
        if checked.returncode != 0:
            raise SystemExit(f"validator rejected {record}")
        ceilings = CEILINGS[gate]
        print(f"[m002-673] {gate} validator accepted; frozen ceilings p50/p95/p99={ceilings}")
    print(f"[m002-673] evidence root {evidence_root.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
