---
name: metal-renderer
description: Design, implement or review Seyal's production macOS Metal renderer with explicit ownership, damage-driven drawing and measurable CPU/GPU/resource behavior.
---

# Metal renderer

Use this skill for terminal presentation, glyph/text rendering, atlases, backing-scale handling, frame scheduling, GPU resources, damage tracking integration, visual effects or renderer performance.

1. Read the renderer/runtime ownership documents and `AGENTS.md`; identify which state is authoritative before changing code.
2. Metal renders existing authoritative terminal/runtime state. Do not parse VT, own terminal semantics, reconstruct a second grid, create Blocks state, or introduce another terminal engine in the renderer.
3. Keep the native host layer as small as practical: window/layer/input/platform APIs belong there; portable terminal/runtime logic stays in Rust unless evidence justifies otherwise.
4. Define the frame inputs explicitly: terminal/grid snapshot or stable read view, viewport, damage regions, glyph/font resources, selections/cursor/overlays and presentation metadata.
5. Prefer damage-driven rendering. A paint job must not force full-frame work unless the measured workload or effect requires it.
6. Account for Retina/backing scale, color space, font fallback, grapheme/cell width agreement, glyph atlas lifecycle, clipping, scissoring, cursor/selection rendering, opacity and resize behavior.
7. Make resource ownership and synchronization explicit: buffers, textures, atlases, command buffers, fences/semaphores if any, frame-in-flight count and teardown/recreation paths.
8. Avoid per-cell/per-frame allocations, unnecessary copies, CPU/GPU synchronization, locks, language round trips and synchronous persistence/agent work.
9. Add or update deterministic renderer tests for geometry/state translation where practical and screenshot/visual-regression coverage for externally visible changes.
10. Benchmark representative workloads: idle, typing, fast scroll, resize/reflow, alternate-screen TUI, dense Unicode, high-volume logs and large viewport. Record CPU, GPU/frame time, allocation/RSS and dropped/stalled frame evidence as relevant.
11. Use Metal/Xcode profiling evidence for material renderer changes; document the bottleneck being addressed rather than optimizing by intuition.
12. Validate accessibility/UI semantics separately from the GPU surface using the macOS accessibility/UI skills.

Never introduce a temporary text-view/SwiftUI terminal renderer into the production path.