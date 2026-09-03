# Seyal Adaptive Depth Design Language

**Status:** Proposed universal visual-design authority  
**Scope:** Seyal OSS UI container and all first-party Seyal product surfaces  
**Applies to:** light/dark themes, single-pane, multipane, Blocks, composer, Workspaces, Agents, Inspector, attention, overlays, and future platform hosts

## 1. Intent

Seyal should feel lighter than a traditional IDE without becoming visually sparse, decorative, or gamer-oriented.

The design system is built for terminal power users: long sessions, dense information, keyboard-first operation, strong focus, predictable geometry, low visual fatigue, and immediate distinction between terminal truth and surrounding product context.

The universal design language is **Seyal Adaptive Depth**.

> Material exists only in proportion to operational relevance.

Seyal is not permanently flat, permanently glass, or permanently elevated. The terminal content remains optically stable while surrounding utility surfaces gain or lose restrained material depth according to focus, relevance, and attention.

The recognizable Seyal identity is the combination of:

- **Zero-Chrome at Rest** — structure disappears until it carries meaning;
- **Adaptive Depth** — utility material appears in proportion to relevance;
- **Semantic Seams** — thin boundaries communicate execution/context/state instead of decorative cards;
- **Focus Gravity** — the active object gains perceptual presence while surrounding chrome recedes;
- **Terminal Truth** — terminal and TUI content remain crisp and absolute.

This document defines presentation only. It does not change PTY, VT, terminal-state, Block, Pane, execution, or renderer authority.

## 2. Universal composition

The existing information architecture remains unchanged:

```text
[UI container]
[left context/navigation] [terminal workspace / Pane tree] [right contextual Inspector]
                                      |
                               Pane-scoped composer
```

Terminology: use **UI container** for application-level framing/chrome. Avoid calling it the "shell" because shell already has terminal/runtime meaning.

Adaptive Depth changes how these regions are presented, not where they live.

## 3. Visual weighting

Use the following as a design budget, not literal screen-area percentages:

- **75% Minimal / Flat:** terminal canvas, Block transcript, normal lists, tabs, code/output, split content.
- **15% Frosted utility material:** left context/navigation, right Inspector, Pane composer, popovers and transient utilities.
- **10% colour / state / depth:** focus, selection, execution state, agent state, approvals, warnings, errors and attention.

The previous 70/20/10 exploration is refined to 75/15/10 to keep the product terminal-first and reduce persistent decorative material.

## 4. Zero-Chrome at Rest

Zero-Chrome does not mean removing functionality or hiding navigation unpredictably. It means the resting presentation should not draw boxes around every object.

Rules:

- terminal/work content owns the strongest surface;
- persistent chrome uses alignment, typography and low-contrast seams before borders/backgrounds;
- action affordances appear on focus, hover, keyboard invocation or attention rather than being permanently repeated;
- a region should not receive its own visible container merely because it exists in the object hierarchy;
- empty space remains empty instead of being filled with decorative cards;
- when labels/branding are removed, Seyal should still be recognizable through continuous work surfaces, semantic seams and context emerging only when operationally relevant.

## 5. Depth model

Adaptive Depth uses four semantic depth levels.

### D0 — Truth

For information whose clarity must never depend on decorative material:

- terminal grid and transcript;
- Block output;
- TUI/alternate-screen content;
- code/diff text where readability is primary.

Properties:

- opaque or effectively opaque content surface;
- no blur behind terminal text;
- no decorative shadow;
- no animated opacity changes during terminal rendering;
- highest text clarity.

### D1 — Receded utility

For persistent but currently secondary UI:

- unfocused left context/navigation;
- unfocused Inspector;
- inactive tab/layout chrome;
- inactive Pane framing.

Properties:

- very low visual contrast against the UI container;
- optional restrained translucency/frost;
- typography and alignment carry hierarchy;
- separators are preferred over cards.

### D2 — Active utility

For the utility surface the user is currently using:

- focused Pane composer;
- selected Inspector mode;
- active workspace/navigation context;
- focused helper surface.

Properties:

- slightly stronger material definition than D1;
- still visually subordinate to terminal content;
- focus may use a subtle edge, tint or contrast lift rather than a heavy border.

### D3 — Attention

For temporary operational relevance:

- approval required;
- execution failure requiring action;
- agent waiting on the user;
- important detached/reconnect state;
- notification/attention popover.

Properties:

- temporary prominence only;
- semantic colour and restrained depth may combine;
- never use pulsing neon, glow, gamer lighting, or continuous distracting animation;
- once resolved, the surface returns to its normal depth.

## 6. Semantic Seams

Semantic Seams are Seyal's primary boundary language.

