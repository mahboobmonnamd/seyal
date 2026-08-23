# Seyal — Foundation Architecture R&D Brief

**Status:** Source requirements for the foundation architecture pass  
**Authority:** This file preserves the initiating brief. The canonical decision is [`../SEYAL-ARCH-FOUNDATION-RD-001.md`](../SEYAL-ARCH-FOUNDATION-RD-001.md).

---

## Product objective

Seyal is an open-source, commercial, enterprise-grade, agent-native execution workspace for software development and operations.

It should eventually bring together:

```text
terminal
+ shells
+ SSH
+ persistent sessions
+ multiplexing
+ coding agents
+ operational agents
+ logs
+ approvals
+ artifacts
+ infrastructure workflows
```

into one extremely fast environment where humans, shells, agents and infrastructure operate together.

It is not merely another terminal emulator, an IDE terminal panel, tmux with a GUI, or an AI chat interface attached to a terminal.

---

## Non-negotiable foundations

### Seyal-owned terminal engine

Seyal owns the production terminal stack from the beginning:

```text
PTY / ConPTY bytes
→ VT parser
→ modes/state machine
→ grid
→ alternate screen
→ Unicode/graphemes/width
→ scrollback
→ reflow
→ damage
```

Do not use Ghostty/libghostty, Alacritty, VTE, xterm.js, or another production terminal engine as Seyal's implementation. They may be studied for architecture and tests.

### GPU rendering

The macOS terminal surface uses Metal from day one. There is no temporary NSTextView/SwiftUI/text terminal renderer that will later be replaced.

### Blocks from day one

Blocks are foundational but must not corrupt terminal semantics. A Block must not automatically imply another PTY, terminal parser, grid, process, or transcript copy.

### Real terminal behavior

The architecture must grow incrementally toward correct support for:

```text
zsh / bash / fish
SSH / nested SSH
tmux as child
vim / neovim
Claude Code / Codex / Cursor CLI / OpenCode
watch / htop / ncurses
kubectl / docker / terraform
high-volume logs
long-running processes
```

### Extreme performance and memory efficiency

Architecture must minimize unnecessary:

```text
IPC
process/thread hops
serialization
JSON
copies
allocations
locks
FFI round trips
main-thread blocking
```

The target includes hundreds of panes across workspaces/windows while preserving low idle CPU/RSS and excellent terminal latency.

---

## Research requirements

Study current official/source material from at least:

- Ghostty — reusable terminal/core layering, native integration, C ABI, rendering ownership;
- Warp — Blocks, shell integration, raw/TUI behavior, GPU/product architecture, agent-native evolution;
- Herdr — headless/background runtime, PTY ownership, detach/reattach, persistent sessions, agent state/attention;
- WezTerm;
- Kitty;
- Alacritty;
- iTerm2;
- tmux.

For research findings distinguish:

```text
FACT FROM SOURCE
INFERENCE
RECOMMENDATION FOR SEYAL
```

Do not blindly combine architectures or copy feature lists.

---

## Hybrid architecture question

Seyal is expected to learn from:

```text
Ghostty → reusable high-performance terminal engineering
Warp    → Block-native + agent-native interaction
Herdr   → persistent/headless execution
```

but the architecture must explicitly resolve conflicts such as:

- persistent runtime vs direct local rendering;
- Blocks vs canonical terminal grid;
- runtime-owned VT vs GUI-owned VT;
- remote clients vs low-latency local rendering;
- process isolation vs memory footprint;
- embedding vs native application requirements.

---

## Headless requirement

Seyal must support a headless form without creating a second terminal implementation.

Possible use cases:

```text
server
→ persistent workspace
→ shells
→ coding agents
→ operational workloads
→ remote attach
```

The R&D pass must explicitly decide whether headless execution is implemented in M001 or only architected in M001 and implemented shortly afterward.

---

## Embedding requirement

Seyal must eventually support:

```text
another native application
→ embedded Seyal terminal

Seyal tooling
→ terminal/runtime without full Seyal.app

headless consumer
→ terminal engine without GPU UI
```

The architecture should expose progressively larger reusable capabilities only where they correspond to real ownership boundaries. Avoid crate/package explosion.

---

