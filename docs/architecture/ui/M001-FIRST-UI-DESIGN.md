# M001 First UI Design — Composer, Adaptive Blocks, Live TUI

**Status:** Design refinement for M001  
**Issue:** #73  
**Authority:** Subordinate to `SEYAL-UI-ARCHITECTURE-001.md`, foundation architecture, accepted ADRs, and `MILESTONE-001.md`  
**Source visual:** user-provided 1586×992 RILL concept screenshot; visual mood/reference only, not implementation authority  
**Approved visual references:** [`references/README.md`](references/README.md) and its checked-in UI reference images

## 1. Goal

Define the smallest first visible Seyal UI without skipping the production architecture sequence.

The first surface is intentionally narrow:

```text
Window
└─ ExecutionViewport
   ├─ Flow/Raw presentation
   │  └─ adaptive Blocks
   └─ CommandComposer (bottom-fixed when explicitly eligible)

Live TUI state
└─ TerminalSurface takes over the execution viewport
   └─ same ExecutionId / PTY / VT / alternate screen
```

No sidebar, inspector, tabs, split management, agent panels, attention stack, or rich Block actions are required for this slice.

This is an **MVP design for the permanent production path**, not a POC design. If the owning Runtime/projection/renderer/input/Block dependencies are not Ready, implementation waits rather than introducing fake UI, a temporary renderer, or a parallel terminal model.

## 2. Competitive reference — Warp architecture and the Seyal boundary

Warp is an important reference because its modern terminal UX is structurally built around command/output Blocks, a separate input editor, GPU rendering, and full-screen alt-grid application handling. Its 2026 open-source architecture also shows that a typed Block list can support terminal blocks and richer agent/content blocks in one scroll stream.

Useful Warp ideas to study:

- commands and output form navigable units;
- the command editor is a first-class interaction surface rather than merely painting a shell prompt;
- full-screen/alt-grid applications receive a dedicated terminal presentation;
- large histories need height indexing/virtualization rather than eagerly materializing everything;
- Block metadata enables navigation/actions without requiring the user to interpret a raw byte stream.

Seyal must **not** copy Warp's terminal-state architecture or visual identity. In particular, Warp terminal Blocks store command/output grid state as Block-owned terminal content. Seyal's accepted architecture instead keeps one authoritative `TerminalState` and logical history per `TerminalExecution`; Blocks are metadata/presentation over that authority and never own copied terminal grids/output.

### 2.1 Architecture comparison

| Concern | Warp reference | Seyal M001 direction |
|---|---|---|
| Terminal history organization | typed `BlockList`; terminal Blocks contain command/output grid state | one authoritative terminal state/history; `BlockTimeline` references `ExecutionId` + logical anchors |
| Input | first-class editor; configurable position | bottom composer only when reliable structured command-entry eligibility is known; direct terminal input is the safe/default path whenever state is unknown or shell semantics matter |
| Large command output | continuous Block list with navigation/sticky command affordances | each Block grows intrinsically only to configured max height, then its output scrolls internally with explicit parent-scroll chaining |
| TUI/alt screen | full-screen app presentation | same concept, but explicitly the same `ExecutionId`/PTY/VT/alternate grid with Block/composer chrome yielding |
| Error styling | can color the Block strongly by exit state | use restrained local status text/icon/border treatment from real metadata; no full-Block error fill |
| Rich agent content | rich content can live in the same Block list | deferred from M001; future agent/artifact/attention views reference runtime identities without redefining terminal history |
| UI stack | Rust custom WarpUI/GPU framework | native Swift/AppKit host + Seyal Metal renderer; portable terminal/runtime authority remains Rust |

### 2.2 What Seyal intentionally changes

The first Seyal UI should not look like "Warp with different colors".

Seyal uses **lightweight execution Blocks** over one canonical terminal history:

```text
┌─ command ───────────────────────────────────────────────────┐
│ output...                                                   │
│ output...                                                   │
└─────────────────────────────────────────────────────────────┘

┌─ next command ──────────────────────────────────────────────┐
│ output...                                                   │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│ > command composer (only when eligible)                run  │
└─────────────────────────────────────────────────────────────┘
```

