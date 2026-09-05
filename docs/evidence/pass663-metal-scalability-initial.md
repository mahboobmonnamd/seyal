# Pass 663 — Metal multi-surface scalability (initial harness)

**Issue:** [#663](https://github.com/mahboobmonnamd/seyal/issues/663)  
**Closing keyword:** **Refs only** — acceptance criteria not fully met (no M003 5×10×5 product topology; short soak; display-link proxy not yet matrixed).  
**Evidence class:** `controlled-host`  
**Harness:** `Seyal.app --pass663-metal-scalability` (`Pass663MetalScalability.swift`)

## Command

```bash
SEYAL_MACOS_CONFIGURATION=Release SEYAL_CODESIGN_IDENTITY=- bash scripts/build-macos.sh
export SEYAL_PASS663_MATRIX=1,5,25,50,125,250
export SEYAL_PASS663_VISIBLE=1,5,25
export SEYAL_PASS663_SOAK_SECONDS=5   # raise to 1800 for full plateau soak
export SEYAL_PASS663_COMMIT=$(git rev-parse HEAD)
./target/macos-derived-data/Build/Products/Release/Seyal.app/Contents/MacOS/Seyal \
  --pass663-metal-scalability | tee docs/evidence/pass663-metal-scalability-initial.log
```

## What this measures (honest)

1. **synthetic** — N independent `MetalTerminalRenderer` instances on prepared frames (Metal resource curve through 250).
2. **real_path_fanout** — one bundled Runtime / Candidate-D attachment fans into N renderers (presentation scaling on the real path).
3. **distinct_pty_per_pane** for N>25 — labelled `PLATFORM_LIMITED` (Pass 5.1 host PTY ceiling ~27–34); not silently reduced.

Product 5 workspace × 10 tab × 5 pane chrome remains **M003 (#674)** and is out of scope for this harness.

## Initial run highlights (M5 Pro, arm64, soak=5s)

From `pass663-metal-scalability-initial.log`:

| panes | visible | rss_kib | dedicated_gpu_bytes | atlas_duplicated |
|------:|--------:|--------:|--------------------:|:-----------------|
| 1 | 1 | 37104 | 16869376 | false |
| 25 | 1 | 177456 | 16869376 | false |
| 25 | 25 | 715217 | 421734400 | true |
| 250 | 1 | 1644881 | 16869376 | false |
| 250 | 25 | 2191586 | 421734400 | true |

**Finding:** With only one visible surface, dedicated GPU / atlas stays at a single ~16 MiB atlas even at 250 renderer instances — hidden surfaces release surface-local atlas residency as designed. When 25 surfaces are visible, atlas residency scales ~25× (`atlas_duplicated=true`), confirming today’s per-renderer atlas/queue ownership.

`real_path_fanout` timed out on bundled Runtime attach in this first run (`PLATFORM_LIMITED`); retry with a longer attach budget is next. Distinct-PTY-per-pane N>25 remains host-ceiling `PLATFORM_LIMITED`.


## Remaining to close #663

1. Controlled-host soak ≥ documented plateau (Issue: ~30 minutes or equivalent).
2. Headed display-link / visible-pane latency for 1/5/25 visible.
3. One-noisy vs many-noisy CPU/tail latency.
4. Explicit judgment vs budgets; fix any material unbounded growth with regression coverage.
5. Either land M003 topology for honest 5×10×5 product exercise, or refine #663 acceptance to accept harness topology with explicit non-goals.

## Raw harness output (excerpt)

```text
pass663_metal_scalability schema=seyal.pass663.metal-scalability.v1 performance_claim=false
commit=55eb5f77a5740133745388ce19d6c6f382ae56ae device=Apple M5 Pro registry_id=4294968668 os=Version 26.5.2 (Build 25F84) arch=arm64 geometry=80x24 soak_seconds=5 percentile_method=nearest_rank evidence_class=controlled-host
note=product_5x10x5_chrome_is_M003; this harness measures Metal presentation scaling for N logical surfaces
row panes=1 visible=1 cohort=synthetic status=MEASURED rss_kib=37104 dedicated_gpu_bytes=16869376 instance_bytes=92160 atlas_resident_bytes=16777216 atlas_duplicated=false renderers=1 display_links=0 command_queues=1 prep_p50_ns=328583 prep_p95_ns=378792 prep_p99_ns=378792 note=hide_show_cycles=225; per_renderer_atlas_and_queue=true; presentation_proxy=offscreen_only
row panes=5 visible=1 cohort=synthetic status=MEASURED rss_kib=58896 dedicated_gpu_bytes=16869376 instance_bytes=92160 atlas_resident_bytes=16777216 atlas_duplicated=false renderers=5 display_links=0 command_queues=5 prep_p50_ns=418333 prep_p95_ns=568041 prep_p99_ns=568041 note=hide_show_cycles=221; per_renderer_atlas_and_queue=true; presentation_proxy=offscreen_only
row panes=5 visible=5 cohort=synthetic status=MEASURED rss_kib=167552 dedicated_gpu_bytes=84346880 instance_bytes=460800 atlas_resident_bytes=83886080 atlas_duplicated=true renderers=5 display_links=0 command_queues=5 prep_p50_ns=548250 prep_p95_ns=643083 prep_p99_ns=656417 note=hide_show_cycles=196; per_renderer_atlas_and_queue=true; presentation_proxy=offscreen_only
row panes=25 visible=1 cohort=synthetic status=MEASURED rss_kib=177456 dedicated_gpu_bytes=16869376 instance_bytes=92160 atlas_resident_bytes=16777216 atlas_duplicated=false renderers=25 display_links=0 command_queues=25 prep_p50_ns=505500 prep_p95_ns=554459 prep_p99_ns=554459 note=hide_show_cycles=220; per_renderer_atlas_and_queue=true; presentation_proxy=offscreen_only
row panes=25 visible=5 cohort=synthetic status=MEASURED rss_kib=283696 dedicated_gpu_bytes=84346880 instance_bytes=460800 atlas_resident_bytes=83886080 atlas_duplicated=true renderers=25 display_links=0 command_queues=25 prep_p50_ns=514625 prep_p95_ns=557083 prep_p99_ns=578083 note=hide_show_cycles=191; per_renderer_atlas_and_queue=true; presentation_proxy=offscreen_only
row panes=25 visible=25 cohort=synthetic status=MEASURED rss_kib=715217 dedicated_gpu_bytes=421734400 instance_bytes=2304000 atlas_resident_bytes=419430400 atlas_duplicated=true renderers=25 display_links=0 command_queues=25 prep_p50_ns=359125 prep_p95_ns=458625 prep_p99_ns=489584 note=hide_show_cycles=150; per_renderer_atlas_and_queue=true; presentation_proxy=offscreen_only
row panes=50 visible=1 cohort=synthetic status=MEASURED rss_kib=661137 dedicated_gpu_bytes=16869376 instance_bytes=92160 atlas_resident_bytes=16777216 atlas_duplicated=false renderers=50 display_links=0 command_queues=50 prep_p50_ns=385167 prep_p95_ns=436291 prep_p99_ns=436291 note=hide_show_cycles=221; per_renderer_atlas_and_queue=true; presentation_proxy=offscreen_only
row panes=50 visible=5 cohort=synthetic status=MEASURED rss_kib=771713 dedicated_gpu_bytes=84346880 instance_bytes=460800 atlas_resident_bytes=83886080 atlas_duplicated=true renderers=50 display_links=0 command_queues=50 prep_p50_ns=437917 prep_p95_ns=480959 prep_p99_ns=510958 note=hide_show_cycles=199; per_renderer_atlas_and_queue=true; presentation_proxy=offscreen_only
row panes=50 visible=25 cohort=synthetic status=MEASURED rss_kib=1202593 dedicated_gpu_bytes=421734400 instance_bytes=2304000 atlas_resident_bytes=419430400 atlas_duplicated=true renderers=50 display_links=0 command_queues=50 prep_p50_ns=320041 prep_p95_ns=432042 prep_p99_ns=451625 note=hide_show_cycles=150; per_renderer_atlas_and_queue=true; presentation_proxy=offscreen_only
row panes=125 visible=1 cohort=synthetic status=MEASURED rss_kib=1150705 dedicated_gpu_bytes=16869376 instance_bytes=92160 atlas_resident_bytes=16777216 atlas_duplicated=false renderers=125 display_links=0 command_queues=125 prep_p50_ns=350084 prep_p95_ns=412875 prep_p99_ns=412875 note=hide_show_cycles=220; per_renderer_atlas_and_queue=true; presentation_proxy=offscreen_only
row panes=125 visible=5 cohort=synthetic status=MEASURED rss_kib=1259137 dedicated_gpu_bytes=84346880 instance_bytes=460800 atlas_resident_bytes=83886080 atlas_duplicated=true renderers=125 display_links=0 command_queues=125 prep_p50_ns=435334 prep_p95_ns=472041 prep_p99_ns=514708 note=hide_show_cycles=194; per_renderer_atlas_and_queue=true; presentation_proxy=offscreen_only
row panes=125 visible=25 cohort=synthetic status=MEASURED rss_kib=1693394 dedicated_gpu_bytes=421734400 instance_bytes=2304000 atlas_resident_bytes=419430400 atlas_duplicated=true renderers=125 display_links=0 command_queues=125 prep_p50_ns=346833 prep_p95_ns=424833 prep_p99_ns=449459 note=hide_show_cycles=151; per_renderer_atlas_and_queue=true; presentation_proxy=offscreen_only
row panes=250 visible=1 cohort=synthetic status=MEASURED rss_kib=1644881 dedicated_gpu_bytes=16869376 instance_bytes=92160 atlas_resident_bytes=16777216 atlas_duplicated=false renderers=250 display_links=0 command_queues=250 prep_p50_ns=326417 prep_p95_ns=396167 prep_p99_ns=396167 note=hide_show_cycles=224; per_renderer_atlas_and_queue=true; presentation_proxy=offscreen_only
row panes=250 visible=5 cohort=synthetic status=MEASURED rss_kib=1757026 dedicated_gpu_bytes=84346880 instance_bytes=460800 atlas_resident_bytes=83886080 atlas_duplicated=true renderers=250 display_links=0 command_queues=250 prep_p50_ns=325500 prep_p95_ns=376834 prep_p99_ns=436750 note=hide_show_cycles=202; per_renderer_atlas_and_queue=true; presentation_proxy=offscreen_only
row panes=250 visible=25 cohort=synthetic status=MEASURED rss_kib=2191586 dedicated_gpu_bytes=421734400 instance_bytes=2304000 atlas_resident_bytes=419430400 atlas_duplicated=true renderers=250 display_links=0 command_queues=250 prep_p50_ns=325125 prep_p95_ns=524666 prep_p99_ns=597208 note=hide_show_cycles=151; per_renderer_atlas_and_queue=true; presentation_proxy=offscreen_only
row panes=1 visible=1 cohort=real_path_fanout status=PLATFORM_LIMITED error=runtimeAttachTimeout
row panes=5 visible=5 cohort=real_path_fanout status=PLATFORM_LIMITED error=runtimeAttachTimeout
row panes=25 visible=5 cohort=real_path_fanout status=PLATFORM_LIMITED error=runtimeAttachTimeout
row panes=50 visible=0 cohort=distinct_pty_per_pane status=PLATFORM_LIMITED reason=host_pty_ceiling_documented_pass5; not_silently_reduced
row panes=125 visible=0 cohort=distinct_pty_per_pane status=PLATFORM_LIMITED reason=host_pty_ceiling_documented_pass5; not_silently_reduced
row panes=250 visible=0 cohort=distinct_pty_per_pane status=PLATFORM_LIMITED reason=host_pty_ceiling_documented_pass5; not_silently_reduced
summary synthetic_250=MEASURED real_path_fanout_rows=0 closes_issue_663=false reason=needs_product_multipane_or_full_AC_judgment_after_controlled_host_soak
```