## Terminal-state authority question

Evaluate at least:

```text
A: PTY → persistent runtime → raw bytes → GUI VT → Metal
B: PTY → persistent runtime VT → display deltas → GUI renderer
C: PTY → runtime VT → GUI VT mirror
D: PTY → canonical state → shared/direct representation → renderer
```

Compare latency, memory, copies, IPC, reconnect, headless operation, embedding, multiple clients, remote access, renderer crash, runtime crash, complexity and correctness.

Choose one authoritative model. Do not accidentally design two VTs/grids.

---

## Runtime/process ownership question

Evaluate:

```text
central runtime with many PTYs
process per execution
runtime task/thread per execution
sharded workers
```

Measure/consider:

- memory per pane;
- context switching;
- crash blast radius;
- PTY lifetime;
- GUI detach;
- runtime failure;
- 50+ panes;
- 10+ active agents;
- headless operation;
- remote clients.

Do not maximize isolation at the cost of memory or minimize processes at the cost of reliability without evidence.

---

## Persistence requirements

Keep separate:

```text
GUI lifetime
PTY lifetime
child-process lifetime
runtime lifetime
terminal-state lifetime
scrollback lifetime
Block lifetime
workspace lifetime
machine lifetime
```

Required direction:

```text
close Seyal.app
→ shell/agent continues

open Seyal.app
→ reconnect

GUI crash
→ execution continues
```

Research runtime-crash survival separately. A disk journal does not resurrect a PTY.

---

## Blocks requirements

Resolve:

- what a Block owns;
- command/output boundaries;
- shell-integration role;
- behavior without shell integration;
- alternate-screen/TUI behavior;
- coding-agent TUI behavior;
- current mutable Block vs completed Blocks;
- raw mode;
- rich/agent Blocks;
- huge-history virtualization;
- memory implications.

A real TUI must remain a real TUI.

---

## Rendering requirements

Decide the production path:

```text
terminal state
→ damage
→ shaping
→ glyph cache/atlas
→ instance data
→ GPU
→ presentation
```

Define ownership for shaping, font fallback, glyph rasterization/cache, emoji, ligatures, wide characters, Retina scaling, frame scheduling and damage tracking.

The renderer must not require serialized cells or JSON.

---

## macOS native-language decision

Evaluate:

```text
Rust + Swift/AppKit/Metal
Rust + Objective-C/AppKit/Metal
Rust + Objective-C++ where required
mixed native host
```

Consider FFI behavior, copies/allocations, AppKit/Metal integration, accessibility, IME, maintainability, SDK evolution, debugging and performance.

---

## Exact hot paths

Document input, output and resize paths with every process/thread/language boundary, queue, lock, allocation, copy and system call category.

Goal:

> minimum practical overhead with maintainable enterprise architecture.

A later amendment further requires avoiding synchronous IPC round-trip/ping-pong on hot terminal paths while preserving the runtime boundary required for persistent execution.

---

## Memory requirements

Define expected incremental costs for:

```text
1 / 10 / 50 / 100+ terminals
1 / 10 active agents
100k / 1M lines scrollback
```

Identify duplication risks for raw bytes, terminal grids, alternate grids, scrollback, Blocks, semantic content, agent transcripts, persistence buffers, protocol data, renderer buffers and GPU resources.

Do not casually allocate megabytes per idle execution.

---

## Concurrency requirements

Define models for PTY reading, VT parsing, GPU rendering, input, persistence, Blocks, agent metadata and background work.

Terminal progress must not wait for:

```text
agents
persistence
analytics
telemetry
network
licensing
semantic processing
Lua/custom scripts
```

---

## Commercial architecture constraint

Seyal must support:

```text
Seyal OSS
Seyal Pro
Seyal Teams
Seyal Enterprise
```

without licensing checks contaminating PTY, VT, grid, renderer, basic Blocks or local execution.

Enterprise value can live in higher-level collaboration, remote/cloud execution, identity/RBAC, policy, audit, secrets governance, administration, compliance, deployment and support.

---

## Security seams

Foundation architecture must leave explicit boundaries for:

- SSH;
- secrets;
- agent permissions;
- environment variables;
- clipboard;
- OSC security;
- filesystem access;
- remote clients;
- enterprise policy;
- local socket authentication.

