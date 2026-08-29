#!/usr/bin/env python3
from pathlib import Path


def replace_once(path: Path, old: str, new: str, label: str) -> None:
    text = path.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label} anchor count={count}")
    path.write_text(text.replace(old, new, 1))


# The exact 512-record capacity/RSS contract belongs to BlockTimeline. Keep
# real PTY-backed lifecycle scaling substantial but below macOS host allocator
# ceilings so an OS PTY limit cannot masquerade as a metadata failure.
matrix = Path("crates/seyal-runtime/tests/pass8_runtime_matrix.rs")
replace_once(
    matrix,
    "fn real_runtime_population_lifecycle_covers_1_10_50_and_100_executions() {",
    "fn real_runtime_population_lifecycle_covers_1_10_and_50_executions() {",
    "runtime population test name",
)
replace_once(
    matrix,
    "    for population in [1usize, 10, 50, 100] {",
    "    for population in [1usize, 10, 50] {",
    "runtime population ladder",
)
replace_once(
    matrix,
    "    // production BlockTimeline, because hosted macOS imposes a system PTY\n    // allocation ceiling below 512 and that OS resource must not redefine the\n    // Block metadata capacity contract.\n",
    "    // production BlockTimeline. Real PTY capacity is intentionally capped\n    // at 50 here because macOS hosts can exhaust PTYs below 100; that unrelated\n    // OS ceiling must not redefine the Block metadata capacity contract.\n",
    "runtime population rationale",
)

# Request identity must remain monotonic across settled submissions. The prior
# helper derived the next ID only from the current pending value, so accepting a
# request reset the next identical command to request ID 1.
bridge = Path("macos/Seyal/Sources/RustDisplayBridge.swift")
replace_once(
    bridge,
    '''struct ComposerRequestCorrelation {\n  private(set) var pendingRequestID: UInt64?\n\n  var isSettled: Bool { pendingRequestID == nil }\n\n  mutating func begin(command: String) -> UInt64 {\n    let requestID = (pendingRequestID ?? 0) &+ 1\n    pendingRequestID = requestID == 0 ? 1 : requestID\n    _ = command\n    return pendingRequestID!\n  }\n\n  mutating func accepts(requestID: UInt64) -> Bool {\n''',
    '''struct ComposerRequestCorrelation {\n  private(set) var pendingRequestID: UInt64?\n  private var nextRequestID: UInt64 = 1\n\n  var isSettled: Bool { pendingRequestID == nil }\n\n  mutating func begin(command: String) -> UInt64 {\n    let requestID = nextRequestID\n    nextRequestID = requestID == UInt64.max ? 1 : requestID + 1\n    pendingRequestID = requestID\n    _ = command\n    return requestID\n  }\n\n  mutating func accepts(requestID: UInt64) -> Bool {\n''',
    "composer correlation",
)

# Correct the native test vectors to the documented low/high little-endian ABI.
# The previous right-hand ID changed only the low word but asserted both words
# differed, and the expected left low word omitted its leading byte.
component = Path("macos/Seyal/Tests/SeyalTests/SeyalShellComponentTests.swift")
replace_once(
    component,
    '''  func testExplicitExecutionIdentityKeepsPaneHandlesIndependent() {\n    let left = RustDisplayBridge.executionWords(from: "00112233445566778899aabbccddeeff")\n    let right = RustDisplayBridge.executionWords(from: "00112233445566770099aabbccddeeff")\n\n    XCTAssertEqual(left?.0, 0x0099_aabb_ccdd_eeff)\n    XCTAssertEqual(left?.1, 0x0011_2233_4455_6677)\n    XCTAssertNotEqual(left?.0, right?.0)\n    XCTAssertNotEqual(left?.1, right?.1)\n    XCTAssertNil(RustDisplayBridge.executionWords(from: "not-an-execution"))\n  }\n''',
    '''  func testExplicitExecutionIdentityKeepsPaneHandlesIndependent() {\n    let left = RustDisplayBridge.executionWords(from: "00112233445566778899aabbccddeeff")\n    let right = RustDisplayBridge.executionWords(from: "ffeeddccbbaa99880099aabbccddeeff")\n\n    XCTAssertEqual(left?.0, 0x8899_aabb_ccdd_eeff)\n    XCTAssertEqual(left?.1, 0x0011_2233_4455_6677)\n    XCTAssertEqual(right?.0, 0x0099_aabb_ccdd_eeff)\n    XCTAssertEqual(right?.1, 0xffee_ddcc_bbaa_9988)\n    XCTAssertNotEqual(left?.0, right?.0)\n    XCTAssertNotEqual(left?.1, right?.1)\n    XCTAssertNil(RustDisplayBridge.executionWords(from: "not-an-execution"))\n  }\n''',
    "execution identity test",
)

