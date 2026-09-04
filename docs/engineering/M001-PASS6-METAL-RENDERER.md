# M001 Pass 6 permanent Metal renderer

## Status

Pass 6 implementation contract for the permanent macOS terminal renderer. The normative behavioral authority is [`../specs/SPEC-005-M001-METAL-RENDERER.md`](../specs/SPEC-005-M001-METAL-RENDERER.md). This document records the concrete implementation ownership, validation path and benchmark methodology used by PR #659.

## Authority and ownership

The renderer is presentation-only. The authoritative execution path remains:

```text
TerminalExecution
  owns PTY + child lifecycle + canonical Seyal VT/TerminalState
        ↓
Runtime Candidate-D producer
        ↓ versioned snapshot/delta over local UDS
seyal-protocol wire schema + validation
        ↓
seyal-client committed DisplayCache
        ↓
seyal-render PreparedSurface
        ↓ coarse in-process frame
Swift/CoreText glyph preparation + Metal
        ↓
MetalSurfaceView / CAMetalLayer
```

There is no GUI VT parser, second authoritative grid, PTY replay path, NSTextView/SwiftUI terminal renderer, Ghostty renderer or CPU full-frame terminal painter.

### Rust ownership

`crates/seyal-core` owns only stable identity/value types required across authority and protocol layers. It owns no PTY, VT, Runtime registry, protocol transport or renderer state.

`crates/seyal-protocol` owns the versioned Candidate-D wire framing, display projection values/decoder, disposable `DisplayCache` commit rules and local runtime discovery/path validation. It is authority-neutral and depends only on `seyal-core` among Seyal crates. It does not own `TerminalExecution`, attachment authority, child lifecycle or terminal state.

`crates/seyal-runtime` remains execution and attachment authority. Its display module is the producer adapter from authoritative `seyal-exec` projection snapshots/updates into `seyal-protocol` Candidate-D batches. Compatibility re-exports preserve the existing internal Runtime module paths without making Runtime the protocol owner.

`crates/seyal-client` owns the disposable local attachment/client state used by the native process. Its production dependency graph is `seyal-client → seyal-protocol + seyal-render`; it does not depend on `seyal-runtime`. Runtime and `seyal-exec` are dev-dependencies only for live integration tests. The client validates and atomically commits complete Candidate-D batches before exposing them to renderer preparation. Incomplete multi-chunk batches never reach the renderer. Reads and Resync writes are nonblocking and readiness-driven after attachment; buffers, frames-per-poll, bytes-per-poll and pending display/control work are bounded.

`crates/seyal-render` owns deterministic renderer-facing normalization. `PreparedSurface` keeps one contiguous prepared-cell cache, fixed-size row damage, generation/geometry/cursor/alternate-screen presentation bookkeeping and presentation-only style/color conversion. Ordinary row damage rewrites only affected prepared rows plus local cursor consequences. Geometry, alternate-screen, backing-resource loss or explicit full invalidation rebuild the visible prepared surface.

The Rust/native boundary is one coarse `SeyalPreparedFrame` containing a pointer/length for the contiguous prepared cells plus generation, geometry, cursor, alternate-screen, full-rebuild and fixed damage words. The pointer remains Rust-owned and is consumed synchronously by Swift; native code never frees it and does not retain it across bridge mutation. There is no call per cell or glyph.

`scripts/check-layering.py` enforces the physical dependency direction for every current Seyal crate and rejects newly added `seyal-*` crates until an explicit layering rule exists. Controlled negative fixtures prove that `seyal-client → seyal-runtime` and `seyal-protocol → seyal-runtime` production edges are rejected. Dev-dependencies are intentionally excluded so integration tests can compose the real Runtime without contaminating production architecture.

### Native macOS ownership

`MetalSurfaceView` is the permanent AppKit surface and creates a `CAMetalLayer` with `.bgra8Unorm`. It owns native visibility/occlusion/backing-scale integration and asks the renderer to present only when the view/window is actually renderable.

`MetalTerminalRenderer` owns the Metal device-facing renderer state: command queue, render pipeline, sampler, reusable instance buffer, bounded one-frame-in-flight scheduling, deferred invalidation and current-frame reconstruction requests. A fresh drawable receives the complete current visible instance set; damage limits CPU rebuilding and glyph work, not drawable coverage. Production presentation is a `CAMetalDisplayLink` frame opportunity supplying a drawable → command encoding → `commandBuffer.present` → commit, with no synchronous drawable acquisition or GPU wait in the production path.