The exact glyphs are illustrative only. The design intent is:

- Blocks use lightweight boundaries/surfaces only where they improve command/output grouping, navigation, actions or focus;
- there is **no persistent execution rail/gutter** and no decorative left timestamp rail;
- Block identity, focus and status use compact local chrome such as command/header metadata, border/surface treatment or a small status indicator when real metadata exists;
- command/output content remains visually primary; Block chrome stays subordinate and must earn its space;
- semantic failure/success uses compact status text/icon/border treatment, not a large red/green Block background;
- when eligible, the bottom composer feels like a **command dock integrated into the execution surface**, not a detached IDE panel;
- when composer eligibility is unknown or unsafe, direct terminal input wins and the dock must not intercept shell/editor semantics;
- when a TUI takes over, the dock retracts and the terminal surface expands cleanly; there is no nested "TUI inside a Block" effect;
- long Block output is locally capped/scrollable as requested, so one noisy command cannot monopolize the whole workspace.

This differentiation is visual and behavioral. It does not create a second terminal engine or a competing state model.

### 2.3 Warp reference sources

Reviewed as competitive evidence, not Seyal authority:

- https://docs.warp.dev/terminal/blocks/block-basics
- https://docs.warp.dev/terminal/appearance/input-position
- https://docs.warp.dev/terminal/more-features/full-screen-apps
- https://www.warp.dev/blog/block-model-behind-warps-agentic-development-environment
- https://github.com/warpdotdev/warp/blob/master/AGENTS.md

## 3. Visual direction

The provided screenshot establishes mood, not pixel geometry. Warp establishes useful product patterns, not visual authority. The checked-in approved references in `references/` establish the current component-level visual direction.

Use:

- both light and dark appearances as first-class product surfaces;
- a calm, low-noise workspace background derived from the active Seyal appearance/theme;
- lightweight Block surfaces/boundaries that make command/output grouping clear without turning the workspace into a stack of heavy cards;
- restrained translucent/material-like surfaces only where they do not hurt render cost;
- thin separators and subtle depth where they improve hierarchy;
- one accent/focus treatment at a time;
- dense terminal information with generous structural spacing;
- minimal chrome around commands and output;
- typography hierarchy driven primarily by terminal text, command identity, and state.

Do not copy the screenshot's left sidebar, right inspector, tabs, agent controls, notification stack, or multi-pane composition into the first UI simply because they appear in a reference. Those broader workspace features land only in their owning milestones.

Do not copy Warp's distinctive heavy Block-card treatment, sticky-command presentation, strong whole-Block error backgrounds, configurable top/reverse input layouts, or agent-rich Block stream into M001.

Futuristic means calm, spatially clear, fast, and context-aware. It does not mean animation-heavy or decorative rendering.

## 4. First-window component hierarchy

```text
SeyalWindow
└─ ExecutionContainerView
   ├─ ExecutionViewportView
   │  ├─ FlowPresentationView
   │  │  └─ BlockViewportView
   │  │     └─ BlockView × N visible/near-visible
   │  ├─ RawTerminalPresentationView
   │  └─ LiveTUIPresentationView
   └─ CommandComposerView (conditional presentation)
```

Only one of `FlowPresentationView`, `RawTerminalPresentationView`, or `LiveTUIPresentationView` is active for the same `ExecutionId`.

`BlockView` is a presentation region over canonical terminal history. It must not become a terminal grid owner merely because it has independent layout/scroll state.

The AppKit hierarchy may differ in naming, but the ownership model must not.

## 5. Command composer

### 5.1 Position and eligibility

When the Runtime/UI has reliable evidence that the execution is in a supported structured command-entry state, the first MVP command composer is fixed to the bottom of the execution container.

The composer is **not** the default simply because the execution happens to be a shell. Ordinary zsh/bash/fish/readline interaction may depend on character-by-character terminal behavior such as tab completion, Ctrl-R, shell widgets/plugins, vi/emacs modes, autosuggestions, multiline editing, history navigation and shell-specific key bindings. Seyal must not intercept those semantics unless an explicit shell-integration/input contract proves that doing so is safe.

