#!/usr/bin/env python3
"""Negative-path test for ensure_seyal_app()'s binary identity verification.

`ensure_seyal_app()` in scripts/run-m002-performance-contract.py must refuse
to reuse an existing Seyal.app binary unless a fresh, matching identity
manifest (sha, configuration, content sha256) proves it is the requested
production build. Before this fix it reused any pre-existing executable at
the hardcoded Debug app path with zero identity check.

This test never runs a real Xcode build: it monkeypatches BUILD_MACOS to a
trivial stub script (that just writes a marker file) and SEYAL_APP to a
tempdir path, then drives the real ensure_seyal_app()/app_identity_matches()
logic to prove every stale-identity case forces a rebuild rather than
silently reusing the existing binary.
"""
from __future__ import annotations

import importlib.util
import json
import stat
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(f"[seyal m002 app identity test] ERROR: {message}")


def load_module():
    spec = importlib.util.spec_from_file_location(
        "seyal_run_m002_performance_contract_unit",
        ROOT / "scripts/run-m002-performance-contract.py",
    )
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    previous_dont_write_bytecode = sys.dont_write_bytecode
    sys.dont_write_bytecode = True
    try:
        spec.loader.exec_module(module)
    finally:
        sys.dont_write_bytecode = previous_dont_write_bytecode
    return module


def write_stub_build_script(path: Path, app_path: Path, marker: str) -> None:
    path.write_text(
        "\n".join(
            [
                "#!/usr/bin/env bash",
                "set -euo pipefail",
                f'mkdir -p "{app_path.parent}"',
                f'printf %s "{marker}" > "{app_path}"',
                f'chmod +x "{app_path}"',
                "",
            ]
        ),
        encoding="utf-8",
    )
    path.chmod(path.stat().st_mode | stat.S_IEXEC)


def main() -> None:
    with tempfile.TemporaryDirectory(prefix="seyal-m002-app-identity-") as tmp:
        base = Path(tmp)
        app_path = base / "derived-data/Seyal.app/Contents/MacOS/Seyal"
        build_script = base / "fake-build-macos.sh"

        module = load_module()
        module.SEYAL_APP = app_path
        module.BUILD_MACOS = build_script
        module.git_sha = lambda: "1111111111111111111111111111111111111111"

        build_calls = {"count": 0}
        real_run = module.run

        def counting_run(command, *, env=None):
            build_calls["count"] += 1
            return real_run(command, env=env)

        module.run = counting_run

        # Case 1: no binary and no manifest at all -> must build.
        write_stub_build_script(build_script, app_path, "binary-v1")
        first = module.ensure_seyal_app()
        require(first == app_path, "ensure_seyal_app did not return the expected app path")
        require(build_calls["count"] == 1, "ensure_seyal_app did not build when no binary existed")
        require(module.app_manifest_path().is_file(), "ensure_seyal_app did not write an identity manifest")

        # Case 2: a fresh, matching manifest -> must reuse without rebuilding.
        second = module.ensure_seyal_app()
        require(build_calls["count"] == 1, "ensure_seyal_app rebuilt despite a fresh matching manifest")
        require(second == app_path, "ensure_seyal_app did not return the expected app path")

        # Case 3 (the original bug): the binary on disk changed after the
        # manifest was written (content hash no longer matches). Zero
        # identity check previously meant a stale binary like this one would
        # be reused blindly.
        app_path.write_text("tampered-binary-content-not-what-was-built", encoding="utf-8")
        third = module.ensure_seyal_app()
        require(
            build_calls["count"] == 2,
            "ensure_seyal_app reused a binary whose content hash no longer matches its manifest",
        )
        require(third == app_path, "ensure_seyal_app did not return the expected app path")

        # Case 4: manifest names a different SHA than requested -> rebuild.
        manifest = module.load_app_manifest()
        require(manifest is not None, "expected a manifest after case 3's rebuild")
        stale_sha_manifest = dict(manifest)
        stale_sha_manifest["sha"] = "2222222222222222222222222222222222222222"
        module.app_manifest_path().write_text(json.dumps(stale_sha_manifest), encoding="utf-8")
        fourth = module.ensure_seyal_app()
        require(build_calls["count"] == 3, "ensure_seyal_app reused a binary built for a different production SHA")
        require(fourth == app_path, "ensure_seyal_app did not return the expected app path")

        # Case 5: manifest names a different configuration (e.g. Debug) than
        # requested (Release) -> rebuild.
        manifest = module.load_app_manifest()
        require(manifest is not None, "expected a manifest after case 4's rebuild")
        stale_configuration_manifest = dict(manifest)
        stale_configuration_manifest["configuration"] = "Debug"
        module.app_manifest_path().write_text(json.dumps(stale_configuration_manifest), encoding="utf-8")
        fifth = module.ensure_seyal_app()
        require(
            build_calls["count"] == 4,
            "ensure_seyal_app reused a binary built with a different configuration",
        )
        require(fifth == app_path, "ensure_seyal_app did not return the expected app path")

        # Case 6: manifest missing entirely (e.g. a binary left over from
        # before this fix existed) -> rebuild rather than trusting it.
        module.app_manifest_path().unlink()
        sixth = module.ensure_seyal_app()
        require(build_calls["count"] == 5, "ensure_seyal_app reused a binary with no manifest at all")
        require(sixth == app_path, "ensure_seyal_app did not return the expected app path")

    print(
        "[seyal m002 app identity test] stale/mismatched manifest and binary "
        "correctly forced a rebuild in every case; a fresh matching manifest was reused."
    )


if __name__ == "__main__":
    main()