`GlyphAtlas` owns native CoreText/CoreGraphics glyph resolution/rasterization and the Metal glyph texture. The M001 atlas is a finite 2048×2048×4 `r8Unorm` array, a 16 MiB texture budget. Glyph identity includes resolved font, glyph, pixel size/backing scale and bold variant; terminal foreground/background colors do not multiply glyph entries. Capacity pressure reclaims the atlas only when no submitted command buffer can reference the old texture, then rebuilds current prepared content. Missing glyphs use a safe replacement path.

## Rendering and invalidation behavior

Terminal coordinates are top-left: row 0 is the first visible row and column 0 is the leftmost visible column. The vertex shader performs the single terminal-pixel to Metal NDC transform; no compensating per-glyph flips exist.

Default, indexed and RGB colors are presentation values. Inverse is resolved in Rust preparation. Blank-cell backgrounds are rendered independently of glyph presence. Underline and cursor are geometry/instance flags in the native renderer. Bold is a glyph/font-selection seam and participates in glyph cache identity when raster pixels differ.

Backing-scale changes invalidate pixel-dependent glyph/resources and force current-surface preparation without changing canonical rows/columns. Canonical resize input is Pass 7 and is deliberately absent here.

## Visibility and failure behavior

A hidden, miniaturized, occluded, detached or hidden-by-ancestor `MetalSurfaceView` does not continuously present. Hiding releases the surface instance buffer and glyph-atlas resources immediately when no command buffer is in flight, or after the in-flight completion when necessary. Showing requests the current committed display state and performs a reconstructable full redraw; it never asks Runtime for PTY-byte replay.

The display link is installed only while the surface is renderable and is paused between one-shot frame opportunities. This lets Core Animation provide a drawable without blocking the main actor on drawable-pool exhaustion. Drawable starvation waits for a later legitimate display-link opportunity.

Renderer recovery deliberately distinguishes failures known before GPU completion from failures reported asynchronously by a submitted command buffer:

- preparation and command/resource submission failures retain one coalesced presentation/reprepare request and use the existing finite delayed retry budget; incoming frames and layout requests cannot reset that budget;
- after the initial presentation attempt and four delayed retries, persistent pre-completion failures latch `presentationSubmissionFailuresExhausted`, stop new presentation/preparation work, and surface a diagnosable renderer-local error; a successful submission or a real hide → show lifecycle transition is required to recover;
- persistent CPU preparation failures have an independent `PreparationRecoveryState`; after the initial attempt and four delayed retries, `preparationFailuresExhausted` blocks further `renderer.update` calls from incoming Candidate-D frames until hide → show recovery;
- asynchronous command-buffer `.error` completions use a separate `GPUCompletionRetryState`, because successful submission is not proof of successful GPU execution;
- after the initial failed command completion, at most four automatic GPU resubmissions are allowed; a fifth consecutive failed completion exhausts that visible-lifecycle recovery series;
- exhaustion latches `gpuCommandCompletionFailuresExhausted`, stops automatic current-frame requests and command submissions, and is surfaced through `MetalSurfaceView.lastRenderError`;
- Candidate-D and the committed client `DisplayCache` continue advancing independently while the GPU display failure is latched; renderer damage is coalesced rather than repeatedly reshaped/rebuilt for a surface that cannot present;
- ordinary terminal output, layout and successful CPU preparation cannot reset or bypass the exhausted GPU state;
- a real hide → show lifecycle transition is the explicit recovery boundary. It clears the GPU-completion failure series and reconstructs renderer resources from the latest committed Candidate-D state.

No failure path mutates `TerminalExecution`, committed client terminal authority, attachment authority or child lifecycle. The failure state is renderer-local and disposable.

## Validation

From a clean checkout of the implementation head:

```sh
make bootstrap
make build
make test
make check
make bench
```

The Rust deterministic suite covers first/full preparation, unchanged/no-op, sparse row damage, coalesced damage, cursor old/new invalidation, style/inverse/color mapping, geometry and alternate-screen transitions, stale generation, invalid geometry/cursor/cell counts and bounded damage representation.

The protocol/client suites cover framing bounds, atomic snapshot/delta commits, generation mismatch, incomplete multi-chunk rejection, bounded client buffering and control writes, Runtime discovery/path validation, and the enforced production dependency boundary.

The native renderer self-test covers deterministic top-left pixels, blank backgrounds, underline, cursor, glyph cache reuse, bold glyph identity, backing-scale invalidation, coalesced frame-opportunity state, static-frame resubmission, hide/show reconstruction, finite atlas pressure/reclamation, in-flight GPU resource safety and repeated surface lifecycle cleanup. It separately exercises the production `CAMetalLayer` submission path rather than treating an offscreen texture as presentation proof.