Eligibility is conservative:

```text
reliable supported shell integration
+ confirmed prompt / line-oriented command-entry state
+ no active secret/password/auth prompt
+ no raw/cbreak interaction
+ no REPL/interactive child prompt requiring terminal editing
+ no alternate-screen/TUI state
→ composer may be shown and focused

anything unknown, unsupported or ambiguous
→ direct terminal input
```

**Unknown always falls back to the real terminal surface.** Seyal never guesses that the composer is safe.

Unlike Warp's configurable input-position model, top/reverse/classic positions are intentionally not part of M001. A single predictable eligible layout reduces interaction and geometry complexity while the terminal/runtime foundation is still being proven.

The composer must not float over terminal output in a way that changes terminal geometry invisibly. When it is present, the viewport dimensions above it are the dimensions sent through the canonical resize path. When it retracts, the reclaimed area is likewise applied through the canonical resize transaction.

### 5.2 Input model

The composer is an AppKit-native text input surface and must support:

- keyboard focus;
- IME/composition;
- copy/paste;
- selection;
- command history integration later;
- accessibility role/name/value;
- submit action;
- cancellation/clear behavior.

The composer is **not shell or terminal authority**. It does not parse VT state, own the shell's line editor, or encode terminal-mode-sensitive key sequences itself.

It also must not absorb shell-editor commands such as Tab completion, Ctrl-R, shell widgets, application key modes or interactive prompt keystrokes merely because those keys are valid AppKit editing input. If Seyal cannot prove equivalent semantics through supported shell integration, focus/input remains on the terminal surface.

### 5.3 Submission

When composer eligibility is established, initial behavior is:

```text
user edits one structured command in composer
→ submit
→ Runtime writes the corresponding bytes/intent to the same TerminalExecution PTY
→ command output mutates canonical TerminalState
→ Block metadata may bracket the execution asynchronously
```

No extra PTY or shell process is created for a Block.

Reliable shell integration may later enrich boundaries, cwd, exit status, prompt context, command history and composer eligibility. The base terminal must remain correct without it.

### 5.4 Direct-input fallback

The UI must preserve a direct terminal-input path for every state in which a line-oriented composer is insufficient or unproven, including:

- normal shell editing when supported shell-integration eligibility is unavailable;
- tab completion, Ctrl-R, shell widgets/plugins, vi/emacs modes and shell-specific editing semantics that are not explicitly integrated;
- applications using canonical terminal modes that require individual key events;
- interactive prompts where structured command boundaries are unknown;
- password/authentication/secret prompts;
- REPLs and nested interactive programs;
- raw/cbreak-mode debugging;
- unsupported shell integration;
- any state where the UI cannot prove that composer semantics are safe.

This fallback is a presentation/input routing change over the same execution, not a second terminal view or PTY. Switching to direct input must not recreate the shell, command, Block, PTY or `TerminalState`.

### 5.5 TUI interaction

When the execution enters a live TUI/alternate-screen presentation:

- the composer retracts from layout or is fully removed from interaction;
- the terminal surface receives the reclaimed vertical area through the canonical resize transaction;
- keyboard/mouse/focus go directly to the terminal surface;
- no command field consumes arrow keys, control sequences, function keys, mouse reports, or text composition intended for the TUI;
- returning from the TUI restores the normal presentation without recreating the execution;
- composer visibility after return is re-evaluated from current eligibility rather than blindly restored from stale pre-TUI UI state.

## 6. Adaptive Blocks

### 6.1 Height rule

A Block has **intrinsic minimum height only**. There is no arbitrary fixed minimum card height.

Conceptually:

```text
blockHeight = min(intrinsicContentHeight, configuredMaxBlockHeight)
```

If the content is shorter than the maximum, the Block ends immediately after its content and required insets.

If the content exceeds the maximum, the output region becomes internally scrollable.

This deliberately differs from a continuous unbounded large-Block presentation: one command's output cannot permanently dominate the workspace viewport.

### 6.2 Recommended initial visual structure

