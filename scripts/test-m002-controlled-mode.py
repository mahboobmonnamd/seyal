#!/usr/bin/env python3
"""Fixtures for the additive --controlled mode on the two M002 #673 runners.

scripts/run-m002-performance-contract.py and
scripts/run-m002-history-reflow-contract.py previously only ever emitted
environment_status=PLATFORM_LIMITED unconditionally (the history runner also
hardcoded a same-SHA self-baseline). --controlled is a new, explicit opt-in
mode that: (a) requires a --baseline-sha distinct from the candidate SHA,
(b) probes real host power/thermal state via `pmset -g batt` instead of
trusting a label, (c) only ever produces something other than
PLATFORM_LIMITED when that probe genuinely confirms AC power AND a distinct
baseline was supplied, (d) leaves the pre-existing always-diagnostic default
behavior byte-for-byte unchanged when --controlled is absent.

This never runs a real cargo bench: collect_cohorts()/collect_cohorts() are
monkeypatched to fast stubs, and the module's ROOT is monkeypatched to a
tempdir so no evidence is written into the real repository tree.
"""
from __future__ import annotations

import importlib.util
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(f"[seyal m002 controlled-mode test] ERROR: {message}")


def load_module(relative_path: str, name: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / relative_path)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    previous = sys.dont_write_bytecode
    sys.dont_write_bytecode = True
    try:
        spec.loader.exec_module(module)
    finally:
        sys.dont_write_bytecode = previous
    return module


def stub_collect_cohorts(gate: str, dest: Path, sha: str) -> str:
    # Evidence directories are timestamped to second resolution; keep
    # successive calls in this test from colliding on the same stamp.
    time.sleep(1.1)
    dest.mkdir(parents=True, exist_ok=True)
    for cohort in range(1, 6):
        (dest / f"{cohort}.toml").write_text(f"cohort = {cohort}\nsamples = [1.0]\n", encoding="utf-8")
    return f"[stub] collected {gate} at {sha}\n"