The same native self-test also exercises the exact asynchronous GPU-completion recovery state machine: initial submission failure → four permitted automatic retries → exhaustion on the fifth consecutive failed completion → no further retry claims under repeated failures → explicit lifecycle reset → finite recovery available again. It also floods presentation requests between persistent pre-completion failures to prove that ordinary output cannot replenish the finite retry budget, and floods 1,000 Candidate-D frame events after persistent preparation exhaustion to prove that `renderer.update` attempts remain bounded. These tests are independent of terminal/client authority by construction, while production guards ensure an exhausted state blocks `requestPresent`, `present`, current-frame requests and repeated CPU renderer preparation until lifecycle recovery.

The live macOS acceptance harness starts a real Seyal Runtime and real shell, then proves both ordinary output and the M001 alternate-screen fixture through:

```text
shell → PTY → Seyal VT → canonical state/damage
→ Runtime Candidate-D producer → seyal-protocol
→ committed client DisplayCache → renderer preparation
→ Metal → CAMetalLayer presentation
```

No test injects terminal cells directly for the live proof.

## Benchmark methodology

`make bench` keeps Pass 5 Candidate-D transport measurements separate from Pass 6 renderer measurements.

`pass6_preparation` measures the Rust boundary from an already committed display state to prepared rows, including sparse one-row-plus-cursor and full 120×40 cases. It records repeated p50/p95/p99/max samples and exact commit/OS/architecture metadata.

The native Release renderer benchmark measures the required Pass 6 boundaries separately: `MetalTerminalRenderer.update` from committed client-generation input to prepared rows; prepared current-state batches through command creation/encoding and command-buffer commit; command-buffer commit through offscreen GPU completion; and a one-shot `CAMetalDisplayLink`/`CAMetalLayer` presentation proxy from the committed prepared generation to command commit. The last two are explicitly proxies and are not presented as physical display scanout latency. It records p50/p95/p99/max, submitted/coalesced frame counts, device/OS/geometry/scale, rebuilt rows/cells, instance bytes, glyph hits/misses/uploads, atlas budget and dedicated GPU bytes. `/usr/bin/time -lp` supplies process resource evidence in the macOS CI job.

The presentation proxy is required for interactive/local acceptance measurements. GitHub-hosted macOS runners may be headless and unable to deliver `CAMetalDisplayLink` callbacks; Foundation `native-macos-smoke` therefore sets `SEYAL_REQUIRE_DISPLAY_LINK_BENCHMARK=0` so the native benchmark may record the proxy as `PLATFORM_LIMITED` while still validating build, preparation, command submission and GPU-completion measurements. A headless CI result is not substituted for the local presentation measurement recorded below. Pass 10 evidence must label CI display-link-off benches as `CI` / non-presentation and must use headed `SEYAL_REQUIRE_DISPLAY_LINK_BENCHMARK=1` (controlled-host) for presentation criteria.

### Same-host measured baseline before final asynchronous-failure hardening

The last fully recorded local acceptance baseline before the bounded asynchronous GPU-completion failure-state correction used implementation commit `db460728741022010af9c5152fa1bb9c661afda2` on `origin/master` `5594b8a37981a29819c2b87ec0cd5f9774f76d9c`, Apple M5 Pro / macOS 26.5.2 (25F84), arm64, Rust 1.98.0, Release build, 1x backing scale, nearest-rank percentiles. The asynchronous-failure correction is outside the successful steady-state renderer update/encode hot path; final exact-head measurements are recorded durably on PR #659 after validation rather than being inferred from this baseline.

| Boundary / workload | p50 | p95 | p99 | max |
| --- | ---: | ---: | ---: | ---: |
| Rust dirty row + cursor preparation | 334 ns | 375 ns | 417 ns | 542 ns |
| Rust full 120×40 preparation | 4,625 ns | 5,375 ns | 6,250 ns | 10,708 ns |
| Native Metal preparation | 32,125 ns | 42,750 ns | 53,500 ns | 96,791 ns |
| GPU completion proxy* | 482,292 ns | 3,817,250 ns | 4,444,750 ns | 5,191,458 ns |

The native run rebuilt 160 rows / 19,200 cells, used 230,400 instance bytes, recorded 19,198 glyph-cache hits, 2 misses, 2 uploads and 306 uploaded bytes, with a 16 MiB atlas budget and 17,007,616 dedicated GPU bytes. `/usr/bin/time -lp` reported 0.66 s wall, 0.02 s user CPU, 0.04 s system CPU, 27,574,272-byte maximum RSS, and 112,919,176-byte peak footprint. The GPU value is an offscreen completion proxy including target allocation; it is not physical display scanout latency.