```text
BlockView
├─ BlockChrome (lightweight, derived metadata only)
│  ├─ command/context identity
│  ├─ optional status
│  └─ optional actions
├─ CommandLine
│  ├─ prompt/context seam
│  └─ command text
└─ OutputRegion
   └─ terminal-derived content
```

Optional execution state such as running/completed/failed may be represented minimally when real metadata exists. Do not synthesize status from scraped raw text.

There is no required persistent execution rail or timestamp gutter. Block chrome is application presentation only; terminal colors/content remain inside terminal-derived output.

### 6.3 Overflow and nested-scroll behavior

The maximum Block height is a presentation preference, not terminal state.

Rules:

- short output: no internal scrollbar;
- long output: clip to max height + internal vertical scroll;
- internal scrolling is visually subordinate; prefer a thin overlay/edge indicator over a heavy permanent scroll gutter where native behavior permits;
- a capped Block must visibly indicate that additional output exists;
- trackpad/wheel scrolling over a scrollable Block consumes motion only while that Block can continue scrolling in the gesture direction;
- when the Block reaches its top/bottom boundary, remaining gesture motion chains naturally to the parent execution stream instead of trapping the user in a nested scroll container;
- scrolling outside a scrollable Block always moves the parent execution stream;
- keyboard Block-scroll commands apply only when application focus is explicitly on that Block's output viewport; terminal input keys are never stolen from the active terminal surface;
- focus movement into/out of a Block viewport must be keyboard and VoiceOver accessible;
- a discoverable `Expand/Open full output` presentation action must exist by the time capped Blocks ship; it may temporarily expand/open the same canonical history range but must not create copied terminal state;
- scrolling a Block never moves terminal cursor state or changes PTY dimensions;
- resizing the application may recompute presentation height without changing Block identity;
- Block content is a view over canonical logical history/projection, not copied terminal output;
- future virtualization may unmaterialize off-screen Block views while preserving `BlockId` and logical anchors.

### 6.4 Configuration seam

The first design should expose a typed value such as:

```text
PresentationConfig.block.max_height
```

The exact unit is an implementation decision for the owning configuration design. Pixels/points or viewport-relative forms may be evaluated later.

## 7. Flow, Raw, and Live TUI

### 7.1 Flow

Flow is preferred when reliable Block boundaries exist.

```text
same ExecutionId
same TerminalState/history
→ structured execution-stream presentation
```

Flow does not mean each Block owns a terminal surface. Block boundaries are layout/navigation metadata over canonical history.

Composer eligibility is independent from Flow eligibility: structured Block boundaries alone do not prove that intercepting shell line editing is safe.

### 7.2 Raw

Raw is the safe fallback whenever structured command boundaries are absent, ambiguous, disabled, or unsupported.

Raw must always remain usable without shell integration. In Raw mode, direct terminal input is the default; a composer may appear only if a separate reliable eligibility signal explicitly proves safe structured command entry.

### 7.3 Live TUI

Live TUI is a presentation takeover, not another session.

```text
canonical terminal state enters alternate/TUI condition
→ Flow/Raw chrome yields
→ composer retracts
→ terminal surface expands to the execution viewport
→ direct input/focus/resize semantics
→ alternate screen exits
→ previous normal presentation resumes
```

No Block wrapper may constrain or internally scroll the live TUI terminal surface.

A minimal mode indicator may exist only if it does not steal terminal space/input or visually resemble a nested card; it is not required for M001.

## 8. State and ownership mapping

| Concern | Authority | UI responsibility |
|---|---|---|
| PTY | `TerminalExecution` / `seyal-exec` | reference only |
| VT/parser/modes | Runtime `TerminalState` | consume derived projection |
| primary/alternate screen | Runtime `TerminalState` | choose presentation from canonical state |
| terminal dimensions | Runtime authority after native proposal | calculate available viewport geometry and request resize |
| Block identity/anchors | Runtime/workspace metadata | present and virtualize |
| Block maximum height | presentation configuration | apply layout constraint |
| Block local scroll offset | UI presentation state | viewport-only; never terminal/history authority |
| composer eligibility | Runtime/shell-integration evidence + explicit UI routing state | show composer only when proven safe; unknown means direct terminal input |
| command text editing | AppKit composer when eligible | native text entry only; never shell line-editor authority |
| terminal-mode key encoding | Runtime | never guessed by composer/view |
| terminal drawing | Metal renderer | host surface and overlays only |

