#!/usr/bin/env python3
from __future__ import annotations

import os
import sys
from pathlib import Path
import tomllib

ROOT = Path(os.environ.get("SEYAL_VALIDATION_ROOT", Path(__file__).resolve().parents[1])).resolve()
WORKSPACE_MANIFEST = ROOT / "Cargo.toml"
TERMINAL_MANIFEST = ROOT / "crates" / "seyal-terminal" / "Cargo.toml"


def fail(message: str) -> None:
    print(f"[seyal workspace test] ERROR: {message}", file=sys.stderr)
    raise SystemExit(1)


if not WORKSPACE_MANIFEST.is_file():
    fail("missing root Cargo.toml; Issue #9 must establish the Rust workspace")

workspace_data = tomllib.loads(WORKSPACE_MANIFEST.read_text(encoding="utf-8"))
workspace = workspace_data.get("workspace")
if not isinstance(workspace, dict):
    fail("root Cargo.toml must define [workspace]")

members = workspace.get("members")
if not isinstance(members, list) or not members:
    fail("workspace must contain at least one explicit member")
if len(members) != len(set(members)):
    fail("workspace members must be unique")
if "crates/seyal-terminal" not in members:
    fail("seyal-terminal must be the first physical M001 ownership boundary")

for member in members:
    member_manifest = ROOT / member / "Cargo.toml"
    if not member_manifest.is_file():
        fail(f"workspace member {member!r} is missing Cargo.toml")

package_defaults = workspace_data.get("workspace", {}).get("package", {})
expected_defaults = {
    "edition": "2024",
    "rust-version": "1.98",
    "license": "Apache-2.0",
}
for key, expected in expected_defaults.items():
    if package_defaults.get(key) != expected:
        fail(f"workspace.package.{key} must be {expected!r}")

if not TERMINAL_MANIFEST.is_file():
    fail("missing crates/seyal-terminal/Cargo.toml")
terminal_data = tomllib.loads(TERMINAL_MANIFEST.read_text(encoding="utf-8"))
package = terminal_data.get("package", {})
if package.get("name") != "seyal-terminal":
    fail("terminal package must be named seyal-terminal")
if package.get("publish") is not False:
    fail("M001 scaffold crates must not be publishable packages")

print("[seyal workspace test] workspace ownership scaffold invariants passed.")