A seam may separate Blocks, Panes or contextual regions, but it should also communicate a relationship or state when useful.

Rules:

- seams are thin and nearly invisible at rest;
- hover/focus may reveal compact actions or metadata associated with the adjacent object;
- focus/running/attention changes the seam token rather than wrapping the entire object in a card;
- the same seam grammar is reused across Blocks, Pane splits, Inspector relationships, agent context and attention edges;
- seams never glow, pulse or become thick decorative borders.

## 7. Focus gravity

The active work target should have the strongest perceptual gravity without changing layout.

Rules:

- focused terminal Pane remains crisp and stable;
- unfocused Panes may recede slightly through chrome/separator contrast, not by reducing terminal-text legibility;
- left navigation and Inspector recede when not being used;
- selecting an Inspector object may promote the Inspector from D1 to D2;
- attention may temporarily promote only the affected region to D3;
- focus changes must not cause geometry shifts.

Avoid large opacity swings. Adaptive Depth should usually be perceived rather than consciously noticed.

## 8. Transparency and frost

Transparency is allowed only when it improves layering and context.

Preferred uses:

- left context/navigation;
- right Inspector;
- Pane composer;
- command palette;
- menus/popovers/attention surfaces;
- lightweight top-level UI-container chrome.

Do not use transparency for:

- terminal text background when it reduces readability;
- TUI takeover;
- Block output bodies;
- every nested component;
- adjacent stacks of multiple frosted layers.

Frost should remain subtle enough that a developer can work for hours without noticing visual noise behind text.

## 9. Blocks

Blocks remain a fundamental Seyal presentation primitive, but Adaptive Depth explicitly rejects card-heavy Block styling.

Default Block presentation:

- command/header and output form a continuous transcript;
- subtle semantic seam/divider between Blocks;
- no large rounded container around every command;
- no persistent drop shadow;
- actions appear on selection, hover, keyboard invocation, or contextual affordance;
- running/completed/failed state uses compact typography/icon/colour, not a coloured card.

A selected Block may temporarily gain D1/D2 definition for inspection, but the terminal transcript remains visually continuous.

## 10. Pane-scoped composer

The composer is the strongest persistent candidate for frosted material because it is an application editing surface layered over terminal execution.

Rules:

- anchored to the bottom of its terminal Pane;
- minimal default controls;
- subtle material separation from transcript content;
- modest radius only; do not make it a large floating pill/card;
- expand vertically for multiline editing without changing the Pane's Block/output scroll ownership;
- retract/disable while the foreground shell is busy as already specified;
- disappear during canonical full-screen TUI takeover;
- each Pane owns independent composer state.

The same composer component anatomy and geometry must be used across every reference screen and terminal Pane. Only documented state/content variants may change.

## 11. Left context/navigation

The left region keeps Workspace, active-Workspace navigation and compact agent inventory.

Adaptive Depth treatment:

- D1 when present but not focused;
- D2 only while actively navigating/operating it;
- dense list rows instead of cards;
- selected item uses restrained tint/edge/typographic emphasis;
- agent state uses small semantic indicators;
- hover reveals secondary actions instead of permanently showing them.

The panel must feel like a thin utility layer beside the terminal, not an IDE project explorer competing for attention.

## 12. Right contextual Inspector

The Inspector is retained as a core Seyal value-add surface.

Adaptive Depth treatment:

- D1 by default when open;
- D2 when selected/focused or when showing the user's explicit selection;
- D3 only for actionable attention/approval states;
- contextual modes use typography and alignment before containers;
- normal sections should not become nested cards;
- agent details may be shown when an agent is selected, but the full left-side agent inventory is not duplicated.

## 13. Multipane

Multipane must remain readable without becoming a grid of cards.

Rules:

- Pane boundaries use minimal semantic seams;
- focused Pane receives subtle focus gravity;
- inactive Pane chrome recedes while terminal content remains readable;
- each Pane preserves its own Blocks, transcript, composer state and TUI state;
- avoid thick outlines, rounded Pane containers, or per-Pane toolbars that visually fragment the workspace;
- split handles/actions appear contextually where possible.

## 14. Typography-led hierarchy

Hierarchy should come primarily from:

1. typography;
2. alignment;
3. spacing;
4. separators;
5. restrained material contrast;
6. colour only when meaning requires it.

Do not solve hierarchy first with cards, shadows, gradients or heavy backgrounds.

Terminal typography remains independent from application utility typography where necessary. User-configured terminal fonts must not be overridden to satisfy application chrome aesthetics.

## 15. Colour discipline

Colour is operational, not decorative.

Use colour for:

- selection/focus;
- success/completion where useful;
- warning;
- failure/error;
- agent activity/attention;
- remote/reconnect/degraded state;
- explicit user-configured theme accents.