Do not implement the full enterprise security system during foundation work.

---

## Architecture variants and scoring

Produce at least three credible complete architectures differing in authority/process/render/headless/embed choices.

Evaluate terminal correctness, latency, memory, maintainability, headless support, embedding, persistence, failure isolation, agent scalability, native macOS quality, Linux evolution and implementation complexity.

Choose one architecture.

---

## Milestone requirements

Only after architecture selection define milestones.

M001 must prove a production-shaped vertical slice:

```text
native macOS Seyal
→ Block
→ real PTY
→ real shell
→ Seyal-owned VT
→ authoritative terminal state
→ damage
→ Metal
→ pixels
```

The architecture pass decides whether the Runtime/headless process already owns the PTY in M001.

---

## Engineering gates

Day-1 plan must cover:

- TDD;
- VT unit tests;
- byte fixtures;
- conformance corpus;
- property tests;
- fuzzing;
- PTY integration tests;
- renderer tests;
- golden/reference tests where appropriate;
- latency/throughput benchmarks;
- RSS/idle CPU measurement;
- sanitizer/static-analysis strategy;
- crash/failure tests;
- CI;
- reproducible builds.

---

## Restart-prevention failures to examine

At minimum:

```text
temporary renderer
temporary VT
duplicate terminal state
UI owning runtime semantics
daemon added too early
daemon added too late
process-per-pane without measurement
JSON/display IPC
Blocks retrofitted later
Blocks controlling PTY semantics
semantic transcript becoming canonical
agent processing on hot path
premature cross-platform UI
over-engineered crate graph
incomplete VT presented as production-ready
persistence confused with journaling
```

Later amendments add:

```text
daemon/thread/Lua VM/renderer per pane
GPU resources per hidden execution
slow-client backpressure
implicit multi-client resize/input authority
Lua on hot paths
agents owning PTYs/terminal state/rendering
mobile treated as a second terminal authority
cloud implemented with a different VT model
```

---

## Configuration amendment

The prior RILL configuration ADRs may be consulted as evidence for configuration only.

Seyal should preserve a canonical typed TOML configuration model and support Lua for full customization/automation, while explicitly deciding the boundary so scripts never participate synchronously in terminal hot paths.

---

## Platform/cloud amendment

The architecture must support future:

- macOS desktop;
- Linux desktop/headless;
- Windows desktop/headless using a ConPTY-compatible endpoint adapter;
- iOS remote client;
- Android remote client;
- Seyal Cloud headless execution;
- user-machine remote attach.

Do not build a premature universal GUI abstraction. Portable runtime/protocol/terminal ownership should make those products possible while each UI platform can remain native when developed.

---

## Multi-agent amendment

Architecture must support future first-class multi-agent orchestration and smart routing without requiring agents to own PTYs, terminal state, Blocks' terminal storage or rendering.

Agents should interact through stable execution capabilities, structured events/actions, routing metadata and explicit approvals.

---

## UI/attention amendment

Architecture must cover:

- workspaces;
- windows;
- tabs;
- split panes;
- inspectors;
- notifications;
- history/scroll;
- Block virtualization/skeletons;
- Flow/raw/TUI presentation;
- global structured attention;
- agent approvals directly from an attention/popover stack where protocol-safe;
- navigation to the exact execution when raw terminal interaction is required.

UI should be ambitious and futuristic, but paint/design work must never delay or block terminal hot-path progress.

---

## Required final output of the R&D pass

The decision package should contain:

1. executive decision;
2. research findings;
3. alternatives considered;
4. final architecture diagram;
5. ownership matrix;
6. hot-path diagrams;
7. layer/crate structure;
8. headless decision;
9. embed decision;
10. macOS native-language decision;
11. Blocks architecture;
12. persistence architecture;
13. performance/memory budgets;
14. security boundaries;
15. architecture invariants;
16. milestone roadmap;
17. M001 definition/acceptance criteria;
18. risks;
19. restart-prevention checklist;
20. final READY/NOT READY verdict.

The architecture package and companion rationale now fulfill this brief; implementation must not start outside the accepted milestone sequence merely to get something visible quickly.
