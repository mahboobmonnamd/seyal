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
RUNTIME_MANIFEST = ROOT / "crates" / "seyal-runtime" / "Cargo.toml"


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
for required in ("crates/seyal-terminal", "crates/seyal-exec", "crates/seyal-runtime"):
    if required not in members:
        fail(f"required production ownership boundary missing: {required}")

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

if not TERMINAL_MANIFEST.is_file() or not EXEC_MANIFEST.is_file() or not RUNTIME_MANIFEST.is_file():
    fail("terminal, exec and runtime manifests must all exist")
terminal_data = tomllib.loads(TERMINAL_MANIFEST.read_text(encoding="utf-8"))
exec_data = tomllib.loads(EXEC_MANIFEST.read_text(encoding="utf-8"))
runtime_data = tomllib.loads(RUNTIME_MANIFEST.read_text(encoding="utf-8"))
for data, expected_name in (
    (terminal_data, "seyal-terminal"),
    (exec_data, "seyal-exec"),
    (runtime_data, "seyal-runtime"),
):
    package = data.get("package", {})
    if package.get("name") != expected_name:
        fail(f"package must be named {expected_name}")
    if package.get("publish") is not False:
        fail("M001 crates must not be publishable packages")

exec_dependencies = exec_data.get("dependencies", {})
if set(exec_dependencies) != {"seyal-terminal"}:
    fail("seyal-exec must depend on exactly seyal-terminal among portable dependencies")
if exec_dependencies["seyal-terminal"].get("path") != "../seyal-terminal":
    fail("seyal-exec must consume seyal-terminal through the local workspace path")

runtime_dependencies = runtime_data.get("dependencies", {})
if set(runtime_dependencies) != {"seyal-exec"}:
    fail("seyal-runtime must depend on exactly seyal-exec among portable dependencies")
if runtime_dependencies["seyal-exec"].get("path") != "../seyal-exec":
    fail("seyal-runtime must consume seyal-exec through the local workspace path")

for name, data in (("seyal-exec", exec_data), ("seyal-runtime", runtime_data)):
    macos_dependencies = data.get("target", {}).get('cfg(target_os = "macos")', {}).get(
        "dependencies", {}
    )
    if set(macos_dependencies) != {"libc"}:
        fail(f"{name} macOS platform boundary may depend only on libc in M001")
    if macos_dependencies["libc"] != "=0.2.189":
        fail(f"{name} must exactly pin the reviewed libc 0.2.189 dependency")

for crate in ("seyal-exec", "seyal-runtime"):
    src = ROOT / "crates" / crate / "src"
    if not src.is_dir():
        fail(f"{crate} source directory is missing")
    for path in sorted(src.rglob("*.rs")):
        text = path.read_text(encoding="utf-8")
        if "RILL_" in text:
            fail(f"legacy RILL environment identifier found in {path.relative_to(ROOT)}")

exec_lib_text = (ROOT / "crates" / "seyal-exec" / "src" / "lib.rs").read_text(encoding="utf-8")
if "pub use endpoint::TerminalEndpoint" in exec_lib_text:
    fail("TerminalEndpoint must not be exported as a public construction surface")

runtime_src = "\n".join(
    path.read_text(encoding="utf-8")
    for path in (ROOT / "crates" / "seyal-runtime" / "src").rglob("*.rs")
)
if "TerminalState::new" in runtime_src:
    fail("seyal-runtime must not construct a second authoritative TerminalState")
if "tokio" in runtime_src.lower() or "mio::" in runtime_src:
    fail("Pass 4 Runtime must not introduce Tokio/Mio")

print("[seyal workspace test] workspace ownership scaffold invariants passed.")
