# SPEC-005 — M001 permanent Metal renderer

- **Status:** Accepted for M001 Pass 6 via PR #657
- **Date:** 2026-08-27
- **Issue:** #656
- **Architecture authority:** accepted foundation architecture; `MILESTONE-001.md` section 11 and Pass 6
- **Depends on:** SPEC-001, SPEC-004

## 1. Purpose

This specification defines the first permanent Seyal terminal renderer on macOS.

The renderer starts only after Candidate-D display state has been validated and atomically committed in the local client:

```text
real process
→ PTY
→ Seyal VT
→ canonical TerminalState/damage
→ Candidate-D snapshot/delta over local UDS
→ disposable client DisplayCache / RenderState
→ renderer-facing damage + coarse prepared batches
→ shaping / font fallback
→ glyph cache / atlas
→ Metal command encoding
→ CAMetalLayer drawable
→ present
```

The renderer is a presentation consumer. It never becomes terminal authority.

## 2. Non-negotiable invariants

1. `TerminalExecution` remains the sole PTY, child-lifecycle and canonical `TerminalState` owner.
2. Seyal.app owns only validated, disposable display state and renderer/GPU state.
3. The GUI must not parse PTY bytes, run a second VT engine, reconstruct terminal history, or make a rendered grid authoritative.
4. Metal is the first production terminal renderer. There is no production NSTextView, SwiftUI text renderer, CPU full-frame text painter, Ghostty renderer, or other fallback terminal engine.
5. PTY → VT → canonical state progress never waits for renderer acknowledgement, drawable availability, GPU completion or presentation.
6. Rust/native boundaries are coarse. No production path performs one Rust↔Swift/native call per cell or per glyph.
7. Ordinary partial damage must reduce CPU-side shaping/rasterization/batch-rebuild work. It must not force unconditional full-grid CPU preparation.
8. Damage is not a promise that a Metal drawable preserves prior pixels. A final render pass may re-issue cached prepared content for the full visible surface when required by drawable semantics, provided unchanged content is not re-shaped, re-rasterized or rebuilt unnecessarily.
9. Renderer work is bounded and replaceable. No unbounded frame queue, shaping cache, glyph cache, transient buffer growth or hidden-surface render loop is permitted.
10. Hidden/occluded/detached surfaces do not continuously render and do not retain unnecessary dedicated GPU render resources.
11. Renderer failure cannot mutate canonical terminal state, attachment/controller authority, or child lifecycle.

## 3. Renderer input authority

### 3.1 Committed client state only

SPEC-004 is the display-model authority. A renderer consumes only an atomically committed client `DisplayCache` / `RenderState` and the presentation invalidation produced while applying that committed state.

A partial or malformed multi-chunk display update is never visible to the renderer.

The renderer must not consume raw socket chunks directly as drawable truth. The order is:

```text
validate complete Candidate-D update
→ atomically commit client display state
→ derive/merge renderer invalidation
→ schedule draw
```

### 3.2 Generations

The renderer tracks the latest client display generation it has prepared/presented only as disposable presentation bookkeeping. This generation is not canonical terminal authority.

Multiple committed display generations may arrive before one frame is encoded. The renderer may coalesce them and draw the latest committed state, provided invalidation covers every visible consequence of skipped intermediate frames.

No requirement exists to present every generation.

### 3.3 Damage and local invalidation

Client update application exposes row/cell invalidation sufficient for incremental CPU-side draw preparation.

A snapshot, geometry change, renderer-resource recreation, display-scale change or loss of retained prepared state causes full visible invalidation.

A normal delta invalidates only its changed visible rows/cells plus any renderer-local consequences such as the old and new cursor cells.

If the committed display generation and all renderer-local presentation inputs are unchanged, draw preparation must be able to return no new preparation work.

Damage constrains recomputation, not necessarily final drawable coverage. `CAMetalLayer` drawable contents must not be assumed to preserve a prior frame. The implementation may therefore construct a complete visible frame by re-issuing cached prepared geometry/instances for unchanged rows while rebuilding only damaged rows. This is compliant; re-shaping, re-rasterizing or reconstructing unchanged rows solely because a new drawable was acquired is not.

## 4. Renderer state ownership

The renderer may own only derived presentation resources, including:

- current surface geometry in pixels;
- display/backing scale;
- font selection and fallback handles;
- bounded shaping/rasterization/prepared-row caches;
- glyph atlas textures and metadata;
- style/color lookup tables;
- reusable instance/vertex/uniform buffers;
- Metal device/queue/pipeline state;
- frame scheduling/coalescing state;
- presentation generation/invalidation bookkeeping.

