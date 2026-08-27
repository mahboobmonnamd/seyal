# SPEC-005 — M001 permanent Metal renderer

- **Status:** Accepted for M001 Pass 6 via PR #657
- **Date:** 2026-08-27
- **Issue:** #658
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
