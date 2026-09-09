#!/usr/bin/env python3
from __future__ import annotations

import os
import shutil
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ENV_ROOT = "SEYAL_VALIDATION_ROOT"


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(f"[seyal CI validator self-test] ERROR: {message}")


def run_negative(command: list[str], fixture_root: Path, expected: str) -> None:
    env = os.environ.copy()
    env[ENV_ROOT] = str(fixture_root)
    result = subprocess.run(
        command,
        cwd=fixture_root,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
    )
    require(result.returncode != 0, f"negative fixture unexpectedly passed: {' '.join(command)}")
    require(expected in result.stdout, f"negative fixture failed for the wrong reason; expected {expected!r}, output was:\n{result.stdout}")


def write(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


def main() -> None:
    with tempfile.TemporaryDirectory(prefix="seyal-ci-validator-") as tmp:
        base = Path(tmp)

        governance = base / "governance"
        governance.mkdir()
        run_negative(["bash", str(ROOT / "scripts/validate-governance.sh")], governance, "missing required file: AGENTS.md")

        docs = base / "doc-links"
        docs.mkdir()
        write(docs / "README.md", "[broken local link](missing.md)\n")
        run_negative(["python3", str(ROOT / "scripts/check-doc-links.py")], docs, "Broken local Markdown links:")

        layering = base / "layering-terminal"
        write(layering / "crates/seyal-terminal/Cargo.toml", '[package]\nname = "seyal-terminal"\nversion = "0.0.0"\n\n[dependencies]\nseyal-runtime = { path = "../seyal-runtime" }\n')
        run_negative(["python3", str(ROOT / "scripts/check-layering.py")], layering, "seyal-terminal has forbidden dependencies: seyal-runtime")

        exec_layering = base / "layering-exec"
        write(exec_layering / "crates/seyal-exec/Cargo.toml", '[package]\nname = "seyal-exec"\nversion = "0.0.0"\n\n[dependencies]\nseyal-runtime = { path = "../seyal-runtime" }\n')
        run_negative(["python3", str(ROOT / "scripts/check-layering.py")], exec_layering, "seyal-exec has forbidden dependencies: seyal-runtime")

        client_layering = base / "layering-client"
        write(client_layering / "crates/seyal-client/Cargo.toml", '[package]\nname = "seyal-client"\nversion = "0.0.0"\n\n[dependencies]\nseyal-runtime = { path = "../seyal-runtime" }\n')
        run_negative(["python3", str(ROOT / "scripts/check-layering.py")], client_layering, "seyal-client has forbidden dependencies: seyal-runtime")

        protocol_layering = base / "layering-protocol"
        write(protocol_layering / "crates/seyal-protocol/Cargo.toml", '[package]\nname = "seyal-protocol"\nversion = "0.0.0"\n\n[dependencies]\nseyal-runtime = { path = "../seyal-runtime" }\n')
        run_negative(["python3", str(ROOT / "scripts/check-layering.py")], protocol_layering, "seyal-protocol has forbidden dependencies: seyal-runtime")

        unknown_layering = base / "layering-unknown"
        write(unknown_layering / "crates/seyal-mystery/Cargo.toml", '[package]\nname = "seyal-mystery"\nversion = "0.0.0"\n')
        run_negative(["python3", str(ROOT / "scripts/check-layering.py")], unknown_layering, "seyal-mystery has no architecture layering rule")

        hot = base / "hot-path"
        write(hot / "crates/seyal-terminal/src/terminal.rs", "impl TerminalState { pub fn feed(&mut self, bytes: &[u8]) { let _ = bytes.to_vec(); } pub fn finish_input(&mut self) {} }")
        write(
            hot / "crates/seyal-runtime/src/runtime/mod.rs",
            "impl Runtime { pub fn poll_once(&mut self) {} }",
        )
        write(
            hot / "crates/seyal-runtime/src/runtime/reactor_io.rs",
            "impl Runtime { fn drain_control(&mut self) {} fn service_reads(&mut self) {} fn service_writes(&mut self) {} }",
        )
        write(hot / "crates/seyal-runtime/src/input.rs", "impl InputIngress { pub fn try_submit(&self) {} }")
        write(
            hot / "crates/seyal-runtime/src/display.rs",
            "pub fn encode_snapshot() {} pub fn encode_delta() {} fn encode_rows() {}",
        )
        write(
            hot / "crates/seyal-runtime/src/runtime/local/display_publish.rs",
            "impl Runtime { pub(super) fn publish_display_updates(&mut self) {} }",
        )
        write(
            hot / "macos/Seyal/Sources/MetalTerminalRenderer.swift",
            "func update() {}\nfunc present() {}\n",
        )
        run_negative(["python3", str(ROOT / "scripts/check-hot-path.py")], hot, "avoidable allocation")

        benchmark = base / "benchmark-contract"
        write(benchmark / "crates/seyal-terminal/benches/bad.rs", 'fn main() { println!("performance_claim=true"); }\n')
        run_negative(["python3", str(ROOT / "scripts/check-benchmark-contract.py")], benchmark, "performance_claim=false")

        performance = base / "m002-performance-contract"
        performance.mkdir()
        run_negative(
            ["python3", str(ROOT / "scripts/check-m002-performance-contract.py")],
            performance,
            "missing M002 performance contract",
        )

        malformed_performance = base / "m002-performance-malformed"
        write(
            malformed_performance / "docs/evidence/M002-PERFORMANCE-CONTRACT-V1.md",
            "Status: proposed contract for Issue #673\nexact production SHA\nbaseline SHA\nnearest-rank\n",
        )
        write(malformed_performance / "docs/evidence/M002-PERFORMANCE-CONTRACT-V1.toml", "schema = 'wrong'\nversion = 1\n")
        run_negative(
            ["python3", str(ROOT / "scripts/check-m002-performance-contract.py")],
            malformed_performance,
            "unsupported identity",
        )

        invalid_result = base / "m002-performance-invalid-result"
        write(
            invalid_result / "docs/evidence/M002-PERFORMANCE-CONTRACT-V1.md",
            "Status: proposed contract for Issue #673\nexact production SHA\nbaseline SHA\nnearest-rank\n",
        )
        shutil.copy(ROOT / "docs/evidence/M002-PERFORMANCE-CONTRACT-V1.toml", invalid_result / "docs/evidence/M002-PERFORMANCE-CONTRACT-V1.toml")
        write(
            invalid_result / "record.toml",
            "contract_schema = 'seyal.m002.performance-contract'\ncontract_version = 1\n",
        )
        run_negative(
            ["python3", str(ROOT / "scripts/check-m002-performance-contract.py"), "--record", "record.toml"],
            invalid_result,
            "performance result missing",
        )

        invalid_percentiles = base / "m002-performance-invalid-percentiles"
        write(
            invalid_percentiles / "docs/evidence/M002-PERFORMANCE-CONTRACT-V1.md",
            "Status: proposed contract for Issue #673\nexact production SHA\nbaseline SHA\nnearest-rank\n",
        )
        shutil.copy(ROOT / "docs/evidence/M002-PERFORMANCE-CONTRACT-V1.toml", invalid_percentiles / "docs/evidence/M002-PERFORMANCE-CONTRACT-V1.toml")
        write(
            invalid_percentiles / "record.toml",
            "contract_schema = 'seyal.m002.performance-contract'\ncontract_version = 1\nproduction_sha = '1111111111111111111111111111111111111111'\nharness_sha = '2222222222222222222222222222222222222222'\nbaseline_sha = '3333333333333333333333333333333333333333'\nbuild_mode = 'release'\nos_version = 'macOS'\ntoolchain = 'Xcode/Rust'\nhardware = 'arm64'\ndisplay = 'display'\npower_thermal_state = 'nominal'\nworkload_hash = 'hash'\ntopology = 'one'\nevidence_class = 'PHYSICAL_ARM64'\ngate = 'history_active_reflow_ms'\nmetric = 'history_active_reflow_ms'\nboundary = 'HistoryStore active reflow'\nunit = 'ms'\npercentile_method = 'nearest-rank'\nsample_count = 500\ncohort_count = 5\nenvironment_status = 'VALID'\nplatform_limit_reason = ''\ncomparator = 'less_equal'\np50 = 3\np95 = 2\np99 = 4\nbaseline_p50 = 2\nbaseline_p95 = 4\nbaseline_p99 = 8\nrelative_regression_percent = 10\nraw_log = 'raw.log'\nraw_cohorts = 'cohorts/'\n",
        )
        run_negative(
            ["python3", str(ROOT / "scripts/check-m002-performance-contract.py"), "--record", "record.toml"],
            invalid_percentiles,
            "percentiles must be ordered",
        )

        unicode_benchmark = base / "unicode-benchmark-contract"
        write(
            unicode_benchmark / "crates/seyal-terminal/benches/good.rs",
            'use std::time::Instant; fn main() { let _ = Instant::now(); println!("performance_claim=false"); }\n',
        )
        write(
            unicode_benchmark / "macos/Seyal/Sources/RendererValidation.swift",
            'print("m002_unicode_renderer performance_claim=false")\n',
        )
        run_negative(
            ["python3", str(ROOT / "scripts/check-benchmark-contract.py")],
            unicode_benchmark,
            "baseline_unicode_pipeline=UNSUPPORTED_NONCOMPARABLE",
        )

        ui_policy = base / "ui-test-policy"
        write(ui_policy / "macos/Seyal/Tests/SeyalTests/SeyalShellComponentTests.swift", "// fixture\n")
        write(ui_policy / "macos/Seyal/Tests/SeyalUITests/SeyalShellUITests.swift", "// fixture\n")
        write(ui_policy / "macos/Seyal/Seyal.xcodeproj/xcshareddata/xcschemes/Seyal.xcscheme", "SeyalTests.xctest SeyalUITests.xctest\n")
        write(ui_policy / "macos/Seyal/Seyal.xcodeproj/project.pbxproj", "SeyalTests\n")
        write(ui_policy / "scripts/test-macos-ui.sh", "#!/usr/bin/env bash\n")
        run_negative(["python3", str(ROOT / "scripts/check-ui-test-policy.py")], ui_policy, "Xcode project is missing SeyalUITests")

        workspace = base / "workspace"
        workspace.mkdir()
        run_negative(["python3", str(ROOT / "scripts/test-workspace.py")], workspace, "missing root Cargo.toml")

        harness = base / "harness"
        harness.mkdir()
        run_negative(["python3", str(ROOT / "scripts/test-harnesses.py")], harness, "missing integration-test harness location")

    print("[seyal CI validator self-test] controlled negative fixtures were rejected by every repository validator.")


if __name__ == "__main__":
    main()