def test_performance_contract_runner(base: Path) -> None:
    module = load_module("scripts/run-m002-performance-contract.py", "seyal_run_m002_perf_unit")
    module.ROOT = base / "perf-contract-root"
    module.ROOT.mkdir()
    module.collect_cohorts = stub_collect_cohorts
    module.git_sha = lambda: "1111111111111111111111111111111111111111"
    # collect_gate() refuses real cohort collection off Apple Silicon macOS
    # (by design -- see require_apple_silicon_collection_host). This test
    # exercises --controlled's branch-selection logic, not real hardware
    # collection, and must run on any CI runner, so the host guard itself
    # (not collect_cohorts, which is already stubbed above) is bypassed here.
    module.require_apple_silicon_collection_host = lambda gate: None

    # (a) regression guard: without --controlled, behavior is unchanged --
    # unconditional PLATFORM_LIMITED, no power probe, no baseline requirement.
    probe_calls = {"count": 0}
    module.probe_ac_power_confirmed = lambda: (probe_calls.__setitem__("count", probe_calls["count"] + 1), (True, "AC Power"))[1]
    module.collect_gate("idle_cpu", allow_history=False)
    require(probe_calls["count"] == 0, "default (non-controlled) path must never probe AC power")
    default_evidence = sorted((module.ROOT / "docs" / "evidence").glob("m002-673-idle_cpu-*"))
    require(len(default_evidence) == 1, "default path did not write exactly one evidence directory")
    note = (default_evidence[0] / "PLATFORM_LIMITED.txt").read_text(encoding="utf-8")
    require("environment=PLATFORM_LIMITED" in note, "default path must remain PLATFORM_LIMITED")
    require("controlled_mode=true" not in note, "default path must not mention controlled_mode")

    # (b) --controlled with no distinct baseline still fails closed to
    # PLATFORM_LIMITED with a clear reason, even though the AC probe is
    # stubbed to succeed.
    module.collect_gate("idle_cpu", allow_history=False, controlled=True, baseline_sha=None)
    no_baseline_evidence = sorted((module.ROOT / "docs" / "evidence").glob("m002-673-idle_cpu-*"))
    require(len(no_baseline_evidence) == 2, "controlled-without-baseline path did not write a new evidence directory")
    latest = no_baseline_evidence[-1]
    require((latest / "PLATFORM_LIMITED.txt").is_file(), "controlled mode without a baseline must stay PLATFORM_LIMITED")
    reason = (latest / "PLATFORM_LIMITED.txt").read_text(encoding="utf-8")
    require("no --baseline-sha supplied" in reason, f"expected a clear no-baseline reason, got: {reason!r}")
    require("controlled_mode=true" in reason, "controlled mode fixture must record controlled_mode=true")

    # --controlled with baseline_sha equal to the candidate SHA must be
    # rejected outright (not silently treated as PLATFORM_LIMITED).
    raised = False
    try:
        module.collect_gate(
            "idle_cpu", allow_history=False, controlled=True, baseline_sha="1111111111111111111111111111111111111111"
        )
    except SystemExit as error:
        raised = True
        require("distinct" in str(error), f"expected a distinct-baseline rejection message, got: {error}")
    require(raised, "--controlled with baseline_sha == candidate sha must raise")

    # (c) --controlled with a distinct baseline AND a mocked AC-confirmed
    # probe can produce environment_status=VALID (something other than
    # PLATFORM_LIMITED).
    module.probe_ac_power_confirmed = lambda: (True, "AC Power")
    module.collect_gate(
        "idle_cpu",
        allow_history=False,
        controlled=True,
        baseline_sha="2222222222222222222222222222222222222222",
    )
    controlled_evidence = sorted((module.ROOT / "docs" / "evidence").glob("m002-673-idle_cpu-*"))
    require(len(controlled_evidence) == 4, "controlled-with-baseline path did not write a new evidence directory")
    controlled_note_path = controlled_evidence[-1] / "CONTROLLED.txt"
    require(controlled_note_path.is_file(), "controlled+baseline+AC-confirmed path must not stay PLATFORM_LIMITED")
    controlled_note = controlled_note_path.read_text(encoding="utf-8")
    require("environment_status=VALID" in controlled_note, f"expected environment_status=VALID, got: {controlled_note!r}")
    require(
        "baseline_sha=2222222222222222222222222222222222222222" in controlled_note,
        "controlled evidence must record the distinct baseline sha",
    )

    # AC power NOT confirmed, even with a distinct baseline, must still fail
    # closed to PLATFORM_LIMITED.
    module.probe_ac_power_confirmed = lambda: (False, "host is running on battery power, not AC")
    module.collect_gate(
        "idle_cpu",
        allow_history=False,
        controlled=True,
        baseline_sha="2222222222222222222222222222222222222222",
    )
    battery_evidence = sorted((module.ROOT / "docs" / "evidence").glob("m002-673-idle_cpu-*"))
    require(len(battery_evidence) == 5, "battery-power controlled path did not write a new evidence directory")
    battery_note = (battery_evidence[-1] / "PLATFORM_LIMITED.txt").read_text(encoding="utf-8")
    require("AC power not confirmed" in battery_note, f"expected an AC-power-not-confirmed reason, got: {battery_note!r}")

    print("[seyal m002 controlled-mode test] run-m002-performance-contract.py --controlled verified.")


