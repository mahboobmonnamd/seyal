#!/usr/bin/env python3
from __future__ import annotations

import os
import sys
from pathlib import Path
import tomllib

ROOT = Path(os.environ.get("SEYAL_VALIDATION_ROOT", Path(__file__).resolve().parents[1])).resolve()
WORKSPACE_MANIFEST = ROOT / "Cargo.toml"
TERMINAL_MANIFEST = ROOT / "crates" / "seyal-terminal" / "Cargo.toml"
EXEC_MANIFEST = ROOT / "crates" / "seyal-exec" / "Cargo.toml"


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
    fail("seyal-terminal must be a physical M001 ownership boundary")
if "crates/seyal-exec" not in members:
    fail("Issue #28 requires seyal-exec as the PTY/child ownership boundary")

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
terminal_package = terminal_data.get("package", {})
if terminal_package.get("name") != "seyal-terminal":
    fail("terminal package must be named seyal-terminal")
if terminal_package.get("publish") is not False:
    fail("M001 crates must not be publishable packages")

if not EXEC_MANIFEST.is_file():
    fail("missing crates/seyal-exec/Cargo.toml")
exec_data = tomllib.loads(EXEC_MANIFEST.read_text(encoding="utf-8"))
exec_package = exec_data.get("package", {})
if exec_package.get("name") != "seyal-exec":
    fail("execution package must be named seyal-exec")
if exec_package.get("publish") is not False:
    fail("M001 crates must not be publishable packages")

exec_dependencies = exec_data.get("dependencies", {})
if set(exec_dependencies) != {"seyal-terminal"}:
    fail("seyal-exec must depend on exactly seyal-terminal among portable dependencies")
terminal_dependency = exec_dependencies["seyal-terminal"]
if not isinstance(terminal_dependency, dict) or terminal_dependency.get("path") != "../seyal-terminal":
    fail("seyal-exec must consume seyal-terminal through the local workspace path")

macos_dependencies = exec_data.get("target", {}).get("cfg(target_os = \"macos\")", {}).get(
    "dependencies", {}
)
if set(macos_dependencies) != {"libc"}:
    fail("seyal-exec macOS platform boundary may depend only on libc in M001")
if macos_dependencies["libc"] != "=0.2.189":
    fail("seyal-exec must exactly pin the reviewed libc 0.2.189 dependency")

exec_src = ROOT / "crates" / "seyal-exec" / "src"
if not exec_src.is_dir():
    fail("seyal-exec source directory is missing")
for path in sorted(exec_src.rglob("*.rs")):
    text = path.read_text(encoding="utf-8")
    if "RILL_" in text:
        fail(f"legacy RILL environment identifier found in {path.relative_to(ROOT)}")

lib_text = (exec_src / "lib.rs").read_text(encoding="utf-8")
if "TerminalEndpoint" in lib_text.split("pub use")[-1]:
    fail("TerminalEndpoint must not be exported as a public construction surface")

print("[seyal workspace test] workspace ownership scaffold invariants passed.")
