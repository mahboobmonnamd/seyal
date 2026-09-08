# M002 #817 exact-head grapheme display evidence record

- **Issue:** #817
- **Authority:** SPEC-011 / ADR-011 grapheme display projection and the #817 acceptance gates
- **Measured production code head:** `d9775b85fb5659cb0e62e0b5c375344416fbd417`
- **Exact evidence-test head:** `432c493dbb533bb891c8504995866c48c942d19e`
- **Final docs-only descendant:** `f5f9f95c477017912d1da32ae09931478e546663`
- **Recorded:** 2026-09-08
- **Host/build boundary:** local Apple Silicon macOS host
- **Claim status:** focused automated evidence plus a retained headed Unicode XCUI case; real IME and release performance gates remain open

## Automated evidence

The exact production head passed the focused runtime/client boundaries:

```text
cargo test -p seyal-runtime --lib --locked                 # 45 passed
cargo test -p seyal-runtime --test m002_grapheme_attach --locked  # 1 passed
cargo test -p seyal-runtime --test pass7_local_ipc --locked      # 6 passed
cargo fmt --all -- --check
git diff --check
make check                                                        # exit 0
```

The production fix preserves frame boundaries when a display frame is
partially written: mandatory and after-display frames finish at frame
boundaries, so control bytes cannot be inserted into a display payload. The
grapheme attach test negotiates the display capability, receives a v2 snapshot,
and applies it through the client path.

The exact-head repository gate completed with exit 0, including the packaged
ARM64 production build, native smoke, deterministic renderer/input/recovery
self-tests, Pass 8 Runtime-to-Swift metadata acceptance, live Candidate-D
renderer probes, and the headed XCTest/XCUI workload. The canonical native CI
job retained the Unicode screenshot artifact. The local XCTest runner could
not start under the restricted host because `com.apple.testmanagerd.control` is
unavailable. `testProductionUnicodeCommandRetainsHeadedRenderedEvidence`
sends combining, CJK-wide, ZWJ emoji, RI flag, Devanagari, and Arabic output
through the production PTY; its captured bytes are asserted against the exact
UTF-8 fixture before the screenshot is retained.

## Exact-head ARM64 Unicode diagnostic

The controlled release benchmark was run with
`SEYAL_REQUIRE_DISPLAY_LINK_BENCHMARK=1 SEYAL_CODESIGN_IDENTITY=- make bench`
on the Apple M5 Pro host (macOS 26.5.2, arm64, Rust 1.98.0) at `d9775b8`.
The production Metal/CoreText path completed successfully and emitted:

```text
pass6_native_renderer ... geometry=120x40 repetitions=120 ... display_link_samples=120
m002_unicode_renderer ... workload=complete_grapheme_fallback geometry=120x40 grapheme_leads=3078
m002_unicode_renderer cache_phase=cold_miss preparation_ns=11467917 full_shaping_ns=6799958 font_fallback_runs=6 process_cpu_ns=8664000 process_rss_bytes=52461568
m002_unicode_renderer cache_phase=warm_hit preparation_ns=890542 full_shaping_ns=0 font_fallback_runs=0 process_cpu_ns=897000 process_rss_bytes=52461568
```

These are diagnostic measurements with `performance_claim=false`; the Unicode
baseline is explicitly `UNSUPPORTED_NONCOMPARABLE`. They do not close the
#673 release ceilings, multi-execution scaling, or physical IME latency gate.

The prior canonical native CI run exposed a real startup layout defect: the
scroll document could report a transient narrow width, causing the Metal
surface to propose narrow terminal geometry and become non-hittable. Exact
head `d9775b8` forces the document and surface through a completed layout pass
and makes the timeline overlay pass empty-area input through to the surface.
Focused component regressions cover initial/resize viewport tracking, the exact
geometry samples emitted by layout, and populated Block overlay hit-testing.
The composer Return test and the new Unicode headed case require the
canonical native CI rerun to prove the Runtime timeline Block is visibly
published and to retain the screenshot artifact.

## Acceptance ledger

| Gate | Status | Evidence / remaining work |
| --- | --- | --- |
| Grapheme-capable attach and v2 snapshot | **Automated** | Runtime attach and snapshot decode/apply test pass. |
| Partial-write frame integrity | **Automated** | Regression covers display remainder followed by mandatory and after-display control frames. |
| TerminalState sole authority | **Reviewed** | Runtime remains the canonical terminal owner; client/renderer consume projections. |
| Native grapheme rendering | **Automated (CI)** | Production-path headed XCUI case passed on the canonical runner with screenshot retention; visual glyph-matrix review remains manual. |
| AppKit IME and candidate interaction | **Unverified** | Requires real headed IME commit/cancel/replacement evidence. |
| Unicode-heavy latency/RSS | **Diagnostic measured** | Exact-head ARM64 M5 Pro Unicode shaping/cache/RSS output is retained above; release ceilings, scaling matrix, and IME latency remain open. |
| `make check` | **Automated** | Exact head `d9775b8` completed the repository checks with exit 0 on the Apple Silicon host. |
| Independent review | **GO** | Exact-head review of implementation/test head `432c493dbb533bb891c8504995866c48c942d19e` found no P0-P2 findings; final descendant `f5f9f95c477017912d1da32ae09931478e546663` is docs-only. Manual IME and release performance gates remain open. |

This record documents the tested boundaries and does not authorize merging or
closing #817 while the native, manual, and performance gates remain open.