## 9. Configuration architecture: TOML + Lua compatible

The UI must be configurable without making configuration part of the terminal hot path.

Use a typed immutable configuration pipeline:

```text
TOML defaults/user config ─┐
                          ├─> ConfigBuilder / validation
optional Lua overrides ───┘
                                ↓
                     immutable ConfigSnapshot
                                ↓
             Runtime/UI receive changed typed values
```

Rules:

1. TOML remains appropriate for declarative settings, themes, font choices, dimensions, and normal preferences.
2. Lua may later provide dynamic composition/automation, but is optional and additive.
3. Lua must not run synchronously for PTY reads, VT mutation, damage publication, renderer frame preparation, or terminal key input.
4. Parsed configuration becomes typed values before entering production subsystems.
5. Config reloads publish coarse changed snapshots/events rather than per-cell/per-frame callbacks.
6. A future Lua runtime requires a separate security/capability decision before production enablement.
7. Missing/broken Lua must never break the base terminal; TOML-only operation remains valid.

This Issue preserves the seam only. M001 still defers the production configuration system and production Lua runtime.

## 10. Native macOS behavior

Use the repository's native UI skills for implementation.

Required behavior:

- AppKit window/view lifecycle;
- predictable first responder transitions between composer, Block scroll viewport and terminal surface;
- standard macOS text editing behavior in the composer only while composer mode is eligible;
- keyboard and mouse parity for visible controls;
- VoiceOver representation for composer and Block metadata/overflow state;
- explicit accessibility representation for the Metal terminal surface;
- Retina/backing-scale correctness;
- reduced-motion compliance for any later transitions;
- no SwiftUI/NSTextView terminal renderer.

The native host may use normal AppKit controls for application chrome/composer, but terminal pixels remain on the production Metal renderer.

## 11. Performance rules

Terminal priority remains:

```text
P0 focused terminal input/cursor + fresh damage
P1 visible terminal content
P2 composer/Block interaction
P3 non-terminal metadata
P4 visual polish/animation
```

Forbidden synchronous work on terminal hot paths:

- Lua execution;
- TOML/config parsing;
- semantic Block extraction;
- persistence;
- rich content measurement;
- agent work;
- JSON;
- synchronous UI acknowledgements.

Block height calculation should use already-derived layout/history information and be bounded to visible/near-visible Blocks.

Block chrome must remain O(visible/near-visible Blocks), not O(total history), during normal paint/layout.

Composer eligibility updates must be coarse state changes derived from already available Runtime/shell-integration evidence; terminal keystrokes must not synchronously invoke semantic classification, agents or shell-output scraping to decide where input goes.

## 12. Initial visual tokens

These are design intentions, not frozen numeric API values.

- window background: theme-derived neutral appropriate to the selected light/dark appearance;
- execution canvas: calm and low-noise, with only slight tonal separation where needed;
- Block surface: lightweight border/surface treatment; no persistent execution rail or timestamp gutter;
- composer surface: visually distinct from output but integrated with the pane rather than a bright card when present;
- separators: 1-pixel/backing-aware hairlines where appropriate;
- corner treatment: restrained radius for Blocks/composer/utility chrome; avoid oversized card treatment;
- focus: one clear accent border/outline/surface treatment, subtle enough not to compete with terminal colors;
- status colors: reserve semantic red/amber/green for real metadata and small indicators only;
- typography: terminal monospaced font dominates; application labels use native legible hierarchy;
- motion: short spatial transitions only when they explain composer/TUI/direct-input takeover/return.

## 13. Visual anti-clone acceptance

A first-glance comparison with Warp must show Seyal's own visual model.

M001 should satisfy all of these:

