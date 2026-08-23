# Seyal Product

Seyal is an open-source, commercial, enterprise-grade, agent-native terminal workspace for software development and operations.

The authoritative product and engineering principles are the **Seyal Project Instructions / Product & Engineering Constitution**. This file does not duplicate that constitution; it defines the product slice exercised by Milestone 001.

## Milestone-001 product objective

Prove the smallest production-shaped Seyal execution path on macOS:

```text
native Seyal app
→ one Block
→ one real PTY
→ one real shell
→ Seyal-owned VT parser/state
→ authoritative terminal grid
→ damage tracking
→ Metal renderer
→ pixels
```

Keyboard input follows the reverse execution path:

```text
native macOS keyboard event
→ minimal native/Rust boundary
→ Rust terminal runtime
→ PTY
→ shell/application
```

Milestone 001 is not a disposable prototype terminal engine. The PTY, VT state model, grid, damage model, and renderer boundary established here are intended to survive and expand in later milestones.

## Product constraints for this milestone

- Terminal correctness takes priority over visual polish.
- Rust owns portable terminal/runtime behavior.
- Native macOS code owns AppKit, Metal, native input, font rasterization, and platform APIs.
- One Block is presentation around one terminal execution; it is not another PTY, process, session, or transcript.
- Terminal I/O must not synchronously depend on Blocks, persistence, agents, cloud, telemetry, licensing, or semantic processing.
- No alternate terminal engine is permitted.
- No temporary text-view renderer is permitted.
- No JSON is permitted in the terminal hot path.

Commercial tiers, collaboration, persistence, agents, tabs, splits, cloud, accounts, plugins, and enterprise administration are explicitly outside Milestone 001.
