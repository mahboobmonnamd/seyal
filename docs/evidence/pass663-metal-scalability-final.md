# Pass 663 — Metal multi-surface scalability (final harness evidence)

**Issue:** [#663](https://github.com/mahboobmonnamd/seyal/issues/663)  
**Closing keyword:** **Closes #663** (after Issue refine accepting harness topology; product 5×10×5 chrome remains M003 [#674](https://github.com/mahboobmonnamd/seyal/issues/674))  
**Evidence class:** `controlled-host`  
**Harness:** `Seyal.app --pass663-metal-scalability` (`Pass663MetalScalability.swift`, schema `seyal.pass663.metal-scalability.v2`)  
**Host:** Apple M5 Pro, arm64, macOS 26.5.2 (Build 25F84)  
**Harness HEAD:** `4af5b06` (and later commits on this PR).  
**Matrix/plateau measurement tree:** controlled-host runs used the v2 harness working tree; regenerate with `SEYAL_PASS663_COMMIT=$(git rev-parse HEAD)` after checkout of the merge commit for exact-SHA archival. Cited numbers below are from the Debug v2 campaign on Apple M5 Pro (synthetic through 250, real_path GPU>0, display_link, one/many noisy, plateau=true @ 300 s).

## Topology honesty

| Claim | Status |
| --- | --- |
| N logical Metal surfaces 1→250 | **MEASURED** (`synthetic`) |
| 1 / 5 / 25 simultaneously visible | **MEASURED** |
| Real PTY→VT→Candidate-D→Metal fanout | **MEASURED** (`real_path_fanout`, Debug ad-hoc) |
| Distinct PTY per pane N>25 | **PLATFORM_LIMITED** (Pass 5.1 host ceiling) |
| Product 5 workspace × 10 tab × 5 pane chrome | **Out of scope** → M003 #674 |
| Release + ad-hoc codesign Candidate-D attach | **PLATFORM_LIMITED** (`helperTrustInvalid`; Debug ad-hoc allowed) |

## Commands

```bash
# Full matrix + real_path + display_link + noisy (Debug: Candidate-D attach)
SEYAL_MACOS_CONFIGURATION=Debug SEYAL_CODESIGN_IDENTITY=- bash scripts/build-macos.sh
export SEYAL_PASS663_MATRIX=1,5,25,50,125,250
export SEYAL_PASS663_VISIBLE=1,5,25
export SEYAL_PASS663_SOAK_SECONDS=5
export SEYAL_PASS663_COMMIT=$(git rev-parse HEAD)
./target/macos-derived-data/Build/Products/Debug/Seyal.app/Contents/MacOS/Seyal \
  --pass663-metal-scalability | tee docs/evidence/pass663-metal-scalability-final.log

# Plateau soak (fresh process; documented equivalent to 30‑minute wall clock when plateau=true)
export SEYAL_PASS663_SKIP_SYNTHETIC=1
export SEYAL_PASS663_SKIP_REAL_PATH=1
export SEYAL_PASS663_SKIP_DISPLAY_LINK=1
export SEYAL_PASS663_SKIP_NOISY=1
export SEYAL_PASS663_PLATEAU_SOAK=1
export SEYAL_PASS663_SOAK_SECONDS=300
./target/macos-derived-data/Build/Products/Debug/Seyal.app/Contents/MacOS/Seyal \
  --pass663-metal-scalability | tee docs/evidence/pass663-metal-scalability-plateau.log
```

## Engineering judgment (AC)

1. **Matrix through 250** — measured, not extrapolated. With `visible=1`, dedicated GPU/atlas stays ~one 16 MiB atlas at N=250. With `visible=25`, atlas scales ~25× (`atlas_duplicated=true`).
2. **Hidden release** — hide/show soak confirms non-visible surfaces do not retain surface-local atlas residency; visible=1 rows stay at single-atlas budget.
3. **Shared resources** — today's ownership is **per-renderer atlas + command queue**. Duplication under multi-visible is **expected**, not a leak. Shared device-level pool remains **measurement-gated follow-up** (Issue “likely direction”); not implemented in this PR.
4. **One noisy vs many** — `one_noisy` (real Candidate-D continuous printf into 5 surfaces) prep p99 ≈ 0.24 ms; `many_noisy` (25 visible synthetic churn) prep p99 ≈ 0.48 ms. No evidence that one noisy stream makes other surfaces' prepare path unbounded.
5. **Material defects** — no unbounded growth / retention defect found requiring a production fix in this PR. Shared-atlas work is explicitly deferred pending separate review of GPU lifetime/eviction.
6. **RSS vs GPU** — process RSS grows with renderer *count* (CPU-side objects); dedicated GPU bytes track **visible** atlas residency. Do not claim every logical pane costs a full single-renderer GPU estimate.
7. **Architecture** — harness uses production `MetalTerminalRenderer` + `RustDisplayBridge` / recovery coordinator; no commercial dependency; no fake multipane chrome.

## Headed display-link (present proxy, not scanout)

| visible | samples | present p50 | present p99 |
|--------:|--------:|------------:|------------:|
| 1 | 16 | ~8.3 ms | ~16.9 ms |
| 5 | 80 | ~6.9 ms | ~16.0 ms |
| 25 | 400 | ~7.7 ms | ~8.1 ms |

Metric = committed generation → `CAMetalDisplayLink` present proxy.

## Plateau soak

See `pass663-metal-scalability-plateau.log`. Criterion: late-window RSS coefficient of variation ≤ 5% (`plateau=true`) over `SEYAL_PASS663_SOAK_SECONDS` hide/show cycles (25 panes / 5 visible). This is the Issue's documented equivalent when a full 1800 s wall clock is unnecessary after a measured plateau.

**Harness note:** tight hide/show/rebuild loops must wrap Metal lifecycle in `autoreleasepool`. Without it, Debug phys_footprint climbs unboundedly (observed ~14 MiB → ~5.5 GiB / 300 s) even though dedicated-GPU accounting stays flat — an ObjC/Metal temporary retention artifact, not a production `NSApplication` run-loop leak. With pools (300 s, 11725 cycles): `plateau=true`, rss_first=13456 → rss_last=14144 KiB. Existing `RendererValidation` already asserts hide releases dedicated surface resources.

## Material-defect judgment (AC #5)

* **No production fix required** for shared atlas in this PR: multi-visible duplication is current ownership; shared pool remains measurement-gated follow-up.
* **Harness fix shipped:** `autoreleasepool` around hide/show cycles (before/after in plateau logs).
* Hide→release regression remains covered by `RendererValidation` dedicated-resource checks.

## Raw matrix excerpt

See `pass663-metal-scalability-final.log` (schema v2). Summary line from matrix run:

```text
summary synthetic_250=MEASURED real_path_fanout_gpu=MEASURED display_link=OK plateau=OK closes_issue_663=true reason=harness_topology_AC_met_pending_issue_refine_and_report
```
