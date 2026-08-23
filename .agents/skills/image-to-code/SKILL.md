---
name: image-to-code
description: Convert an approved screenshot/mockup into a high-fidelity native Seyal macOS implementation through exhaustive visual decomposition, design documentation, issue slicing, implementation and measured visual QA.
---

# Image to native Seyal code

Use this skill when a screenshot, mockup, prototype image or approved visual reference defines how Seyal should look. The source image is visual evidence, not permission to violate terminal architecture, AppKit behavior, accessibility or runtime ownership.

Read first:

- `AGENTS.md`
- applicable UI/architecture/spec/milestone documents
- `.agents/skills/macos-native-design/SKILL.md`
- `.agents/skills/macos-accessibility/SKILL.md`
- `.agents/skills/visual-regression/SKILL.md`
- `.agents/skills/apple-platform-docs/SKILL.md`
- `.agents/skills/issue-refinement/SKILL.md`

Do not start production implementation until Gates 1-4 below are complete.

## Gate 1 — establish visual authority

1. Identify every source screenshot/mockup and the state each represents.
2. Record original pixel dimensions, scale factor if known, target window/content size, appearance, expected macOS version and any known font/assets.
3. Identify whether the source is native macOS, web prototype, generated concept or another product. Web/generated references provide visual intent only; production behavior still follows Seyal architecture and current macOS conventions.
4. List anything not actually visible or inferable from the source as an **unknown**. Never invent hidden behavior and later describe it as screenshot fidelity.
5. If multiple images disagree, record the conflict and choose/obtain one explicit visual authority before implementation.

## Gate 2 — forensic visual decomposition

Inspect the source at native resolution and high zoom. Use image measurement/sampling tools when available. Perform a pixel-forensic pass rather than a single visual impression.

Create an inventory containing **every visible element**, including small or repeated details:

- top-level window/chrome/content bounds
- title/traffic-light/toolbar/sidebar/tab/pane regions
- separators, dividers, hairlines and gutters
- terminal/Block surfaces and their internal chrome
- buttons, icons, badges, labels, breadcrumbs and disclosure controls
- status indicators, counters and attention markers
- scrollbars, scroll tracks, clipping and overflow behavior
- cards, popovers, menus, sheets, inspectors and notification stacks
- empty states, placeholders and skeletons
- cursor, selection, focus ring, hover/pressed/disabled/active states visible in references
- shadows, blur/material effects, opacity, gradients, borders and corner radii
- all typography hierarchy and baseline/alignment relationships
- z-order/overlap relationships

For each identifiable element assign a stable component ID and record, where measurable:

```text
component-id
parent-id
bounding box: x, y, width, height
padding/insets
inter-component gaps
alignment/baseline rules
corner radius
border/separator thickness
foreground/background/material
font family/size/weight/line height/tracking
icon dimensions/stroke/optical alignment
shadow/blur/opacity
clip/scroll behavior
visible state
source of runtime state
native control/custom rendering choice
confidence / unknowns
```

Do not round measurements early. Preserve measured values in the design analysis, then intentionally normalize them into reusable tokens only after repeated relationships are proven.

"Pixel-level" validation means measured geometry plus image comparison at a controlled capture size. It does **not** mean pretending antialiasing, font rasterization, dynamic timestamps/content or different display scale factors can produce identical bytes. Document those tolerances explicitly rather than ignoring them.

## Gate 3 — component and behavior model

1. Convert the visual inventory into a complete hierarchy/tree. No orphan visual element may disappear between analysis and implementation.
2. For every component identify:
   - purpose and ownership
   - AppKit/native primitive or justified custom view/layer
   - authoritative runtime state consumed
   - interactions and commands
   - keyboard/focus behavior
   - mouse/trackpad behavior
   - accessibility role/name/value/actions
   - resizing/min/max behavior
   - scrolling/clipping
   - visible states and transitions
   - tests and screenshot state needed
3. Distinguish terminal-rendered pixels from application chrome. Never reproduce terminal content as fake AppKit text merely to match the screenshot.
4. Blocks remain presentations of real `TerminalExecution`; they do not gain a PTY, VT grid or copied terminal engine.
5. Approval/attention/popover UI must use existing runtime identities/state, not layout-specific duplicated authority.

## Gate 4 — design document and issue plan

Before implementation, create or update a reviewable design document under the repository's accepted design/UI documentation area. It must contain:

1. source-reference inventory and dimensions
2. annotated region/component hierarchy
3. measurement/token table
4. typography/icon/material specification
5. component contracts and runtime-state mapping
6. interactions, focus, keyboard, accessibility and window-resize behavior
7. scrolling/clipping/overlay/z-order rules
8. visual states and transition/motion rules
9. intentional deviations from the source and why
10. unknowns/assumptions
11. screenshot/visual-regression matrix
12. implementation dependency graph

Then determine issue size.

Use **one Issue** only when the complete visual outcome is independently reviewable, has one clear ownership boundary and can be implemented/tested without creating a large mixed PR.

Propose/create **multiple GitHub Issues** when the screenshot spans independent or dependency-ordered concerns, for example:

```text
foundation/design tokens + window geometry
→ workspace chrome/sidebar/tabs/panes
→ terminal/Block presentation
→ inspectors/popovers/attention stack
→ interactions/accessibility
→ final visual-convergence pass
```

This is an example, not a mandatory split. Derive boundaries from the actual component/state graph.

Each child Issue must:

- use `issue-refinement`
- reference the same approved visual/design document
- define exact components/regions it owns
- define dependencies
- have its own screenshot states and measurable visual acceptance criteria
- preserve one Issue → one branch/worktree → one PR
- avoid parallel mutation of the same authoritative UI/runtime subsystem unless independence is proven

Do not hide a multi-issue feature inside one oversized implementation PR. If multiple issues are needed, propose the full issue graph before coding and implement in dependency order.

## Gate 5 — implementation

For each Ready Issue:

1. Re-read its owned components and source measurements before coding.
2. Implement the smallest component hierarchy that reproduces the design while preserving native macOS semantics.
3. Reuse proven tokens/components only when the source demonstrates the same relationship; do not generalize prematurely.
4. Implement all visible micro-details assigned to the Issue: spacing, separators, icon sizing, baselines, clipping, states and overlays. "Minor" is not a reason to omit a source detail.
5. Keep production terminal rendering on Metal and terminal state authoritative in the runtime.
6. Preserve raw terminal/TUI behavior; visual Blocks/chrome cannot interfere with PTY I/O or alt-screen semantics.
7. Add deterministic component/behavior tests before or with implementation as appropriate.
8. Add XCUI/accessibility coverage for user-visible interaction and focus paths.
9. Capture a native implementation screenshot as soon as the first complete visual state exists; do not wait until the end to discover systemic geometry drift.

## Gate 6 — visual convergence loop

At every material visual checkpoint:

1. Capture the implementation at the same controlled dimensions/scale/appearance/content state as the source where possible.
2. Align source and implementation images by known content bounds.
3. Generate or inspect overlay, absolute-difference and high-zoom views when tooling permits.
4. Check component-by-component, not just whole-screen similarity.
5. Classify every mismatch:
   - geometry/spacing
   - typography/baseline
   - color/material
   - iconography
   - border/radius/shadow
   - clipping/scrolling
   - state/content
   - native-platform intentional deviation
   - nondeterministic rasterization/content
6. Fix deterministic mismatches before declaring the Issue complete.
7. Mask a visual-diff region only when its nondeterminism is documented and the surrounding geometry remains tested.
8. Repeat until no unexplained visible mismatch remains inside the Issue's owned region.

## Gate 7 — functional/native validation

A screenshot match alone is insufficient. Validate:

- keyboard-only workflow and focus order
- mouse/trackpad behavior
- VoiceOver/accessibility tree and actions
- resize/minimum-size behavior
- scrolling and clipping
- normal/raw/alternate-screen terminal behavior where affected
- long/short content and localization-sensitive sizing where relevant
- Retina/backing-scale behavior
- performance/paint impact for terminal-adjacent UI
- no extra PTYs, duplicate terminal state or synchronous terminal hot-path work

## Definition of done

The PR may claim screenshot fidelity only when:

- every visible source element in scope is represented in the component inventory
- every implemented component traces back to the design document/source or an explicit native-platform deviation
- no source detail was silently omitted
- controlled before/source/after/diff evidence is attached or reproducible
- remaining differences are individually explained and accepted
- interaction/accessibility tests pass
- architecture/performance invariants remain intact
- the Issue's visual acceptance criteria are satisfied

Never use "looks close", "approximately the same" or a successful build as evidence that an image-to-code task is complete.