The renderer must not own:

- PTY or child process;
- VT parser/modes;
- canonical terminal grid;
- canonical cursor/mode state;
- scrollback/history authority;
- Block timeline/semantic state;
- attachment/controller authority.

All renderer state is disposable and reconstructable from current committed client display state plus local font/surface configuration.

## 5. Coordinate and geometry contract

Seyal has one terminal presentation coordinate system:

```text
terminal row 0 = top visible row
terminal column 0 = left visible column
```

The native renderer may use Metal's preferred coordinate system internally, but exactly one explicit transform must map terminal top-left coordinates into GPU coordinates. Per-glyph or per-path compensating flips are forbidden.

For a cell at `(row, column)`:

- horizontal placement is derived from `column × cell_width`;
- vertical placement is derived from `row × cell_height` from the top edge;
- glyph baseline is derived from selected font metrics inside that cell;
- background and cursor geometry use the same cell rectangle as glyph placement.

The renderer must not infer terminal rows/columns from pixels as authoritative state. Canonical geometry arrives through committed display state; native surface size only determines pixel layout and later Pass-7 resize proposals.

Orientation tests must fail for vertical inversion, horizontal mirroring or row/column transposition.

## 6. Coarse renderer-preparation boundary

MILESTONE-001 assigns renderer-facing normalization/batching to Rust and platform font/Metal lifecycle to native macOS code.

The production boundary must therefore transfer coarse arrays/runs/batches rather than per-cell callbacks.

A renderer batch must be able to represent, at minimum:

- generation / invalidation identity;
- visible geometry;
- dirty row/cell ranges;
- background rectangles or equivalent cell instances;
- glyph instances referencing resolved glyph/cache identities;
- foreground/style information not baked unnecessarily into glyph texture identity;
- underline geometry for the M001 underline attribute;
- cursor rectangle/visibility information;
- full-invalidation marker when required.

The exact Rust struct layout is not an ABI. Native-facing transfer must use an explicit stable in-process representation appropriate to the chosen FFI/bridge and must not expose Rust pointers with lifetimes that outlive the call/buffer contract.

One/few coarse transfers per committed generation/frame are acceptable. A call per cell or glyph is not.

The renderer may retain bounded prepared row/run data so unchanged content can be re-issued to a fresh drawable without repeating expensive preparation. Cache keys must include every local input that can change the prepared result, and cache invalidation must be deterministic.

Do not create a speculative cross-platform GUI/renderer framework for platforms not yet under development.

## 7. M001 cell/style rendering

SPEC-004 provides one Unicode scalar and presentation attributes per visible cell.

M001 rendering supports the current display-model subset:

- default foreground/background;
- indexed terminal colors;
- 24-bit RGB colors;
- bold/intensity seam;
- underline;
- inverse;
- cursor visibility and position;
- primary/alternate-screen committed state.

Inverse is resolved as presentation by exchanging effective foreground/background for the affected cell.

Underline is rendered as geometry derived from font/cell metrics and does not require a separate glyph rasterization.

Background drawing must not depend on glyph presence.

Invalid/reserved display-model values are rejected before renderer input by SPEC-004; renderer code still must remain bounds-safe if handed an empty batch, zero dirty ranges or locally missing font/glyph resources.

## 8. Shaping and font fallback seam

M001 deliberately does not claim full grapheme-cluster, emoji-width, bidirectional or complex-script terminal correctness. Pass 6 must nevertheless establish the permanent shaping/fallback architecture so later correctness expands shaping inputs/caches rather than replacing the renderer.

Requirements:

1. Font discovery and fallback use native macOS font facilities.
2. Font resolution is separated from terminal-state authority.
3. A missing glyph in the primary face may resolve through fallback without changing terminal cell geometry or canonical state.
4. If no usable glyph can be resolved, rendering uses a safe replacement/missing-glyph presentation rather than crashing or blocking Runtime progress.
5. Shaping results that are expensive enough to cache must use a bounded cache whose identity includes every input that can change glyph selection/placement, such as text/run content, resolved font/fallback identity, relevant font features/configuration and scale/metrics inputs.
6. Shaping/prepared-run cache invalidation must be deterministic on font/configuration/scale changes; terminal foreground/background color alone must not invalidate shape identity.
7. M001 deterministic pixel tests may use a controlled bundled/system-stable test face or synthetic glyph fixtures where permitted by repository policy; tests must not assume every macOS release rasterizes arbitrary fonts identically.
8. Future grapheme/width/ligature shaping must be able to replace the narrow scalar-to-glyph preparation stage without changing Metal surface ownership or PTY/VT authority.

