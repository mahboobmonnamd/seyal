# Seyal Product

Seyal is an open-source, commercial, enterprise-grade, agent-native terminal workspace for software development and operations.

The authoritative product and engineering principles are the **Seyal Project Instructions / Product & Engineering Constitution**. This file does not duplicate that constitution; it defines the product slice exercised by Milestone 001.

## Milestone-001 product objective

Prove the smallest production-shaped Seyal execution path on macOS:

```text
headless Seyal Runtime
→ TerminalExecution
→ one real PTY
→ one real shell
→ Seyal-owned VT parser/state
→ authoritative TerminalState
→ damage
→ derived local display projection
→ native Seyal app
→ Metal renderer
→ pixels
```

Keyboard input follows the reverse execution path:

```text
native macOS keyboard event
→ native input normalization
→ one-way/bounded runtime input path
→ canonical terminal mode handling
→ PTY
→ shell/application
```

Milestone 001 is not a disposable prototype terminal engine. The Runtime ownership, PTY, VT state model, grid, damage model, display-projection boundary, and Metal renderer established here are intended to survive and expand in later milestones.

## Persistence proof required in M001

Because the per-user Seyal Runtime owns the `TerminalExecution` from day one, M001 includes only the minimum persistence contract needed to prove that ownership:

```text
launch Seyal.app
→ terminal execution exists

close or crash Seyal.app
→ Runtime + PTY + shell continue

reopen Seyal.app
→ attach to the same TerminalExecution
```

M001 does **not** claim Runtime-crash live-PTY survival, reboot recovery, durable terminal history, cloud persistence, or production layout/history persistence.

## Product constraints for this milestone

- Terminal correctness takes priority over visual polish.
- Rust owns portable terminal/runtime behavior.
- Native macOS code owns AppKit, Metal, native input, font rasterization, and platform APIs.
- One Block is presentation/metadata around one terminal execution; it is not another PTY, process, session, VT, grid, or transcript.
- Terminal I/O must not synchronously depend on Blocks, persistence, agents, cloud, telemetry, licensing, or semantic processing.
- No alternate terminal engine is permitted.
- No temporary text-view renderer is permitted.
- No JSON is permitted in the terminal hot path.
- The GUI never owns or mirrors another authoritative terminal state.

Commercial tiers, collaboration, durable persistence, agents, tabs, splits, workspaces, cloud, accounts, plugins, and enterprise administration are explicitly outside Milestone 001.