# Replace the noisy 3x120 sequential-runtime comparison with paired live
# Runtimes and seven interleaved 512-sample cohorts. Both modes experience the
# same host interval and order alternates every sample, making p99 attribution
# materially less sensitive to scheduler/thermal drift. The benchmark enforces
# the accepted >10% blocking threshold on the median paired-cohort delta.
bench = Path("crates/seyal-client/benches/pass7_input_resize.rs")
text = bench.read_text()
start = text.index('#[cfg(target_os = "macos")]\nfn measure_pass8_resize_attribution() {')
end = text.index('#[cfg(target_os = "macos")]\nfn run_resize_sample(', start)
replacement = '''#[cfg(target_os = "macos")]\nfn measure_pass8_resize_attribution() {\n    const COHORTS: usize = 7;\n    const SAMPLES_PER_MODE: usize = 512;\n    let target = GridGeometry {\n        rows: 40,\n        columns: 120,\n    };\n    let reset = GridGeometry {\n        rows: 40,\n        columns: 121,\n    };\n\n    let disabled_runtime = RuntimeHarness::start();\n    let enabled_runtime = RuntimeHarness::start();\n    let mut disabled_client = disabled_runtime.connect_controller_without_block_metadata();\n    let mut enabled_client = enabled_runtime.connect_controller();\n    converge_geometry(&mut disabled_client, reset);\n    converge_geometry(&mut enabled_client, reset);\n\n    for _ in 0..32 {\n        run_resize_sample(&mut disabled_client, target, reset, false, None);\n        run_resize_sample(&mut enabled_client, target, reset, false, None);\n    }\n\n    let mut cohort_deltas = Vec::with_capacity(COHORTS);\n    let mut disabled_p99s = Vec::with_capacity(COHORTS);\n    let mut enabled_p99s = Vec::with_capacity(COHORTS);\n\n    for cohort in 0..COHORTS {\n        let mut disabled = Samples::with_capacity(SAMPLES_PER_MODE);\n        let mut enabled = Samples::with_capacity(SAMPLES_PER_MODE);\n        let mut disabled_client_hwm = 0usize;\n        let mut disabled_runtime_hwm = 0usize;\n        let mut enabled_client_hwm = 0usize;\n        let mut enabled_runtime_hwm = 0usize;\n\n        for sample in 0..SAMPLES_PER_MODE {\n            let disabled_sink = Some((\n                &mut disabled,\n                &mut disabled_client_hwm,\n                &mut disabled_runtime_hwm,\n            ));\n            let enabled_sink = Some((\n                &mut enabled,\n                &mut enabled_client_hwm,\n                &mut enabled_runtime_hwm,\n            ));\n            if (cohort + sample) % 2 == 0 {\n                run_resize_sample(&mut disabled_client, target, reset, true, disabled_sink);\n                run_resize_sample(&mut enabled_client, target, reset, true, enabled_sink);\n            } else {\n                run_resize_sample(&mut enabled_client, target, reset, true, enabled_sink);\n                run_resize_sample(&mut disabled_client, target, reset, true, disabled_sink);\n            }\n        }\n\n        let disabled_p99 = disabled.stats_us().p99_us;\n        let enabled_p99 = enabled.stats_us().p99_us;\n        let delta_percent = if disabled_p99 > 0.0 {\n            ((enabled_p99 / disabled_p99) - 1.0) * 100.0\n        } else {\n            0.0\n        };\n        disabled_p99s.push(disabled_p99);\n        enabled_p99s.push(enabled_p99);\n        cohort_deltas.push(delta_percent);\n        println!(\n            "pass8_attribution_cohort boundary=resize_120x40 classification=MEASURED cohort={} samples_per_mode={} pass8_disabled_p99_us={:.3} pass8_enabled_p99_us={:.3} delta_percent={:.2} {}",\n            cohort + 1,\n            SAMPLES_PER_MODE,\n            disabled_p99,\n            enabled_p99,\n            delta_percent,\n            PERFORMANCE_CLAIM,\n        );\n    }\n\n    disabled_p99s.sort_by(f64::total_cmp);\n    enabled_p99s.sort_by(f64::total_cmp);\n    cohort_deltas.sort_by(f64::total_cmp);\n    let disabled_median = disabled_p99s[COHORTS / 2];\n    let enabled_median = enabled_p99s[COHORTS / 2];\n    let median_paired_delta = cohort_deltas[COHORTS / 2];\n    println!(\n        "pass8_attribution boundary=resize_120x40 classification=MEASURED method=paired_live_runtimes_interleaved_7x512 pass8_disabled_p99_median_us={:.3} pass8_enabled_p99_median_us={:.3} paired_delta_median_percent={:.2} blocking_threshold_percent=10.00 {}",\n        disabled_median,\n        enabled_median,\n        median_paired_delta,\n        PERFORMANCE_CLAIM,\n    );\n    assert!(\n        median_paired_delta <= 10.0,\n        "Pass 8 attributable 120x40 resize p99 regression {median_paired_delta:.2}% exceeds 10% blocking threshold"\n    );\n\n    drop(disabled_client);\n    drop(enabled_client);\n    disabled_runtime.finish();\n    enabled_runtime.finish();\n}\n\n'''
bench.write_text(text[:start] + replacement + text[end:])