For M001, a visible cell remains the placement unit provided by SPEC-004. Pass 6 must not invent width, grapheme or ligature semantics that canonical terminal state does not yet provide.

## 9. Glyph cache and atlas contract

Glyph rasterization is cached and uploaded to Metal textures for reuse. Steady-state frames must not rasterize every visible glyph again.

Glyph cache identity must include every parameter that materially changes rasterized glyph pixels, including at least:

- resolved font face identity;
- glyph identity;
- effective font size/raster scale;
- backing/display scale;
- synthetic/selected font variant when bold changes resolved glyph pixels;
- rasterization mode relevant to the atlas representation.

Foreground/background terminal colors are not part of monochrome glyph texture identity and must not multiply identical glyph entries.

The atlas/cache must have a finite documented production budget and deterministic eviction/reclamation behavior. Exact page dimensions, texture format strategy and budget size are implementation details unless they affect observable behavior, but an implementation with unbounded atlas growth cannot pass this specification.

Required behavior under pressure:

```text
cache hit
→ reuse existing glyph location

cache miss with capacity
→ rasterize/upload once

cache miss under budget pressure
→ evict/reclaim eligible entries/pages
→ upload if possible

allocation still unavailable
→ fail the frame/surface safely
→ preserve terminal authority
```

Eviction must never invalidate an in-flight command buffer's referenced texture region. Resource lifetime must cover all submitted GPU work that can still reference it.

Font configuration or backing-scale changes invalidate affected glyph identities; unrelated terminal color changes do not require atlas flush.

M001 does not require production color-emoji/graphics rendering, but atlas/resource ownership must not require replacing the Metal surface architecture when a later specified color-glyph/image path is added.

## 10. Metal ownership and frame lifecycle

Native macOS code owns:

- `MTLDevice`;
- command queue;
- render pipeline state;
- `CAMetalLayer` / drawable sizing;
- atlas textures and uploads;
- reusable/transient GPU buffers;
- command buffer and render command encoder creation;
- presentation.

The existing `MetalSurfaceView`/`CAMetalLayer` is the permanent surface seam; Pass 6 extends it rather than replacing it with a text view.

A visible frame follows conceptually:

```text
committed DisplayCache state
→ merge damage/local invalidation
→ rebuild only invalid prepared rows/runs
→ resolve/cache only missing glyph resources
→ combine cached + rebuilt prepared content for current visible state
→ acquire drawable
→ encode a correct current frame
→ commit command buffer
→ present drawable
```

The specification requires incremental preparation, not partial-present behavior. A Metal render pass may draw the complete visible terminal from cached prepared/GPU state when a fresh drawable requires it. The implementation must not rely on undefined/preserved previous drawable contents merely to claim lower draw counts.

Frame scheduling must be bounded:

- at most bounded pending presentation work per surface;
- newer committed state may supersede not-yet-encoded older frame work;
- no queue grows with terminal output rate;
- no busy loop runs when there is no damage/local invalidation/exposure requirement;
- renderer completion is never required for Runtime to accept more terminal output.

The implementation may use platform frame/display scheduling primitives, but the scheduling mechanism must preserve these invariants and must be measured before being treated as a performance claim.

## 11. Cursor invalidation

Cursor state is present in every Candidate-D display update header and may change even when cell contents do not.

The renderer tracks the previously presented cursor rectangle as disposable state.

When cursor visibility or position changes, preparation invalidation includes:

- the old cursor cell if it was visible;
- the new cursor cell if it is visible.

This prevents cursor trails without forcing unrelated rows to be re-prepared.

M001 cursor style value `0` uses one defined renderer presentation chosen by the implementation and covered by deterministic tests. Broader cursor-style protocol support is deferred until terminal/display-model authority defines it.

## 12. Resize, backing scale and surface changes

### 12.1 Terminal geometry change

When committed display rows/columns change, the renderer treats the visible terminal as fully invalidated and recomputes cell-to-pixel layout.

The renderer does not initiate or commit canonical terminal resize in Pass 6. Pass 7 owns the native input/resize transaction back to Runtime.

### 12.2 Backing/display scale change

When AppKit backing scale changes:

