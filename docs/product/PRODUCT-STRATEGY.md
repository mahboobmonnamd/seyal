# Seyal Product Strategy — Terminal-First Agentic Development Environment

**Status:** Product strategy authority  
**Scope:** Product positioning and scope-selection rules. `FEATURES.md` remains the canonical capability/disposition registry; architecture/ADR/spec/milestone documents remain implementation authority.

## 1. Product definition

Seyal is a **terminal-first Agentic Development Environment (ADE)** built on a persistent execution-workspace architecture.

The terminal is not an embedded utility inside the ADE. It remains a first-class, standalone-quality terminal whose correctness, latency, memory behavior and TUI compatibility are protected independently from editor, AI, Git, preview, indexing, persistence and cloud features.

A user who disables AI and never opens an editor or preview must still receive an exceptional terminal with no synchronous ADE overhead on the terminal hot path.

```text
human + shell + agent + repository + infrastructure
                         │
                         ▼
                       Seyal
                         │
       ┌─────────────────┼─────────────────┐
       │                 │                 │
 terminal plane     development plane    agent plane
       │                 │                 │
 PTY / VT / state     editor / files     providers
 scrollback / TUI     Git / diff         local models
 Blocks / damage      preview             orchestration
 renderer             artifacts          approvals
```

## 2. Competitive strategy: selective superset, not feature-list copying

Seyal must not become "another product plus more features". Competitive products are evidence for useful workflows, not architecture or roadmap authority.

Use four buckets when evaluating a competitor capability:

### Match or exceed

Capabilities that materially improve terminal-first agentic development should reach parity and eventually exceed strong competitors where they are useful:

- BYOK provider/model selection;
- fully local model providers;
- plans, tasks, sub-agents and custom agents;
- external coding-agent orchestration;
- prompt queues/follow-ups;
- scoped tool permissions and approvals;
- context attachments;
- AI edit/diff review;
- code autocomplete;
- lightweight code editing;
- project/file navigation;
- focused Git/source-control workflows;
- local dev-server preview;
- workspace/project memory and rules/skills;
- secure secret/key handling.

### Adopt but reshape for Seyal

Useful ideas may be adopted with a different ownership model:

- a built-in editor is a **focused development surface**, not product authority;
- live preview is a sandboxed cold surface, not a browser platform;
- source control is explicit typed Git/SCM actions off the terminal hot path;
- quick AI interaction uses a compact transient popover, not a permanently space-consuming side chat;
- agent context comes from typed resources/selections/provenance rather than arbitrary screen scraping;
- editor/preview surfaces may use appropriate system webview technology without moving terminal rendering or terminal state into a webview.

### Intentionally filter

Do not add capabilities merely because an IDE or competitor has them. Seyal should avoid becoming a general IDE unless a future explicit product decision changes this boundary.

Examples intentionally outside the current core direction:

- heavyweight debugger/profiler suites;
- IDE-scale arbitrary extension hosts;
- permanent full-repository semantic/indexing services that consume resources without user value;
- browser DevTools/browser-platform scope;
- notebook/document-workspace expansion unrelated to execution workflows;
- features that require terminal input/output/rendering to wait on AI, LSP, Git, persistence, network or cloud work.

### Go beyond

Seyal's intended moat is deeper execution and orchestration rather than editor feature count:

- persistent/headless execution independent from GUI lifetime;
- provider-neutral WorkItem → Attempt → AgentRun → Execution/Artifact model;
- parallel agent execution with writer/worktree isolation;
- independent evaluator/reviewer agents;
- duplicate-work/conflict detection and reconciliation;
- deterministic/local routing plus future smart routing;
- cost/token/time budgets and explainability;
- typed approvals and global attention;
- remote continuation and multi-client execution;
- DevOps/infrastructure workflows using the same execution substrate;
- team/enterprise collaboration, identity, RBAC, policy, audit and governance above the strong OSS foundation.

## 3. Product priority order

When priorities conflict, use this order:

```text
1. terminal correctness + terminal performance
2. reliable execution/runtime foundations
3. excellent agent execution/orchestration
4. focused developer surfaces
5. DevOps/infrastructure workflows
6. collaboration and enterprise capabilities
```

This ordering does not mean developer surfaces are unimportant. It means they cannot weaken the terminal/runtime foundation to ship faster.

## 4. Terminal-plane invariant

The production terminal path remains:

```text
input
→ platform input handling
→ Runtime / TerminalExecution
→ PTY / child
→ Seyal VT / canonical TerminalState
→ damage / display projection
→ platform GPU renderer
→ pixels
```

Terminal progress must never synchronously wait for:

```text
AI / agent inference
editor / LSP
Git / source control
file indexing
preview/browser
persistence
telemetry
cloud
licensing
collaboration
```

Editor, preview and AI functionality may observe or act on typed terminal/runtime resources asynchronously. They never become PTY, VT/grid, scrollback or renderer authority.

## 5. Focused built-in editor

Seyal should include a lightweight first-class code-editing surface because agentic development repeatedly crosses terminal ↔ file ↔ diff boundaries.

Target capabilities:

- multi-language syntax highlighting;
- normal editing, selections, search and replace;
- Vim mode where practical;
- formatting;
- optional/lazy LSP integration for diagnostics, navigation, completion and formatting;
- inline AI autocomplete, including local-model completion;
- contextual `Ask Seyal` actions from a selection;
- AI edit/diff review;
- file change reload and conflict handling;
- exact path/line navigation from terminal, Git, diagnostics and agents.

