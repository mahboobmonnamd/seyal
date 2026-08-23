---
name: macos-native-design
description: Design or review Seyal native macOS UI using current Apple HIG and AppKit conventions without turning web-prototype patterns into production authority.
---

# Native macOS design

Use this skill for production Seyal windows, tabs, panes, toolbars, menus, popovers, inspectors, notifications, focus behavior and keyboard/mouse interaction.

1. Treat current Apple Human Interface Guidelines and platform behavior as external design authority; record links/evidence in the Issue or PR when a non-obvious convention matters.
2. Prefer AppKit-native interaction semantics where they fit Seyal; do not force web UI patterns into the native host.
3. Preserve keyboard-first and mouse-first parity, predictable focus, discoverable commands and system menu integration.
4. Keep terminal rendering and hot-path state outside SwiftUI/AppKit presentation state. UI may observe authoritative runtime state but must not create a competing terminal model.
5. Design Blocks, attention items, approvals, inspectors and notifications as presentations over existing runtime identities; do not invent extra PTYs or execution objects.
6. For novel/futuristic UI, document which macOS convention is intentionally extended and why usability/accessibility remain sound.
7. Require screenshots or an executable demo for material visual changes.

Do not use generic frontend-design guidance as authority for production AppKit/Metal behavior.