Avoid:

- decorative gradients;
- rainbow status palettes;
- neon edges/glows;
- colour merely to make empty space interesting;
- saturating entire persistent surfaces for ordinary state.

Light and dark themes must preserve the same semantic hierarchy rather than being independently designed skins.

## 16. Motion

Motion communicates state change; it is not ambient decoration.

Allowed:

- short focus/material transitions;
- contextual popover transitions;
- compact progress where the underlying operation has real progress semantics.

Avoid:

- continuous breathing/glow effects;
- animated backgrounds;
- terminal-content blur/opacity animation;
- motion that delays input or rendering;
- motion on the PTY/VT/render hot path.

Reduced-motion accessibility preference must be respected.

## 17. Platform mapping

Adaptive Depth is universal. Native material implementation is platform-specific.

### macOS

Use the smallest appropriate native material/blur integration for D1-D3 utility surfaces. Terminal rendering remains Metal and must not route through a material abstraction.

### Windows

Map the same semantic depth levels to appropriate native system material where available. Preserve layout, spacing, hierarchy and state semantics.

### Linux

Use compositor-supported translucency/blur where reliable. Fall back to carefully chosen opaque tonal surfaces when material support is absent or inconsistent.

### Universal rule

A platform material is an implementation of Seyal Adaptive Depth, not the design authority itself. Seyal must remain recognizably the same product when frost/blur is unavailable.

## 18. Performance rules

Adaptive Depth must never compromise terminal performance.

- material rendering cannot synchronously block PTY read/write, VT parsing, state updates, damage tracking or terminal rendering;
- avoid unnecessary offscreen passes over the terminal canvas;
- blur regions should be bounded to utility surfaces, not the entire terminal viewport;
- inactive depth state should not require continuous recomposition;
- transitions must be benchmarked if they touch render scheduling;
- low-power/reduced-transparency modes must have a cheap opaque fallback.

If a material effect threatens latency, CPU, GPU or memory targets, remove/degrade the effect rather than weakening terminal performance.

## 19. Accessibility

- honour reduced-transparency and reduced-motion settings;
- all states represented by colour must also have shape/icon/text or another non-colour cue;
- terminal contrast/readability has priority over glass aesthetics;
- focus indicators must remain discoverable in both light and dark themes;
- font scaling must not depend on fixed-height decorative containers;
- transparency fallbacks must preserve hierarchy.

## 20. Anti-patterns

Seyal UI should reject:

- gamer/neon styling;
- glass everywhere;
- card grids around terminal Blocks;
- thick Pane borders;
- permanent shadows inside the main workspace;
- large pill-shaped controls as the default language;
- decorative gradients;
- duplicated UI solely for visual balance;
- low-contrast terminal text for aesthetic reasons;
- visual effects that create additional terminal/runtime state ownership;
- platform-specific redesigns that break universal parity;
- screen-specific redesign of a shared component such as the composer, Inspector, sidebar row, tabs or attention item.

## 21. Design acceptance checklist

A screen using Adaptive Depth is acceptable only if:

- the terminal remains the perceptually dominant work surface;
- Blocks are clearly identifiable without looking like a stack of cards;
- left/right utility surfaces can recede without losing discoverability;
- focus is obvious without heavy borders;
- attention is noticeable without glow/noise;
- light and dark themes share the same hierarchy;
- multipane does not become visually fragmented;
- raw/TUI takeover removes application treatment from the terminal viewport;
- reduced-transparency mode remains fully usable;
- the design does not add synchronous work to terminal I/O/render paths;
- shared components match `SEYAL-UNIVERSAL-COMPONENT-CONTRACT.md` rather than drifting per screen;
- the complete replacement set satisfies `SEYAL-REFERENCE-SCREEN-CONTRACTS.md`.

## 22. Relationship to existing M001 references

Existing M001 Core Terminal documents remain authoritative for information architecture, behavior, state ownership, Block semantics, multipane, composer, TUI takeover, Inspector context and implementation ordering.

**Seyal Adaptive Depth is the authority for visual material, depth, transparency, focus hierarchy, colour discipline and cross-platform visual consistency.**

`SEYAL-UNIVERSAL-COMPONENT-CONTRACT.md` is the authority for shared component anatomy/state consistency. `SEYAL-REFERENCE-SCREEN-CONTRACTS.md` defines the replacement visual coverage for the nine historical reference states.

Where the previously frozen generated reference image conflicts only in visual heaviness, cards, borders, material treatment or colour/depth, follow these design authorities. Where a visual reference conflicts with terminal behavior or architecture, the existing architecture/specification remains authoritative.

This document does not authorize implementation ahead of the accepted M001 pass frontier.