The editor is **not** intended to become a VS Code replacement. Debugger/DAP, heavyweight profiler suites, an IDE-scale extension host and permanently resident language infrastructure remain outside the current core scope.

Implementation should prefer a mature bounded editor engine rather than building a text editor from scratch. A system webview-hosted editor is acceptable because it is a cold development surface; the terminal surface itself must not move to that architecture.

## 6. AI provider and local-model strategy

Local use must support both BYOK cloud providers and local/private providers without requiring a Seyal account.

The provider layer should support:

- first-party adapters for major providers where justified;
- generic OpenAI-compatible endpoints;
- local endpoints such as Ollama, LM Studio and MLX-class runtimes where platform support exists;
- provider/model capability discovery;
- separate model choice by workload;
- deterministic user routing rules, budgets and fallbacks;
- secure OS-backed credential storage for secrets.

Example routing:

```text
editor autocomplete → low-latency local/small model
implementation agent → capable coding model
review/evaluation → independent model/agent
private repository → local/private endpoint by policy
```

Local providers are first-class providers, not degraded fallback integrations.

## 7. Development preview and artifact surfaces

Seyal should provide focused preview surfaces for development workflows:

- auto-detected local development servers;
- sandboxed localhost web preview;
- Markdown rendered/raw preview;
- image, PDF and other high-value artifact viewers where useful;
- explicit remote/tunnel security when a preview originates outside the local trust boundary.

Preview detection should use trusted process/port/runtime signals where possible rather than parsing arbitrary terminal text as authority.

Opening or updating a preview must not affect PTY/VT/render progress.

## 8. Source control

Seyal should provide a focused Git/SCM surface that supports the workflows surrounding agents and terminal work:

- status and changed-file inventory;
- diffs and hunk review;
- stage/unstage/discard with explicit safe semantics;
- commit and branch/worktree workflows;
- history/graph where useful;
- PR/MR/check integrations through provider-neutral adapters;
- agent worktree visibility and exact transitions between changes, execution and review.

SCM work is cold/asynchronous relative to the terminal path.

## 9. Quick Agent Popover

Seyal should provide a compact **Quick Agent Popover** for contextual AI interaction without imposing a permanent side-chat layout.

The popover is:

- transient;
- non-modal;
- anchored near the bottom-right of the active window/surface;
- small by default and content-bounded;
- explicitly invoked by the user or opened from a contextual `Ask Seyal` action;
- able to expand into the full Agent surface without losing the underlying AgentRun/conversation;
- positioned so it does not cover the terminal composer, editor insertion point or other critical active controls;
- a presentation of authoritative agent/run state, never the state owner itself.

Detailed behavior is defined in `../architecture/ui/SEYAL-AGENT-QUICK-POPOVER.md`.

This is intentionally different from the global Attention/Notifications popover. Attention handles events/approvals; Quick Agent handles active conversation/task interaction.

## 10. Cross-platform strategy

Cross-platform parity should come primarily from portable Rust product/runtime semantics and narrow platform adapters, not from replacing the terminal with a universal web UI.

```text
shared portable core
├─ Runtime / execution model
├─ VT / terminal semantics
├─ Blocks/history identities
├─ workspace / agent / workflow model
├─ provider/context/router logic
└─ protocols

platform layers
├─ terminal endpoint: PTY / ConPTY
├─ window/input/IME/accessibility
├─ GPU renderer
├─ credential/keychain integration
└─ system webview/editor/preview integration where appropriate
```

Functional parity is required; implementation details may differ by platform. macOS remains native-first while another platform is not actively being built.

## 11. Distribution/resource strategy

Seyal does not need to compete for the smallest possible binary at the expense of architecture, but uncontrolled application growth is unacceptable.

Rules:

- do not bundle local-model weights with the base application;
- prefer system webviews over shipping an entire private Chromium runtime when they satisfy requirements;
- lazy-load editor, LSP, preview, indexing and AI surfaces;
- hidden/inactive surfaces release expensive renderer/runtime resources where correctness allows;
- track install size, cold start, idle RSS, per-visible-pane RSS and background CPU as release metrics;
- a move from a compact native application toward hundreds-of-megabytes of avoidable runtime/framework payload requires explicit architectural justification.

Install size is secondary to correctness and latency, but it remains an engineering budget rather than an ignored outcome.

## 12. Performance-claim rule

Do not claim Seyal is faster than another terminal/ADE from architecture alone.

Competitive performance claims require same-hardware measurement of at least:

- key-to-photon p50/p95/p99;
- sustained PTY output throughput;
- scrolling/reflow under large history;
- TUI repaint/resize behavior;
- 1/10/50/100+ execution idle RSS and CPU;
- visible-pane rendering cost;
- cold/warm startup where relevant.

The architecture is designed to make Seyal extremely fast. Measurement makes that a product fact.

## 13. Decision test for new features

Before accepting a new capability, ask:

> Does this materially improve terminal-first agentic development, execution, operations or collaboration without compromising terminal correctness, latency, memory discipline or architectural simplicity?

If yes, evaluate it through the normal product/R&D process.

If its main justification is "another product has it," do not add it automatically.
