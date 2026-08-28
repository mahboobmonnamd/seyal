# M001 Pass 7 native input and resize evidence

This record captures the reproducible automated Pass 7 benchmark run for
benchmark source head `149a8f2` (consolidated with the Blocks UI) on 2026-08-28;
the evidence record is committed in the following documentation commit.
It is implementation evidence, not a release-readiness claim.

## Environment and commands

```text
host: Apple M5 Pro / arm64
OS: macOS 26.5.2 (25F84)
Rust: rustc 1.98.0 (88d9e12ae 2026-08-18)
build: Release
percentiles: nearest-rank
representative repetitions: 120
commands: make bench; cargo test -p seyal-runtime --features test-fault-injection --test pass7_local_ipc persistent_winsize_failure_is_bounded_before_canonical_commit -- --exact --nocapture; cargo test -p seyal-client --test pass7_interactive --locked
```

All benchmark records use `performance_claim=false`. Input text, composition
text, control bytes and terminal contents are not emitted by benchmark records.

## Machine-readable exact-head records

```text
pass7_host macos_version=26.5.2 macos_build=25F84 model="Mac17,9" hardware="Apple M5 Pro" arch=aarch64 rust="rustc 1.98.0 (88d9e12ae 2026-08-18)" build_mode=release commit=149a8f205848493a4f4d63e1f47005f6987bcd7a repetitions=120 percentile_method=nearest_rank cpu_rss_sources=ps_and_usr_bin_time performance_claim=false
pass7_latency boundary=controlled_native_callback_to_client_admission classification=MEASURED sample_count=120 p50_us=0.125 p95_us=0.209 p99_us=0.375 max_us=2.625 performance_claim=false
pass7_latency boundary=client_admission_to_socket_complete classification=MEASURED sample_count=120 p50_us=1.167 p95_us=2.334 p99_us=3.666 max_us=7.000 performance_claim=false
pass7_latency boundary=runtime_frame_admission_to_pty_write classification=MEASURED sample_count=120 p50_us=1.958 p95_us=2.583 p99_us=5.833 max_us=7.083 performance_claim=false
pass7_latency boundary=controlled_native_callback_to_pty_write classification=MEASURED sample_count=120 p50_us=6.584 p95_us=11.500 p99_us=13.958 max_us=14.292 performance_claim=false
pass7_input_resources classification=MEASURED measurement_phase=post_input_workload client_queue_high_water_bytes=45 runtime_queue_high_water_bytes=1 rss_baseline_kib=1760 rss_populated_kib=2928 rss_measured_kib=3200 incremental_post_workload_rss_kib=1440 cpu_percent_sample=0 threads_baseline=1 threads_populated=2 threads_measured=2 fds_baseline=4 fds_populated=10 fds_measured=10 native_boundary_classification=CONTROLLED_FFI_EQUIVALENT_APPKIT_EVENT_NOT_CLAIMED performance_claim=false
pass7_latency boundary=resize_120x40 classification=MEASURED sample_count=120 p50_us=7.792 p95_us=10.166 p99_us=13.250 max_us=17.625 performance_claim=false
pass7_resize_resources case=resize_120x40 geometry=120x40 classification=MEASURED measurement_phase=post_resize_workload client_queue_high_water_bytes=56 runtime_queue_high_water_bytes=0 rss_baseline_kib=1760 rss_populated_kib=3536 rss_measured_kib=3920 incremental_post_resize_rss_kib=2160 cpu_percent_sample=0 performance_claim=false
pass7_latency boundary=resize_512x256 classification=MEASURED sample_count=120 p50_us=97.541 p95_us=124.875 p99_us=136.750 max_us=144.084 performance_claim=false
pass7_resize_resources case=resize_512x256 geometry=512x256 classification=MEASURED measurement_phase=post_resize_workload client_queue_high_water_bytes=56 runtime_queue_high_water_bytes=0 rss_baseline_kib=1760 rss_populated_kib=13456 rss_measured_kib=20944 incremental_post_resize_rss_kib=19184 cpu_percent_sample=142.2 performance_claim=false
pass7_idle_resource classification=MEASURED idle_window_ms=500 rss_baseline_kib=1760 rss_populated_kib=2928 rss_idle_kib=2976 incremental_idle_rss_kib=1216 cpu_percent_sample=0.4 threads_baseline=1 threads_idle=2 fds_baseline=4 fds_idle=10 client_wants_write=false performance_claim=false
```

The `post_input_workload` and `post_resize_workload` deltas are measured RSS
minus the same-process baseline after the workload. They are not idle deltas.
The separate `idle_resource` record is the only idle measurement.

## Validation matrix and failure evidence

The exact-head `make bench` run measured the required commit sizes (1 B, 16 KiB,
64 KiB), atomic 65,537 B rejection, 64-key repeat bursts, input under sustained
output, and alternate-screen input/resize. Each case emitted 120 samples,
completed at the PTY, and passed `scripts/check-pass7-validation-matrix.py`.

The deterministic failure test injected three consecutive PTY winsize failures
at the endpoint boundary. It observed three correlated `InternalFailure` results,
zero applied generations, unchanged 80x24 canonical geometry, and no generation
advance. A fourth request after the fault epoch cleared applied 120x40 and
advanced exactly one generation. This proves the automated bounded-failure
contract; it does not replace physical AppKit acceptance.

```text
pass7_matrix_remaining validation=physical_keyboard_hardware_and_independent_review classification=NOT_CLAIMED performance_claim=false
pass7_failure case=persistent_runtime_resize_failure attempts=3 result_per_attempt=1 canonical_geometry_unchanged=true generation_unchanged=true retry_loop=false explicit_recovery_request=true classification=MEASURED performance_claim=false
pass7_native_ui_automation cases=return_submit,focus_recovery,dead_key_commit,ime_cancel classification=HEADED_NATIVE_AUTOMATION app=dev.seyal.Seyal commit=149a8f205848493a4f4d63e1f47005f6987bcd7a physical_keyboard_hardware=false terminal_contents_recorded=false performance_claim=false
```

## Pass 6 comparison

The same `make bench` run recorded the Pass 6 native renderer comparator at
120x40, 120 repetitions: preparation p50/p95/p99/max `917/185209/484417/746208`
ns; prepared-to-command-commit `24875/57000/139125/692291` ns; and
command-commit-to-GPU-completion proxy `760583/1048208/1093875/3418209` ns.
Compared with the prior recorded candidate (`750/240167/928792/1553875`,
`32167/52625/195958/542417`, and `854000/1086250/1670833/5126250` ns), the
current p99 values are lower on all three boundaries. This is a measured
comparison, not a claim that controlled input/resize proves physical scanout.

## Explicit limits and remaining acceptance work

The native input benchmark boundary remains a controlled synthetic
`NSEvent`/FFI equivalent. Direct headed automation additionally exercised Return,
focus recovery, dead-key commit and IME cancellation on the exact head, but it is
not physical human keyboard hardware evidence. Physical keyboard hardware,
overlapping-resize/manual acceptance and independent security/implementation
review remain required by SPEC-006 before Pass 7 or PR #707 can be considered
ready.
