# M001 Pass 7 native input and resize evidence

This record captures the reproducible automated Pass 7 benchmark run for
benchmark source head `99df57b` (rebased onto current `master`) on 2026-08-28;
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
commands: make bench; cargo test -p seyal-runtime --features test-fault-injection --test pass7_local_ipc persistent_winsize_failure_is_bounded_before_canonical_commit -- --exact --nocapture
```

All benchmark records use `performance_claim=false`. Input text, composition
text, control bytes and terminal contents are not emitted by benchmark records.

## Machine-readable exact-head records

```text
pass7_host macos_version=26.5.2 macos_build=25F84 model="Mac17,9" hardware="Apple M5 Pro" arch=aarch64 rust="rustc 1.98.0 (88d9e12ae 2026-08-18)" build_mode=release commit=99df57bc577b349a182114b583cee279a8587594 repetitions=120 percentile_method=nearest_rank cpu_rss_sources=ps_and_usr_bin_time performance_claim=false
pass7_latency boundary=controlled_native_callback_to_client_admission classification=MEASURED sample_count=120 p50_us=0.083 p95_us=0.167 p99_us=1.959 max_us=2.417 performance_claim=false
pass7_latency boundary=client_admission_to_socket_complete classification=MEASURED sample_count=120 p50_us=0.917 p95_us=2.125 p99_us=2.666 max_us=3.250 performance_claim=false
pass7_latency boundary=runtime_frame_admission_to_pty_write classification=MEASURED sample_count=120 p50_us=1.834 p95_us=2.541 p99_us=4.375 max_us=6.542 performance_claim=false
pass7_latency boundary=controlled_native_callback_to_pty_write classification=MEASURED sample_count=120 p50_us=6.083 p95_us=9.500 p99_us=12.041 max_us=14.459 performance_claim=false
pass7_input_resources classification=MEASURED measurement_phase=post_input_workload client_queue_high_water_bytes=45 runtime_queue_high_water_bytes=1 rss_baseline_kib=1744 rss_populated_kib=2912 rss_measured_kib=3168 incremental_post_workload_rss_kib=1424 cpu_percent_sample=0 threads_baseline=1 threads_populated=2 threads_measured=2 fds_baseline=4 fds_populated=10 fds_measured=10 native_boundary_classification=CONTROLLED_FFI_EQUIVALENT_APPKIT_EVENT_NOT_CLAIMED performance_claim=false
pass7_latency boundary=resize_120x40 classification=MEASURED sample_count=120 p50_us=10.625 p95_us=13.041 p99_us=14.833 max_us=15.417 performance_claim=false
pass7_resize_resources case=resize_120x40 geometry=120x40 classification=MEASURED measurement_phase=post_resize_workload client_queue_high_water_bytes=56 runtime_queue_high_water_bytes=0 rss_baseline_kib=1744 rss_populated_kib=3504 rss_measured_kib=3968 incremental_post_resize_rss_kib=2224 cpu_percent_sample=17 performance_claim=false
pass7_latency boundary=resize_512x256 classification=MEASURED sample_count=120 p50_us=94.375 p95_us=121.417 p99_us=131.791 max_us=153.792 performance_claim=false
pass7_resize_resources case=resize_512x256 geometry=512x256 classification=MEASURED measurement_phase=post_resize_workload client_queue_high_water_bytes=56 runtime_queue_high_water_bytes=0 rss_baseline_kib=1744 rss_populated_kib=13472 rss_measured_kib=19680 incremental_post_resize_rss_kib=17936 cpu_percent_sample=147.4 performance_claim=false
pass7_idle_resource classification=MEASURED idle_window_ms=500 rss_baseline_kib=1744 rss_populated_kib=2960 rss_idle_kib=2976 incremental_idle_rss_kib=1232 cpu_percent_sample=0.1 threads_baseline=1 threads_idle=2 fds_baseline=4 fds_idle=10 client_wants_write=false performance_claim=false
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
pass7_matrix_remaining validation=true_AppKit_event_boundary_and_physical_keyboard_IME_focus classification=NOT_CLAIMED performance_claim=false
pass7_failure case=persistent_runtime_resize_failure attempts=3 result_per_attempt=1 canonical_geometry_unchanged=true generation_unchanged=true retry_loop=false explicit_recovery_request=true classification=MEASURED performance_claim=false
```

## Pass 6 comparison

The same `make bench` run recorded the Pass 6 native renderer comparator at
120x40, 120 repetitions: preparation p50/p95/p99/max `750/240167/928792/1553875`
ns; prepared-to-command-commit `32167/52625/195958/542417` ns; and
command-commit-to-GPU-completion proxy `854000/1086250/1670833/5126250` ns.
These are retained as a separate renderer boundary; Pass 7 does not claim that
controlled input/resize measurements prove physical presentation performance.

## Explicit limits and remaining acceptance work

The native input boundary is a controlled synthetic `NSEvent`/FFI equivalent and
is labelled `CONTROLLED_FFI_EQUIVALENT_APPKIT_EVENT_NOT_CLAIMED`. Physical
keyboard, dead-key/IME commit and cancel, AppKit focus transitions, overlapping
resize, and alternate-screen interaction still require the SPEC-006 section
16.4 manual acceptance on the exact final head. Independent security/implementation
review and validation against the current `master` merge result are also required
before Pass 7 or PR #707 can be considered ready.