- `CAMetalLayer.drawableSize` follows backing pixels;
- pixel-space font/glyph/shaping/prepared-cache identities affected by the scale change are invalidated or separately keyed;
- surface layout is fully invalidated;
- terminal rows/columns remain whatever the committed client state says until Pass-7 resize authority changes them.

### 12.3 Font change seam

If a local font configuration change is introduced later, it invalidates cell metrics, affected shape/glyph/prepared caches and the visible surface. Pass 6 may use one fixed/configured M001 font path; it must still keep cache identity and layout seams capable of correct invalidation.

## 13. Hidden, occluded and detached surfaces

A surface that is not visible must not continuously encode/present terminal frames.

When hidden/occluded:

- committed client display state may continue advancing independently;
- renderer invalidation may coalesce to a single `needs current/full redraw when visible` state;
- surface-local transient frame/drawable resources are released or allowed to drain;
- no per-frame display scheduling continues solely because terminal output is arriving.

When a terminal surface is detached/destroyed:

- release surface-local GPU buffers, drawable references and scheduling objects after in-flight GPU safety permits;
- shared device/pipeline/global glyph-cache resources may remain only under their own bounded lifecycle if other visible surfaces use them;
- no renderer resource may keep `TerminalExecution` alive.

On becoming visible again, the renderer redraws from current committed DisplayCache and reconstructable prepared resources without requesting PTY-byte replay. If client display state itself is absent/stale, SPEC-004 attach/resync semantics are used; the renderer does not invent recovery.

## 14. Failure and recovery behavior

### 14.1 Metal device unavailable

A production terminal surface requires Metal. If no suitable Metal device exists, Seyal must fail the terminal surface/app startup with an explicit diagnosable error. It must not silently fall back to NSTextView, SwiftUI, CPU full-frame text painting or another terminal engine.

### 14.2 Drawable temporarily unavailable

If `nextDrawable` or equivalent drawable acquisition is temporarily unavailable:

- do not block Runtime/client display-state progress;
- do not busy retry;
- retain/coalesce invalidation;
- retry on a later legitimate visibility/frame opportunity.

### 14.3 Pipeline/resource creation failure

Pipeline, texture or buffer allocation failure leaves the surface unpresented but terminal authority intact. The renderer may reclaim bounded caches/resources and retry according to an explicitly bounded policy. Persistent failure becomes a visible/diagnosable display failure rather than an infinite retry loop.

### 14.4 GPU command failure

A failed command buffer invalidates assumptions about the affected surface presentation. Subsequent successful rendering must perform sufficient redraw/recreation to produce current state. Failure does not roll back or mutate committed client DisplayCache or Runtime state.

Repeated device/command failures must not create unbounded logs, allocations or retry frequency.

## 15. Performance and resource requirements

Pass 6 is latency-sensitive production code.

Forbidden steady-state patterns include:

- per-cell Rust/native calls;
- per-cell heap allocation;
- full-grid CPU rasterization for ordinary partial damage;
- re-shaping/rebuilding unchanged rows every frame when valid prepared data exists;
- rasterizing the same cached glyph every frame;
- unbounded frame queues;
- unbounded shaping/prepared/glyph-cache growth;
- synchronous wait for GPU completion before Runtime/IPC can progress;
- hidden-surface continuous drawing;
- renderer-driven polling of Runtime state.

A complete GPU render pass over cached visible instances is not itself a violation. The performance requirement is that damage prevent unnecessary CPU preparation/rasterization/copy/allocation work and that measured GPU work remain acceptable.

Renderer-prep structures and GPU buffers should be reusable/capacity-managed where measured evidence justifies it.

Required measured boundaries, where the platform exposes reliable timestamps:

1. committed client display generation → invalid rows/runs prepared;
2. prepared current-state batches → command buffer committed;
3. command buffer committed → GPU completion/presentation proxy;
4. committed display generation → presented-frame proxy;
5. end-to-end PTY/display-model measurements remain combined later in M001 Pass 10.

For repeated workloads record p50/p95/p99 where meaningful plus:

- CPU time/usage;
- app RSS;
- GPU/resource bytes where measurable or explicitly estimated;
- dirty rows/cells versus rebuilt prepared rows/instances;
- total GPU instances/quads/draw calls where meaningful;
- command-buffer/frame count;
- skipped/coalesced frame count;
- shape/prepared-cache hits/misses/evictions where implemented;
- glyph cache hits/misses/evictions;
- glyph rasterizations/uploads and uploaded bytes;
- buffer/texture allocations and reallocations;
- hidden/visible surface resource counts;
- geometry, font, backing scale, hardware, macOS, build mode and commit SHA.