### Exact committed correction measurement

The bounded pre-completion failure correction was measured from implementation commit `49bb58354b8a8ff74509fa3efe65a6c2b7563415` on Apple M5 Pro / macOS 26.5.2 (25F84), arm64, Release build, 1x backing scale, nearest-rank percentiles, 120 repetitions, 120×40 geometry. The correction is renderer-state bookkeeping outside the successful steady-state update/encode path.

| Boundary / workload | p50 | p95 | p99 | max |
| --- | ---: | ---: | ---: | ---: |
| Native Metal preparation | 23,292 ns | 43,000 ns | 56,083 ns | 72,208 ns |
| GPU completion proxy* | 365,791 ns | 2,490,333 ns | 2,803,708 ns | 3,274,375 ns |

This run rebuilt 160 rows / 19,200 cells, used 230,400 instance bytes, recorded 19,198 glyph-cache hits, 2 misses, 2 uploads and 306 uploaded bytes, with a 16 MiB atlas budget and 17,007,616 dedicated GPU bytes. The GPU value remains an offscreen completion proxy, not physical display scanout latency. The benchmark was run before this documentation-only evidence update; the commit hash identifies the exact code under measurement. This older record predates the separate-boundary instrumentation and is retained only as the prior correction baseline; the exact current-head measurements are required below before Pass 6 closure.

The full `make bench` Candidate-D run also covered the required fanout/workload/geometry matrix. At 16 viewers and sustained 2-second high output at 200×60 it recorded p95/p99 client-cache latency of 3,237/3,627 µs, 16,544 KiB populated RSS, 22.6% sampled CPU, `3,568` latency samples, 678,423,552 aggregate UDS bytes, `shutdown_ok=true`, and final pending input `0`. The 50- and 100-execution cases were explicitly `PLATFORM_LIMITED` at 34 created executions because the host returned `Device not configured (os error 6)`; this is retained platform evidence, not a Seyal capacity claim.

### Current-head four-boundary measurement

The current executable head `a8b891683b3bae0fa7ddbdf3f9af4628ba12611e` was measured locally on an Apple M5 Pro running macOS 26.5.2 (25F84), arm64, Release build, 1x backing scale, 120×40 geometry and 120 repetitions. Percentiles use nearest-rank. The run built the current `Seyal.app`, used a visible AppKit window and recorded 120/120 one-shot `CAMetalDisplayLink` samples.

| Boundary | p50 | p95 | p99 | max |
| --- | ---: | ---: | ---: | ---: |
| Committed generation → prepared rows | 1,625 ns | 182,083 ns | 866,084 ns | 1,106,834 ns |
| Prepared batch → command-buffer commit | 49,791 ns | 76,959 ns | 202,167 ns | 794,500 ns |
| Command commit → GPU completion proxy | 784,292 ns | 1,009,708 ns | 1,413,625 ns | 3,314,666 ns |
| Committed generation → presented-frame proxy | 7,129,667 ns | 7,642,083 ns | 7,794,209 ns | 8,513,917 ns |

The run submitted 120 frames and recorded 98 coalesced frames, rebuilt 121 rows / 14,520 cells, used 230,400 instance bytes, recorded 14,518 glyph-cache hits, 2 misses, 2 uploads and 306 uploaded bytes, with a 16 MiB atlas budget and 17,007,616 dedicated GPU bytes. `/usr/bin/time -lp` reported 1.59 s wall, 0.11 s user CPU, 0.16 s system CPU, 44,924,928-byte maximum RSS and 138,379,960-byte peak memory footprint. The GPU and presented-frame values are explicitly proxies; they do not claim physical display scanout latency.

Against the closest same-host pre-fix renderer run `7084915d940d486623e1e10c72fb05cdd4772d3c`, repeated accepted measurements remained in the same range as the prior 26,500 ns / 386,958 ns medians. This does not establish an absolute product latency target. The exact post-correction measurements are recorded above and will also be attached to PR #659 for independent review.

## Deferred behavior

Pass 6 does not implement native key input, IME or canonical resize transactions; those belong to Pass 7. It also does not claim complete grapheme/emoji/wide-character/bidirectional/ligature correctness, image protocols, Blocks/history, tabs/splits/product chrome, remote rendering or a speculative Windows/Linux renderer abstraction.
