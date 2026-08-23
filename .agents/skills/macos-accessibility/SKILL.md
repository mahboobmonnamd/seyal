---
name: macos-accessibility
description: Review and test Seyal native macOS accessibility, including VoiceOver semantics, keyboard access, focus order, labels, contrast and reduced-motion behavior.
---

# macOS accessibility

1. Use Accessibility Inspector and XCUI accessibility queries for material UI changes.
2. Ensure controls have meaningful roles, labels, values and help text where needed.
3. Verify logical focus order and complete keyboard operation for commands that have mouse affordances.
4. Do not expose terminal secrets or copied raw output unnecessarily through accessibility metadata.
5. Validate high-contrast/system appearance behavior, text scaling where applicable, and reduced-motion alternatives for non-essential animation.
6. For custom Metal/AppKit surfaces, explicitly define the accessibility representation rather than assuming pixels are accessible.
7. Record known limitations as tracked issues; accessibility regressions block completion for affected workflows.
