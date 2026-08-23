---
name: macos-ui-testing
description: Add or review native Seyal UI tests with XCTest/XCUIAutomation, focusing on real macOS interaction, focus, keyboard behavior and end-to-end workflows.
---

# macOS UI testing

Use XCTest/XCUIAutomation for production native UI workflows; browser automation is not a substitute.

1. Test user-visible workflows through stable accessibility identifiers where appropriate.
2. Cover keyboard shortcuts, menu commands, focus transitions, mouse interaction, window/tab/pane behavior and failure/reconnect states relevant to the Issue.
3. Exercise representative terminal workloads without asserting a duplicate UI terminal model.
4. Avoid timing sleeps where deterministic expectations, notifications or polling can be used.
5. Capture diagnostics/screenshots on failure and keep tests reproducible from the canonical task interface.
6. Do not make terminal I/O or rendering synchronously depend on test instrumentation.
7. Treat flaky UI tests as defects; quarantine only with a tracked Issue and explicit rationale.

Material native UI changes are incomplete without appropriate UI or integration coverage.