def test_history_reflow_runner(base: Path) -> None:
    module = load_module("scripts/run-m002-history-reflow-contract.py", "seyal_run_m002_history_unit")
    module.ROOT = base / "history-reflow-root"
    module.ROOT.mkdir()
    module.collect_cohorts = stub_collect_cohorts
    module.git_sha = lambda: "3333333333333333333333333333333333333333"
    # See the matching comment in test_performance_contract_runner: main()'s
    # Apple Silicon host guard is bypassed here so this test can exercise
    # --controlled's branch-selection logic on any CI runner.
    module.require_apple_silicon_collection_host = lambda: None

    # Stub the validator subprocess call so this test does not depend on the
    # real validator's exact acceptance path for a fabricated baseline SHA
    # that has no real git checkout; the point of this fixture is the
    # --controlled branch selection logic in main(), not the validator.
    import types

    def fake_run(command, *, env=None):
        if command[0] == "python3" and "check-m002-performance-contract.py" in command[1]:
            record_path = Path(command[3])
            text = record_path.read_text(encoding="utf-8")
            status = "PLATFORM_LIMITED" if "environment_status = 'PLATFORM_LIMITED'" in text else "VALID-evaluated"
            gate_line = next(line for line in text.splitlines() if line.startswith("gate ="))
            gate = gate_line.split("=", 1)[1].strip().strip("'\"")
            stdout = f"M002 performance result: {status} metric={gate} samples=500 cohorts=5\n"
            return types.SimpleNamespace(returncode=0, stdout=stdout)
        return module._real_run(command, env=env)

    module._real_run = module.run
    module.run = fake_run

    baseline_dir = base / "baseline-cohorts-source"
    for gate in module.GATES:
        gate_dir = baseline_dir / gate
        gate_dir.mkdir(parents=True)
        for cohort in range(1, 6):
            (gate_dir / f"{cohort}.toml").write_text(f"cohort = {cohort}\nsamples = [1.0]\n", encoding="utf-8")

    original_argv = sys.argv
    try:
        # (a) regression guard: default behavior (no --controlled) is
        # byte-for-byte the pre-existing always-PLATFORM_LIMITED, same-SHA
        # self-baseline path.
        module.probe_ac_power_confirmed = lambda: (True, "AC Power")
        sys.argv = ["run-m002-history-reflow-contract.py"]
        module.main()
        default_roots = sorted((module.ROOT / "docs" / "evidence").glob("m002-673-history-reflow-*"))
        require(len(default_roots) == 1, "default history-reflow run did not write exactly one evidence root")
        default_record = (default_roots[0] / "history_active_reflow_ms" / "record.toml").read_text(encoding="utf-8")
        require('baseline_sha = "3333333333333333333333333333333333333333"' in default_record, "default path must keep the same-SHA self-baseline")
        require("environment_status = 'PLATFORM_LIMITED'" in default_record, "default path must remain PLATFORM_LIMITED")

        # (b) --controlled with no baseline still fails closed.
        time.sleep(1.1)
        sys.argv = ["run-m002-history-reflow-contract.py", "--controlled"]
        module.main()
        no_baseline_roots = sorted((module.ROOT / "docs" / "evidence").glob("m002-673-history-reflow-*"))
        require(len(no_baseline_roots) == 2, "controlled-without-baseline run did not write a new evidence root")
        no_baseline_record = (no_baseline_roots[-1] / "history_active_reflow_ms" / "record.toml").read_text(encoding="utf-8")
        require("environment_status = 'PLATFORM_LIMITED'" in no_baseline_record, "controlled mode without a baseline must stay PLATFORM_LIMITED")
        require("no --baseline-sha supplied" in no_baseline_record, "expected a clear no-baseline reason in platform_limit_reason")

        # --controlled with baseline_sha == candidate sha must be rejected.
        time.sleep(1.1)
        sys.argv = [
            "run-m002-history-reflow-contract.py",
            "--controlled",
            "--baseline-sha",
            "3333333333333333333333333333333333333333",
            "--baseline-cohorts-dir",
            str(baseline_dir),
        ]
        raised = False
        try:
            module.main()
        except SystemExit as error:
            raised = True
            require("distinct" in str(error), f"expected distinct-baseline rejection, got: {error}")
        require(raised, "--controlled with baseline_sha == candidate sha must raise")

        # (c) --controlled with a distinct baseline and AC-confirmed probe
        # can become VALID (not PLATFORM_LIMITED).
        time.sleep(1.1)
        sys.argv = [
            "run-m002-history-reflow-contract.py",
            "--controlled",
            "--baseline-sha",
            "4444444444444444444444444444444444444444",
            "--baseline-cohorts-dir",
            str(baseline_dir),
        ]
        module.main()
        controlled_roots = sorted((module.ROOT / "docs" / "evidence").glob("m002-673-history-reflow-*"))
        require(len(controlled_roots) == 4, "controlled-with-baseline run did not write a new evidence root")
        controlled_record = (controlled_roots[-1] / "history_active_reflow_ms" / "record.toml").read_text(encoding="utf-8")
        require("environment_status = 'VALID'" in controlled_record, f"expected environment_status = 'VALID', got: {controlled_record!r}")
        require(
            'baseline_sha = "4444444444444444444444444444444444444444"' in controlled_record,
            "controlled record must carry the distinct baseline sha",
        )
    finally:
        sys.argv = original_argv

    print("[seyal m002 controlled-mode test] run-m002-history-reflow-contract.py --controlled verified.")


def main() -> None:
    with tempfile.TemporaryDirectory(prefix="seyal-m002-controlled-mode-") as tmp:
        base = Path(tmp)
        test_performance_contract_runner(base)
        test_history_reflow_runner(base)
    print("[seyal m002 controlled-mode test] all --controlled fixtures passed for both runners.")


if __name__ == "__main__":
    main()