- no stack of visually heavy/prominent rounded command cards;
- lightweight Block boundaries/surfaces may be used when they improve grouping, focus, actions or navigation;
- no persistent execution rail/gutter or decorative left timestamp rail;
- no full-Block red/green status backgrounds;
- no Warp-like sticky command header as a default visual primitive;
- no top/reverse input-position modes in the first MVP;
- no agent conversation/rich-content Blocks in the first terminal stream;
- when eligible, the bottom composer is visually integrated as a command dock; when unsafe/unknown, it yields to direct terminal input rather than imitating shell editing;
- capped long Blocks with local scrolling and parent-scroll chaining are a first-class Seyal behavior;
- TUI takeover visibly removes/retracts normal Flow chrome and makes the full pane the live TUI rather than appearing inside a Block.

These are M001 visual guardrails, not claims that Seyal can never add different navigation or richer views later.

## 14. Visual states to capture later

Controlled native screenshot matrix:

1. eligible structured command-entry state with focused composer;
2. direct-input shell state with composer absent/non-interactive;
3. one short command + one-line output Block with lightweight boundary/surface treatment;
4. multiline Block below max height;
5. long Block at max height with local overflow/scroll indicator visible;
6. expanded/open-full-output presentation for that same Block history range;
7. failed command with restrained status treatment but no full-card error fill;
8. raw terminal fallback;
9. live TUI takeover with composer absent/non-interactive and the TUI occupying the full pane;
10. return from TUI with composer eligibility re-evaluated;
11. window resize at minimum and representative large size;
12. high-contrast/accessibility appearance where applicable.

The provided RILL screenshot and Warp screenshots are not pixel-diff targets for these states. The checked-in approved references in `references/` are the current visual input for component implementation, subject to the architecture and milestone authority above them.

## 15. Functional acceptance for later implementation

The first production UI is acceptable only when all of these are true:

- real command submission reaches a real `TerminalExecution`;
- output is rendered from Seyal's derived projection through the Metal path;
- Block layout does not create a second VT/grid or output copy;
- short Blocks collapse naturally to content height;
- long Blocks scroll internally at configured maximum height;
- nested Block scrolling chains to the parent stream at directional boundaries and does not trap trackpad/wheel gestures;
- capped output has discoverable overflow state and a way to expand/open the same canonical history range;
- composer appears only in explicitly eligible structured command-entry state;
- unsupported/unknown/ambiguous shell, prompt, REPL, secret, raw and TUI states use direct terminal input;
- Tab/Ctrl-R/shell widgets/application key semantics are not stolen by the composer without an explicit supported integration contract;
- direct terminal-input fallback does not recreate the execution or terminal state;
- live alternate-screen/TUI interaction receives full terminal input/focus/resize semantics;
- returning from TUI restores the same execution and re-evaluates composer eligibility;
- raw fallback works when Block metadata is unavailable;
- no configuration/Lua/composer-eligibility semantic work is synchronous on terminal hot paths;
- keyboard, accessibility, resize, screenshot and performance tests are green;
- visual anti-clone acceptance in section 13 is satisfied.

## 16. Dependency-safe implementation sequence

This design does not change M001 pass ordering:

```text
Pass 4 Runtime
→ Pass 5 local attach/projection
→ Pass 6 Metal renderer
→ Pass 7 native input + resize
→ Pass 8 minimal Block + logical anchor
```

UI implementation should be sliced into the owning pass rather than introduced as a temporary disconnected mock terminal.

Safe work before those passes complete:

- visual design refinement;
- deterministic layout/token specification;
- composer eligibility contract refinement tied to future shell-integration evidence;
- nested-scroll/accessibility behavior design;
- screenshot state definitions;
- test-plan refinement;
- competitive architecture review.

Unsafe early work:

- fake command/output cards backed by AppKit strings;
- a composer that intercepts ordinary shell editing without proven eligibility;
- a temporary text terminal renderer;
- a second GUI VT/grid;
- a separate PTY for each Block;
- fake TUI rendering independent of canonical alternate-screen state;
- production Lua execution before its security/performance design;
- a Warp-style Block grid model added alongside Seyal's canonical terminal history;
- any alternate old/new UI engine intended to coexist in mergeable production code.
