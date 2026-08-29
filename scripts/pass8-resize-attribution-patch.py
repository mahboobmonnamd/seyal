#!/usr/bin/env python3
from pathlib import Path

local_path = Path("crates/seyal-client/src/local.rs")
text = local_path.read_text()
anchor = '''    fn finish_attach(\n        mut stream: UnixStream,\n'''
insert = '''    /// Benchmark-only control connection that preserves the exact Pass 7\n    /// interactive path while deliberately omitting Pass 8 metadata negotiation.\n    /// This exists solely to attribute same-head latency movement; production\n    /// callers always use `connect_execution`, which requests Pass 8 normally.\n    #[cfg(feature = "benchmark-instrumentation")]\n    pub fn connect_execution_without_block_metadata(\n        socket_path: &Path,\n        execution_id: ExecutionId,\n        role: Role,\n    ) -> Result<Self, ClientError> {\n        let mut stream = connect_stream(socket_path)?;\n        let server_hello = hello(&mut stream, role == Role::Controller, false)?;\n        Self::finish_attach(\n            stream,\n            execution_id,\n            role,\n            server_hello.server_capabilities & CAP_COMMAND_BLOCKS != 0,\n            server_hello.runtime_id,\n            false,\n        )\n    }\n\n'''
if text.count(anchor) != 1:
    raise SystemExit(f"finish_attach anchor count={text.count(anchor)}")
local_path.write_text(text.replace(anchor, insert + anchor, 1))

bench_path = Path("crates/seyal-client/benches/pass7_input_resize.rs")
text = bench_path.read_text()
old = '''    for case in ["input", "resize_120x40", "resize_512x256", "idle_resource"] {\n'''
new = '''    for case in [\n        "input",\n        "resize_120x40",\n        "resize_512x256",\n        "idle_resource",\n        "pass8_resize_attribution",\n    ] {\n'''
if text.count(old) != 1:
    raise SystemExit(f"worker list anchor count={text.count(old)}")
text = text.replace(old, new, 1)

old = '''        "idle_resource" => measure_idle_resources(),\n        other => panic!("unknown Pass 7 benchmark worker {other}"),\n'''
new = '''        "idle_resource" => measure_idle_resources(),\n        "pass8_resize_attribution" => measure_pass8_resize_attribution(),\n        other => panic!("unknown Pass 7 benchmark worker {other}"),\n'''
if text.count(old) != 1:
    raise SystemExit(f"worker match anchor count={text.count(old)}")
text = text.replace(old, new, 1)

old = '''    fn connect_controller(&self) -> LocalDisplayClient {\n        LocalDisplayClient::connect_execution(\n            &self.socket_path,\n            self.execution_id,\n            Role::Controller,\n        )\n        .expect("controller attach")\n    }\n\n'''
new = old + '''    fn connect_controller_without_block_metadata(&self) -> LocalDisplayClient {\n        LocalDisplayClient::connect_execution_without_block_metadata(\n            &self.socket_path,\n            self.execution_id,\n            Role::Controller,\n        )\n        .expect("controller attach without Pass 8 metadata")\n    }\n\n'''
if text.count(old) != 1:
    raise SystemExit(f"connect_controller anchor count={text.count(old)}")
text = text.replace(old, new, 1)

anchor = '''#[cfg(target_os = "macos")]\nfn run_resize_sample(\n'''
insert = '''#[cfg(target_os = "macos")]\nfn measure_pass8_resize_attribution() {\n    let target = GridGeometry {\n        rows: 40,\n        columns: 120,\n    };\n    let reset = GridGeometry {\n        rows: 40,\n        columns: 121,\n    };\n    let mut disabled = Vec::with_capacity(3);\n    let mut enabled = Vec::with_capacity(3);\n\n    // Alternate modes so host drift cannot systematically favor one side.\n    for block_metadata_enabled in [false, true, true, false, false, true] {\n        let p99 = collect_resize_attribution_p99(block_metadata_enabled, target, reset);\n        if block_metadata_enabled {\n            enabled.push(p99);\n        } else {\n            disabled.push(p99);\n        }\n    }\n    disabled.sort_by(f64::total_cmp);\n    enabled.sort_by(f64::total_cmp);\n    let disabled_median = disabled[1];\n    let enabled_median = enabled[1];\n    let delta_percent = if disabled_median > 0.0 {\n        ((enabled_median / disabled_median) - 1.0) * 100.0\n    } else {\n        0.0\n    };\n    println!(\n        "pass8_attribution boundary=resize_120x40 classification=MEASURED method=same_head_alternating_3x120 pass8_disabled_p99_median_us={:.3} pass8_enabled_p99_median_us={:.3} delta_percent={:.2} {}",\n        disabled_median,\n        enabled_median,\n        delta_percent,\n        PERFORMANCE_CLAIM,\n    );\n}\n\n#[cfg(target_os = "macos")]\nfn collect_resize_attribution_p99(\n    block_metadata_enabled: bool,\n    target: GridGeometry,\n    reset: GridGeometry,\n) -> f64 {\n    let runtime = RuntimeHarness::start();\n    let mut client = if block_metadata_enabled {\n        runtime.connect_controller()\n    } else {\n        runtime.connect_controller_without_block_metadata()\n    };\n    converge_geometry(&mut client, reset);\n    for _ in 0..8 {\n        run_resize_sample(&mut client, target, reset, false, None);\n    }\n\n    let mut samples = Samples::with_capacity(REPETITIONS);\n    let mut client_queue_high_water = 0usize;\n    let mut runtime_queue_high_water = 0usize;\n    for _ in 0..REPETITIONS {\n        run_resize_sample(\n            &mut client,\n            target,\n            reset,\n            true,\n            Some((\n                &mut samples,\n                &mut client_queue_high_water,\n                &mut runtime_queue_high_water,\n            )),\n        );\n    }\n    let p99 = samples.stats_us().p99_us;\n    drop(client);\n    runtime.finish();\n    p99\n}\n\n'''
if text.count(anchor) != 1:
    raise SystemExit(f"run_resize_sample anchor count={text.count(anchor)}")
bench_path.write_text(text.replace(anchor, insert + anchor, 1))