Measurements are evidence, not permission to weaken correctness or damage semantics.

## 16. Deterministic test contract

Pass 6 implementation starts test-first. Deterministic fixtures must exercise renderer preparation without requiring a live PTY.

Required cases:

1. empty/unchanged committed generation produces no new CPU preparation;
2. full snapshot produces full visible preparation invalidation;
3. one dirty row rebuilds preparation only for that row plus required cursor effects, while a final GPU frame may re-issue cached unchanged rows;
4. multiple coalesced updates preserve all final-state invalidation;
5. row 0 renders at the top and column 0 at the left;
6. vertical inversion/horizontal mirroring/transposition regressions fail;
7. default/indexed/RGB color mapping;
8. bold/underline/inverse mapping;
9. background rendering with blank/no-glyph cells;
10. cursor visible/hidden and move invalidates old/new cells only where possible;
11. primary/alternate committed display state is rendered without a second terminal engine;
12. geometry change causes full preparation invalidation;
13. backing-scale change invalidates pixel-dependent shape/glyph/layout state;
14. shape/cache identity changes for relevant text/font/scale inputs but not foreground color alone;
15. glyph cache identity changes for raster-affecting font/scale inputs but not foreground color alone;
16. repeated glyphs reuse atlas entries;
17. bounded shape/prepared/glyph-cache pressure evicts/reclaims without referencing freed in-flight resources;
18. hidden/occluded surface schedules no continuous draw work;
19. show-after-hidden redraws current committed state;
20. drawable-unavailable path retains invalidation without busy retry;
21. repeated create/show/hide/destroy returns surface-local GPU/resource counters to baseline after in-flight completion.

Controlled pixel/golden tests may be added for synthetic/stable glyph and geometry fixtures. Golden tests must not encode undocumented platform-font rasterization noise as correctness.

## 17. Live integration acceptance

After deterministic fixtures pass, production implementation must render real Candidate-D client state from a live Runtime attachment through the same renderer path.

The live demonstration must prove:

```text
real shell output
→ PTY
→ Seyal VT
→ canonical TerminalState
→ Candidate-D client DisplayCache
→ permanent renderer preparation
→ Metal
→ pixels
```

Acceptance requires:

- no GUI VT/parser or second canonical grid;
- no temporary renderer path;
- normal text and basic ANSI colors visible;
- cursor position/visibility correct for M001 fixtures;
- partial damage reduces CPU preparation to changed rows/cells plus local invalidation;
- final Metal drawing does not assume prior drawable contents are preserved;
- resize/scale renderer invalidation is correct even though native resize control wiring remains Pass 7;
- alternate-screen M001 fixture can be rendered from committed display state;
- hidden-surface resource behavior passes;
- deterministic renderer suite is green;
- native build/test/check gates are green;
- renderer performance/resource evidence is recorded from the exact implementation head;
- independent architecture/performance/code-quality review has no unresolved blocker.

## 18. Explicit non-goals

Pass 6 does not implement or claim:

- AppKit key routing, IME completion or Runtime resize command path (Pass 7);
- production Blocks/history/composer semantics (Pass 8 and later UI work);
- GUI detach/crash lifecycle proof (Pass 9);
- full M001 cross-layer benchmark closure (Pass 10);
- full grapheme/emoji/wide-character/bidirectional/ligature correctness beyond the permanent shaping/fallback seam;
- Kitty, Sixel or iTerm image protocols;
- public renderer/plugin API;
- remote rendering;
- Windows/Linux renderer abstraction;
- agent, cloud, Teams or Enterprise behavior;
- libghostty/Ghostty as production terminal or renderer authority.

## 19. Pass-6 specification acceptance gate

SPEC-005 is accepted only when:

- it agrees with SPEC-004 Candidate-D client-state ownership;
- it preserves the accepted Rust/native responsibility split and coarse boundary;
- it distinguishes incremental CPU preparation from final drawable coverage;
- it defines deterministic damage/orientation/cursor/cache/resource behavior precisely enough to write failing tests before implementation;
- failure behavior cannot create a renderer fallback that violates architecture;
- performance/resource measurement requirements are explicit;
- independent review finds no unresolved architecture or performance blocker.

After acceptance, create/refine exactly one production Pass-6 implementation Issue. Implementation may not silently broaden this specification; behavior that changes authority or the renderer boundary requires the architecture process first.