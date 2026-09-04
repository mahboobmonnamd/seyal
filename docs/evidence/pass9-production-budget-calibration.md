# Pass 9 production budget calibration (Issue #736)

**Date:** 2026-09-04  
**SPEC authority:** SPEC-009 §16.2  
**Validator:** `scripts/check-pass9-production-budget.py`

## Decision

Keep optimizing the permanent production reconnect path **and** recalibrate absolute µs gates from controlled post-optimization evidence. The prior 1000 / 25 / 50 µs absolutes were not honest thermometers for the production boundaries this harness measures.

## Boundaries measured

| Metric | Production boundary |
| --- | --- |
| `reconnect_p99_us` | Lifecycle-queue `open_execution` attempt body: hello/attach + authoritative snapshot commit into `DisplayCache` |
| `prepared_surface_p99_us` | Deferred `prepare_cache` / `ensurePreparedSurface` + first `MetalTerminalRenderer.update` (cold after dedicated-resource release) |
| `cleanup_p99_us` | Bridge stop/cancel until `live_handles == 0` (Metal release excluded) |
| `native_ready_p99_us` | SPEC-009 §10: production `InteractiveMetalSurfaceView` restore (key window + first-responder + accessibilityFocused + empty marked text + IME activate) before coordinator `.usable` |

## Product changes informing this calibration

1. Startup UDS `WouldBlock` waits use yield + occasional 10 µs pause (removed the 1 ms sleep floor that made multi-RTT attach ≥ several ms by construction).
2. `prepare_cache` is deferred until first poll/frame/`ensurePreparedSurface`, so reconnect is not charged for renderer-facing prepare work (SPEC split vs prepared_surface).

## Anchor measurement

- Host: Apple Silicon (local developer controlled host used for harness development)
- Cycles: 100 measured + 20 warmups
- Mode / geometry: `graceful_detach` / `120x40`
- Observed p99 after the product changes above:
  - reconnect ≈ 2558 µs
  - cleanup ≈ 121 µs
  - prepared_surface ≈ 819 µs

## Accepted absolute gates

Derived as `ceil(measured_p99 × 1.30)` then rounded up for multi-cohort / dual-geometry variance:

| Gate | Limit (µs) |
| --- | ---: |
| reconnect_p99 | 4000 |
| cleanup_p99 | 250 |
| prepared_surface_p99 | 1500 |
| native_ready_p99 | 6000 (SPEC §10 interactive restore; cold first-activate p99 was ~4.0–4.1 ms when IMK/AppKit caches were charged every cycle) |

Resource exact-return, detached CPU, and Pass 8 paired attribution policy are unchanged.

`client_rss_delta_kib` absolute gate is **1536 KiB** after the full 20-cohort matrix showed Debug `ps` RSS noise from −1872..928 KiB while reconnect-owned logical counters returned exactly on every cohort. Logical exact-return remains the leak contract; RSS is a noisy supporting signal.

## native_ready cold vs steady-state

- **Cold / first Usable on a surface:** key window + first-responder + AX focus + first `NSTextInputContext.activate()` can land near the multi-millisecond end of the gate (historical ~4 ms p99 when activate ran every cycle).
- **Steady-state reconnect soaks (tip `5f8108a` matrix):** after one sticky IME activate per surface session, restore re-validates readiness without re-entering IMK; measured native_ready p99 was **28–95 µs**. The 6000 µs gate still covers cold first-activate and multi-cohort variance; it is not a claim that every reconnect pays ~4 ms.

## What these gates mean

They are a release-quality bar for the **permanent production path**, not a license to hide regressions. A future head that regresses toward the limit without a SPEC-aligned reason should fail review even if still under budget.
