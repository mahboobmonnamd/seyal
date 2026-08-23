---
name: apple-platform-docs
description: Research current Apple platform APIs, AppKit, Metal, accessibility and HIG guidance before Seyal native macOS design or implementation decisions.
---

# Apple platform documentation

Use this skill whenever a Seyal macOS decision depends on current Apple API behavior, availability, Human Interface Guidelines, accessibility requirements, Metal/Xcode behavior or platform conventions.

1. State the platform target, macOS/Xcode/SDK assumptions and the concrete question before searching.
2. Prefer current first-party Apple sources: local Xcode documentation, Apple Developer documentation, Human Interface Guidelines, framework headers/SDK declarations and relevant WWDC material.
3. Use the official Xcode MCP bridge when it can expose project/build/documentation context more reliably than generic browsing.
4. Treat third-party AppleDeepDocs or similar indexes as discovery/supplemental evidence only. Verify material API/design claims against Apple sources before making architecture or production decisions.
5. Check API availability, deprecations, minimum macOS version, thread/main-actor requirements, ownership/lifetime rules and documented performance constraints.
6. For AppKit/Swift/Objective-C/Objective-C++ choices, select the smallest native layer justified by the evidence; do not choose a language/framework by fashion.
7. For HIG questions, separate platform convention from Seyal's intentional product extension. Document any deliberate deviation and its keyboard/accessibility consequences.
8. For Metal questions, cross-check the renderer ownership/performance constraints in `.agents/skills/metal-renderer/SKILL.md`.
9. Record links, doc titles/API symbols and relevant SDK/Xcode version in the Issue/ADR/PR when the evidence drives a non-obvious decision.
10. If current Apple documentation contradicts accepted Seyal architecture, stop implementation and use `architecture-change`; do not silently override architecture through an API workaround.

Generic frontend guidance is never authoritative for production AppKit/Metal behavior.