#!/usr/bin/env python3
"""Merge Pass 9 single-cohort release-qualification partials into one artifact."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("partials", nargs="+", type=Path)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--pass8-delta-percent", type=float)
    parser.add_argument("--pass8-explanation")
    parser.add_argument("--pass8-cohorts", type=int, default=5)
    args = parser.parse_args()

    cohorts: list[dict[str, Any]] = []
    notes: list[str] = []
    recovery: dict[str, Any] | None = None
    for path in args.partials:
        document = json.loads(path.read_text(encoding="utf-8"))
        if document.get("schema") != "seyal.pass9.production-budget.v1":
            print(f"unexpected schema in {path}", file=sys.stderr)
            return 1
        if document.get("commit") != args.commit:
            print(f"commit mismatch in {path}", file=sys.stderr)
            return 1
        partial_recovery = document.get("recovery")
        if not isinstance(partial_recovery, dict):
            print(f"missing recovery in {path}", file=sys.stderr)
            return 1
        if recovery is None:
            recovery = partial_recovery
        elif recovery != partial_recovery:
            print(f"recovery mismatch in {path}", file=sys.stderr)
            return 1
        partial_cohorts = document.get("cohorts")
        if not isinstance(partial_cohorts, list) or not partial_cohorts:
            print(f"missing cohorts in {path}", file=sys.stderr)
            return 1
        cohorts.extend(partial_cohorts)
        note = document.get("topology_note")
        if isinstance(note, str) and note and note not in notes:
            notes.append(note)

    if recovery is None:
        print("no partials supplied recovery", file=sys.stderr)
        return 1

    artifact: dict[str, Any] = {
        "schema": "seyal.pass9.production-budget.v1",
        "measurement_source": "supplied_exact_head_evidence",
        "commit": args.commit,
        "recovery": recovery,
        "cohorts": cohorts,
    }
    if notes:
        artifact["topology_note"] = " | ".join(notes)

    if args.pass8_delta_percent is not None:
        pass8: dict[str, Any] = {
            "paired_delta_percent": args.pass8_delta_percent,
            "gate": "ENFORCED_CONTROLLED_HOST",
            "cohorts": args.pass8_cohorts,
        }
        if args.pass8_explanation:
            pass8["root_cause_explanation"] = args.pass8_explanation
        artifact["pass8"] = pass8
    else:
        artifact["pass8"] = {
            "paired_delta_percent": 0.0,
            "gate": "PENDING_CONTROLLED_HOST",
            "cohorts": 0,
            "root_cause_explanation": "Pass 8 attribution not yet collected for this head",
        }

    args.output.write_text(json.dumps(artifact, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"[pass9-release-qualification] merged {len(cohorts)} cohorts -> {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
