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

`MetalTerminalRenderer` owns the Metal device-facing renderer state: command queue, render pipeline, sampler, reusable instance buffer, bounded one-frame-in-flight scheduling, deferred invalidation and current-frame reconstruction requests. A fresh drawable receives the complete current visible instance set; damage limits CPU rebuilding and glyph work, not drawable coverage. Production presentation is `CAMetalLayer.nextDrawable` → command encoding → `commandBuffer.present` → commit, with no synchronous GPU wait in the production path.

`GlyphAtlas` owns native CoreText/CoreGraphics glyph resolution/rasterization and the Metal glyph texture. The M001 atlas is a finite 2048×2048×4 `r8Unorm` array, a 16 MiB texture budget. Glyph identity includes resolved font, glyph, pixel size/backing scale and bold variant; terminal foreground/background colors do not multiply glyph entries. Capacity pressure reclaims the atlas only when no submitted command buffer can reference the old texture, then rebuilds current prepared content. Missing glyphs use a safe replacement path.

## Rendering and invalidation behavior

Terminal coordinates are top-left: row 0 is the first visible row and column 0 is the leftmost visible column. The vertex shader performs the single terminal-pixel to Metal NDC transform; no compensating per-glyph flips exist.

Default, indexed and RGB colors are presentation values. Inverse is resolved in Rust preparation. Blank-cell backgrounds are rendered independently of glyph presence. Underline and cursor are geometry/instance flags in the native renderer. Bold is a glyph/font-selection seam and participates in glyph cache identity when raster pixels differ.

Backing-scale changes invalidate pixel-dependent glyph/resources and force current-surface preparation without changing canonical rows/columns. Canonical resize input is Pass 7 and is deliberately absent here.

## Visibility and failure behavior

A hidden, miniaturized, occluded, detached or hidden-by-ancestor `MetalSurfaceView` does not continuously present. Hiding releases the surface instance buffer and glyph-atlas resources immediately when no command buffer is in flight, or after the in-flight completion when necessary. Showing requests the current committed display state and performs a reconstructable full redraw; it never asks Runtime for PTY-byte replay.

Temporary drawable unavailability preserves pending presentation state and returns without a busy retry. Command-buffer failure invalidates presentation assumptions and requests a reconstructable current frame. Renderer/resource failure does not mutate `TerminalExecution`, committed client terminal authority, attachment authority or child lifecycle.

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

The native renderer self-test covers deterministic top-left pixels, blank backgrounds, underline, cursor, glyph cache reuse, bold glyph identity, backing-scale invalidation, drawable-unavailable state, hide/show reconstruction, finite atlas pressure/reclamation, in-flight GPU resource safety and repeated surface lifecycle cleanup. It separately exercises the production `CAMetalLayer` path rather than treating an offscreen texture as presentation proof.

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

The native Release renderer benchmark measures `MetalTerminalRenderer.update` for sparse 120×40 damage and a Metal GPU-completion proxy using an offscreen target. The latter is explicitly a proxy and is not presented as display scanout latency. It records p50/p95/p99/max, device/OS/geometry/scale, rebuilt rows/cells, instance bytes, glyph hits/misses/uploads, atlas budget and dedicated GPU bytes. `/usr/bin/time -lp` supplies process resource evidence in the macOS CI job.

### Final exact-head evidence

The final local acceptance run used implementation commit `a7abeb50c179be293eca5b4355084931a527cbdc` on `origin/master` `5594b8a37981a29819c2b87ec0cd5f9774f76d9c`, Apple M5 Pro / macOS 26.5.2 (25F84), arm64, Rust 1.98.0, Release build, 1x backing scale, nearest-rank percentiles. The closest pre-fix comparison is the immediately preceding PR head `7084915d940d486623e1e10c72fb05cdd4772d3c` on the same host and OS; these are repeated local measurements, not a cross-machine product threshold.

| Boundary / workload | p50 | p95 | p99 | max |
| --- | ---: | ---: | ---: | ---: |
| Rust dirty row + cursor preparation | 333 ns | 458 ns | 500 ns | 7,083 ns |
| Rust full 120×40 preparation | 4,500 ns | 4,958 ns | 6,000 ns | 6,166 ns |
| Native Metal preparation | 33,208 ns | 47,542 ns | 61,042 ns | 120,625 ns |
| GPU completion proxy* | 477,667 ns | 3,531,042 ns | 4,861,791 ns | 5,497,459 ns |

The native run rebuilt 160 rows / 19,200 cells, used 230,400 instance bytes, recorded 19,198 glyph-cache hits, 2 misses, 2 uploads and 306 uploaded bytes, with a 16 MiB atlas budget and 17,007,616 dedicated GPU bytes. `/usr/bin/time -lp` reported 0.64 s wall, 0.02 s user CPU, 0.04 s system CPU, 27,623,424-byte maximum RSS, and 112,984,712-byte peak footprint. The GPU value is an offscreen completion proxy including target allocation; it is not physical display scanout latency.

The full `make bench` Candidate-D run also covered the required fanout/workload/geometry matrix. At 16 viewers and sustained 2-second high output at 200×60 it recorded p95/p99 client-cache latency of 2,956/3,382 µs, 16,336 KiB populated RSS, 22.7% sampled CPU, `3,568` latency samples, 678,423,552 aggregate UDS bytes, `shutdown_ok=true`, and final pending input `0`. The 50- and 100-execution cases were explicitly `PLATFORM_LIMITED` at 34 created executions because the host returned `Device not configured (os error 6)`; this is retained platform evidence, not a Seyal capacity claim.

Against the same-host pre-fix renderer run, the current implementation remains in the same order of magnitude; the latest exact-head run measured 26,333 ns / 373,875 ns medians versus the prior 26,500 ns / 386,958 ns. This supports no material regression judgment for this workload only; it does not establish an absolute product latency target. The durable PR validation comment records the same measurements against the exact implementation commit.

## Deferred behavior

Pass 6 does not implement native key input, IME or canonical resize transactions; those belong to Pass 7. It also does not claim complete grapheme/emoji/wide-character/bidirectional/ligature correctness, image protocols, Blocks/history, tabs/splits/product chrome, remote rendering or a speculative Windows/Linux renderer abstraction.
