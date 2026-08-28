import AppKit
import XCTest
@testable import Seyal

final class SeyalShellComponentTests: XCTestCase {
    @MainActor
    func testBlockOwnsNoNestedScrollView() {
        let body = NSView()
        body.translatesAutoresizingMaskIntoConstraints = false
        body.heightAnchor.constraint(equalToConstant: 120).isActive = true

        let presentation = BlockPresentation(
            id: "component-test",
            command: "make test",
            state: BlockPresentationState.completed,
            elapsed: "12 ms",
            timestamp: "09:00",
            isSelected: true,
            actions: ["Copy", "Pin"]
        )
        let block = BlockView(presentation: presentation, bodyView: body)

        XCTAssertTrue(descendants(of: NSScrollView.self, in: block).isEmpty)
        XCTAssertTrue(block.subviewsRecursively.contains { $0 === body })
    }

    @MainActor
    func testBlockTUITakeoverHidesOnlyPresentationChrome() {
        let body = NSView()
        body.translatesAutoresizingMaskIntoConstraints = false
        body.heightAnchor.constraint(equalToConstant: 120).isActive = true
        let block = BlockView(
            presentation: BlockPresentation(
                id: "tui", command: "nvim", state: .running, elapsed: "Live",
                timestamp: nil, isSelected: true, actions: []
            ),
            bodyView: body
        )

        block.setTUITakeover(true)

        XCTAssertTrue(block.subviewsRecursively.contains { $0 === body })
        XCTAssertEqual(block.layer?.borderWidth, 0)
        XCTAssertFalse(body.isHidden)
    }

    @MainActor
    func testComposerModesRespectPaneOwnershipRules() {
        let available = PaneComposerShellView(mode: .available, draft: "git status")
        let busy = PaneComposerShellView(mode: .busy(process: "vite"), draft: "")
        let tui = PaneComposerShellView(mode: .hiddenForTUI, draft: "")

        XCTAssertFalse(available.isHidden)
        XCTAssertFalse(busy.isHidden)
        XCTAssertTrue(tui.isHidden)
        XCTAssertEqual(descendants(of: NSTextView.self, in: available).count, 1)
        XCTAssertTrue(
            descendants(of: NSScrollView.self, in: available)
                .allSatisfy { !$0.hasVerticalScroller }
        )
        XCTAssertTrue(descendants(of: NSTextView.self, in: busy).isEmpty)
    }

    @MainActor
    func testComposerReturnSubmitsInsteadOfInsertingANewline() {
        var submitted: String?
        let composer = PaneComposerShellView(
            mode: .available,
            draft: "echo from composer",
            onSubmit: {
                submitted = $0
                return true
            }
        )
        let editor = try! XCTUnwrap(descendants(of: NSTextView.self, in: composer).first)

        editor.doCommand(by: #selector(NSResponder.insertNewline(_:)))

        XCTAssertEqual(submitted, "echo from composer")
        XCTAssertFalse(editor.string.contains("\n"))
    }

    @MainActor
    func testComposerFieldEditorReturnSubmitsInsteadOfInsertingANewline() {
        var submitted: String?
        let composer = PaneComposerShellView(
            mode: .available,
            draft: "pwd",
            onSubmit: {
                submitted = $0
                return true
            }
        )
        let editor = try! XCTUnwrap(descendants(of: NSTextView.self, in: composer).first)

        editor.doCommand(by: #selector(NSResponder.insertNewlineIgnoringFieldEditor(_:)))

        XCTAssertEqual(submitted, "pwd")
        XCTAssertEqual(editor.string, "")
    }

    @MainActor
    func testComposerPreservesDraftWhenSubmissionIsRejected() {
        let composer = PaneComposerShellView(
            mode: .available,
            draft: "echo while disconnected",
            onSubmit: { _ in false }
        )
        let editor = try! XCTUnwrap(descendants(of: NSTextView.self, in: composer).first)

        editor.doCommand(by: #selector(NSResponder.insertNewline(_:)))

        XCTAssertEqual(editor.string, "echo while disconnected")
    }

    @MainActor
    func testComposerCanRestoreFirstResponderAfterBlockTimelineRebuild() throws {
        let composer = PaneComposerShellView(mode: .available, draft: "")
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 640, height: 120),
            styleMask: .borderless,
            backing: .buffered,
            defer: true
        )
        window.contentView = composer
        window.makeKeyAndOrderFront(nil)
        defer { window.orderOut(nil) }

        let editor = try XCTUnwrap(descendants(of: NSTextView.self, in: composer).first)
        composer.focusEditor()

        XCTAssertTrue(window.firstResponder === editor)
    }

    @MainActor
    func testPreparedFrameProjectionProducesBlockOutputText() {
        var cells = [
            SeyalPreparedCell(scalar: 108, foreground: 0, background: 0, flags: 0, reserved: 0),
            SeyalPreparedCell(scalar: 115, foreground: 0, background: 0, flags: 0, reserved: 0),
            SeyalPreparedCell(scalar: 0, foreground: 0, background: 0, flags: 0, reserved: 0),
            SeyalPreparedCell(scalar: 0, foreground: 0, background: 0, flags: 0, reserved: 0),
        ]
        let frame = cells.withUnsafeBufferPointer {
            NativePreparedFrame(
                cells: $0,
                generation: 1,
                rows: 2,
                columns: 2,
                cursorRow: 0,
                cursorColumn: 0,
                cursorVisible: false,
                alternateScreen: false,
                fullRebuild: true,
                damage: DamageMask()
            )
        }

        XCTAssertEqual(CommandBlockBodyView.text(from: frame), "ls")
    }

    @MainActor
    func testShellHasExactlyOneVerticalTranscriptScrollOwnerInitially() {
        let shell = SeyalShellPreviewFactory.make(
            frame: NSRect(x: 0, y: 0, width: 1280, height: 800)
        )
        shell.layoutSubtreeIfNeeded()

        let verticalOwners = descendants(of: NSScrollView.self, in: shell)
            .filter(\.hasVerticalScroller)
        XCTAssertEqual(verticalOwners.count, 1)
    }

    @MainActor
    func testProductionShellUsesOneRealSurfaceAndOneBlock() {
        let shell = SeyalShellProductionFactory.make(
            frame: NSRect(x: 0, y: 0, width: 960, height: 600)
        )
        shell.layoutSubtreeIfNeeded()

        XCTAssertEqual(descendants(of: InteractiveMetalSurfaceView.self, in: shell).count, 1)
        XCTAssertEqual(descendants(of: BlockView.self, in: shell).count, 1)
        XCTAssertTrue(
            descendants(of: NSTextField.self, in: shell)
                .contains { $0.stringValue == "Interactive shell" }
        )
        XCTAssertTrue(descendants(of: TerminalSurfaceHostView.self, in: shell).isEmpty)
    }

    @MainActor
    func testPreviewWorkspaceInventoryMatchesFrozenWorkspaceModel() {
        let state = SeyalShellState.makePreview()

        XCTAssertEqual(
            state.snapshot.workspaces.map(\.name),
            ["Seyal OSS", "Payments Platform", "Infra Operations", "Personal Lab"]
        )
        XCTAssertEqual(state.leftPanelMode, .workspaces)
        XCTAssertEqual(state.snapshot.agents.map(\.name), ["Claude Code", "Codex", "OpenCode"])

        state.setLeftPanelMode(.tabs)
        XCTAssertEqual(state.leftPanelMode, .tabs)
        XCTAssertEqual(state.snapshot.tabs.count, 4)
    }

    @MainActor
    func testPreviewTabSelectionChangesCanonicalUISelection() {
        let state = SeyalShellState.makePreview()

        state.selectTab(id: "tab-agent")

        XCTAssertEqual(state.snapshot.activeTabID, "tab-agent")
        XCTAssertEqual(
            state.snapshot.inspectorRows.first(where: { $0.id == "tab-name" })?.value,
            "Agent Development"
        )
    }

    @MainActor
    func testPreviewNewTabIsRealLocalNavigationState() {
        let state = SeyalShellState.makePreview()
        let originalCount = state.snapshot.tabs.count

        let tab = state.createTab()

        XCTAssertEqual(state.snapshot.tabs.count, originalCount + 1)
        XCTAssertEqual(state.snapshot.activeTabID, tab.id)
        XCTAssertEqual(state.activeTab.paneCount, 1)
    }

    @MainActor
    func testPreviewSplitAndCloseArePaneLocal() {
        let state = SeyalShellState.makePreview()
        let firstPaneID = state.activeTab.focusedPaneID
        state.updateDraft("first draft", paneID: firstPaneID)

        let secondPane = state.splitPane(id: firstPaneID, axis: .right)
        state.updateDraft("second draft", paneID: secondPane.id)

        XCTAssertEqual(state.activeTab.paneCount, 2)
        XCTAssertEqual(state.activeTab.layoutDescription, "Split right")
        XCTAssertEqual(state.activeTab.panes[firstPaneID]?.draft, "first draft")
        XCTAssertEqual(state.activeTab.panes[secondPane.id]?.draft, "second draft")
        XCTAssertEqual(state.activeTab.focusedPaneID, secondPane.id)

        state.closePane(id: secondPane.id)

        XCTAssertEqual(state.activeTab.paneCount, 1)
        XCTAssertEqual(state.activeTab.focusedPaneID, firstPaneID)
        XCTAssertEqual(state.activeTab.panes[firstPaneID]?.draft, "first draft")
    }

    @MainActor
    func testPreviewInspectorDoesNotFabricateRuntimeTelemetry() {
        let state = SeyalShellState.makePreview()
        let rows = state.snapshot.inspectorRows

        XCTAssertFalse(rows.contains { $0.section == "Runtime" })
        XCTAssertFalse(rows.contains { $0.label == "PID" })
        XCTAssertFalse(rows.contains { $0.label == "CPU" })
        XCTAssertFalse(rows.contains { $0.label == "Memory" })
    }

    @MainActor
    func testInspectorRailFiltersExistingContextOnly() {
        let shell = SeyalShellPreviewFactory.make(
            frame: NSRect(x: 0, y: 0, width: 1280, height: 800)
        )

        shell.debugSetInspectorMode(.tab)
        let tabRows = shell.debugVisibleInspectorRows()
        XCTAssertFalse(tabRows.isEmpty)
        XCTAssertTrue(tabRows.allSatisfy { $0.section == "Tab" })
        XCTAssertTrue(tabRows.contains { $0.id == "tab-name" })
        XCTAssertFalse(tabRows.contains { $0.id == "workspace-name" })

        shell.debugSetInspectorMode(.pane)
        let paneRows = shell.debugVisibleInspectorRows()
        XCTAssertFalse(paneRows.isEmpty)
        XCTAssertTrue(paneRows.allSatisfy { $0.section == "Active Pane" })
        XCTAssertFalse(paneRows.contains { $0.label == "PID" || $0.label == "CPU" || $0.label == "Memory" })
    }

    @MainActor
    func testSidebarsCollapseAndCenterPaneReclaimsTheirWidth() throws {
        let shell = SeyalShellPreviewFactory.make(
            frame: NSRect(x: 0, y: 0, width: 1280, height: 800)
        )
        shell.layoutSubtreeIfNeeded()
        let expanded = try XCTUnwrap(shell.debugLayoutContract())

        shell.debugSetSidebarVisibility(left: false, inspector: false)
        shell.layoutSubtreeIfNeeded()
        let collapsed = try XCTUnwrap(shell.debugLayoutContract())

        XCTAssertEqual(collapsed.leftContext.width, 0, accuracy: 1)
        XCTAssertEqual(collapsed.inspector.width, 0, accuracy: 1)
        XCTAssertEqual(collapsed.pane.minX, shell.bounds.minX, accuracy: 1)
        XCTAssertEqual(collapsed.pane.maxX, shell.bounds.maxX, accuracy: 1)
        XCTAssertGreaterThan(
            collapsed.pane.width,
            expanded.pane.width
                + SeyalDesignTokens.Layout.leftContextWidth
                + SeyalDesignTokens.Layout.inspectorWidth
                - 4
        )

        shell.debugSetSidebarVisibility(left: true, inspector: true)
        shell.layoutSubtreeIfNeeded()
        let restored = try XCTUnwrap(shell.debugLayoutContract())
        XCTAssertEqual(restored.leftContext.width, SeyalDesignTokens.Layout.leftContextWidth, accuracy: 1)
        XCTAssertEqual(restored.inspector.width, SeyalDesignTokens.Layout.inspectorWidth, accuracy: 1)
        XCTAssertEqual(restored.pane.width, expanded.pane.width, accuracy: 1)
    }

    @MainActor
    func testNativeShortcutMenuUsesMacTerminalConventions() throws {
        let oldMainMenu = NSApp.mainMenu
        let oldWindowsMenu = NSApp.windowsMenu
        defer {
            NSApp.mainMenu = oldMainMenu
            NSApp.windowsMenu = oldWindowsMenu
        }

        let state = SeyalShellState.makePreview()
        let shell = SeyalShellPreviewFactory.make(
            frame: NSRect(x: 0, y: 0, width: 1280, height: 800),
            state: state
        )
        let window = NSWindow(
            contentRect: shell.frame,
            styleMask: [.titled, .closable, .resizable],
            backing: .buffered,
            defer: false
        )
        window.contentView = shell

        let shortcuts = SeyalPreviewShortcutController(window: window, state: state)
        shortcuts.installMenus()

        let workspaceMenu = try XCTUnwrap(NSApp.mainMenu?.item(withTitle: "Workspace")?.submenu)
        let workspace2 = try XCTUnwrap(workspaceMenu.items.first { $0.tag == 1 && $0.keyEquivalent == "2" })
        XCTAssertEqual(normalizedModifiers(workspace2), [.command, .control])

        let tabMenu = try XCTUnwrap(NSApp.mainMenu?.item(withTitle: "Tab")?.submenu)
        let tab2 = try XCTUnwrap(tabMenu.items.first { $0.tag == 1 && $0.keyEquivalent == "2" })
        XCTAssertEqual(normalizedModifiers(tab2), [.command])
        let close = try XCTUnwrap(tabMenu.item(withTitle: "Close Focused Pane / Tab / Window"))
        XCTAssertEqual(close.keyEquivalent, "w")
        XCTAssertEqual(normalizedModifiers(close), [.command])
        let nextTab = try XCTUnwrap(tabMenu.item(withTitle: "Next Tab"))
        XCTAssertEqual(nextTab.keyEquivalent, "]")
        XCTAssertEqual(normalizedModifiers(nextTab), [.command, .shift])

        let windowMenu = try XCTUnwrap(NSApp.mainMenu?.item(withTitle: "Window")?.submenu)
        let window2 = try XCTUnwrap(windowMenu.items.first { $0.tag == 1 && $0.keyEquivalent == "2" })
        XCTAssertEqual(normalizedModifiers(window2), [.command, .option])
        let nextWindow = try XCTUnwrap(windowMenu.item(withTitle: "Next Window"))
        XCTAssertEqual(nextWindow.keyEquivalent, "`")
        XCTAssertEqual(normalizedModifiers(nextWindow), [.command])

        let viewMenu = try XCTUnwrap(NSApp.mainMenu?.item(withTitle: "View")?.submenu)
        XCTAssertEqual(
            normalizedModifiers(try XCTUnwrap(viewMenu.item(withTitle: "Toggle Navigation Sidebar"))),
            [.command]
        )
        XCTAssertEqual(
            normalizedModifiers(try XCTUnwrap(viewMenu.item(withTitle: "Toggle Inspector"))),
            [.command, .option]
        )
    }

    @MainActor
    func testCloseShortcutTargetCascadesPaneThenTabThenWindow() {
        let state = SeyalShellState.makePreview()
        let originalTabID = state.activeTab.id
        let secondPane = state.splitPane(id: state.activeTab.focusedPaneID, axis: .right)

        XCTAssertEqual(
            SeyalPreviewShortcutController.closeTarget(for: state),
            .pane(secondPane.id)
        )

        state.closePane(id: secondPane.id)
        XCTAssertEqual(
            SeyalPreviewShortcutController.closeTarget(for: state),
            .tab(originalTabID)
        )

        while state.activeWorkspace.tabs.count > 1 {
            state.closeTab(id: state.activeTab.id)
        }
        XCTAssertEqual(SeyalPreviewShortcutController.closeTarget(for: state), .window)
        XCTAssertEqual(state.activeTab.paneCount, 1)
    }

    func testCommandHoldHintPolicyRequiresIntentionalCommandOnlyHold() {
        XCTAssertEqual(SeyalShortcutHintPolicy.intentionalHoldDelay, 0.30, accuracy: 0.0001)
        XCTAssertTrue(SeyalShortcutHintPolicy.isCommandOnly([.command]))
        XCTAssertTrue(SeyalShortcutHintPolicy.isCommandOnly([.command, .capsLock]))
        XCTAssertFalse(SeyalShortcutHintPolicy.isCommandOnly([.command, .shift]))
        XCTAssertFalse(SeyalShortcutHintPolicy.isCommandOnly([.command, .option]))
        XCTAssertFalse(SeyalShortcutHintPolicy.isCommandOnly([.control]))
        XCTAssertFalse(SeyalShortcutHintPolicy.isCommandOnly([]))
    }

    @MainActor
    func testShortcutHintOverlayDoesNotChangeShellLayout() throws {
        let shell = SeyalShellPreviewFactory.make(
            frame: NSRect(x: 0, y: 0, width: 1280, height: 800)
        )
        shell.layoutSubtreeIfNeeded()
        let before = try XCTUnwrap(shell.debugLayoutContract())

        let overlay = SeyalShortcutHintOverlay(frame: .zero)
        overlay.present([
            .init(
                targetAccessibilityID: "tab.tab-terminal",
                text: "⌘1",
                id: "tab.tab-terminal"
            ),
            .init(
                targetAccessibilityID: "toggle-left-sidebar",
                text: "⌘0",
                id: "left-sidebar"
            ),
        ], in: shell)
        shell.layoutSubtreeIfNeeded()
        let after = try XCTUnwrap(shell.debugLayoutContract())

        XCTAssertEqual(before.pane, after.pane)
        XCTAssertEqual(before.leftContext, after.leftContext)
        XCTAssertEqual(before.inspector, after.inspector)
        XCTAssertFalse(overlay.isHidden)
        XCTAssertTrue(shell.subviewsRecursively.contains {
            $0.accessibilityIdentifier() == "shortcut-hint.tab.tab-terminal"
        })

        overlay.dismiss()
        XCTAssertTrue(overlay.isHidden)
    }

    @MainActor
    func testShortcutRoutingMutatesExistingShellStateWithoutReplacingView() throws {
        let oldMainMenu = NSApp.mainMenu
        let oldWindowsMenu = NSApp.windowsMenu
        defer {
            NSApp.mainMenu = oldMainMenu
            NSApp.windowsMenu = oldWindowsMenu
        }

        let state = SeyalShellState.makePreview()
        let shell = SeyalShellPreviewFactory.make(
            frame: NSRect(x: 0, y: 0, width: 1280, height: 800),
            state: state
        )
        let window = NSWindow(
            contentRect: shell.frame,
            styleMask: [.titled, .closable, .resizable],
            backing: .buffered,
            defer: false
        )
        window.contentView = shell
        let shortcuts = SeyalPreviewShortcutController(window: window, state: state)
        shortcuts.installMenus()

        shell.debugSetSidebarVisibility(left: false, inspector: true)
        let tab2 = NSMenuItem()
        tab2.tag = 1
        shortcuts.selectTabByNumber(tab2)

        XCTAssertTrue(window.contentView === shell)
        XCTAssertEqual(state.snapshot.activeTabID, "tab-agent")
        shell.layoutSubtreeIfNeeded()
        XCTAssertEqual(try XCTUnwrap(shell.debugLayoutContract()).leftContext.width, 0, accuracy: 1)

        let workspace2 = NSMenuItem()
        workspace2.tag = 1
        shortcuts.selectWorkspaceByNumber(workspace2)
        XCTAssertEqual(state.snapshot.activeWorkspaceID, "workspace-payments")
        XCTAssertEqual(state.snapshot.activeTabID, "tab-payments-api")

        shortcuts.nextWorkspace(nil)
        XCTAssertEqual(state.snapshot.activeWorkspaceID, "workspace-infra")
        shortcuts.previousWorkspace(nil)
        XCTAssertEqual(state.snapshot.activeWorkspaceID, "workspace-payments")
    }

    func testShortcutWrappedIndexCyclesBothDirections() {
        XCTAssertEqual(SeyalPreviewShortcutController.wrappedIndex(current: 0, count: 4, offset: -1), 3)
        XCTAssertEqual(SeyalPreviewShortcutController.wrappedIndex(current: 3, count: 4, offset: 1), 0)
        XCTAssertEqual(SeyalPreviewShortcutController.wrappedIndex(current: 1, count: 4, offset: 1), 2)
    }

    @MainActor
    func testFrozenReferenceUsesDenseThreeColumnLayoutWithoutTrailingGap() throws {
        let shell = SeyalShellPreviewFactory.make(
            frame: NSRect(x: 0, y: 0, width: 1280, height: 800)
        )
        shell.layoutSubtreeIfNeeded()
        let contract = try XCTUnwrap(shell.debugLayoutContract())

        XCTAssertEqual(contract.topChrome.width, 1280, accuracy: 1)
        XCTAssertEqual(
            contract.topChrome.height,
            SeyalDesignTokens.Layout.topChromeHeight,
            accuracy: 1
        )
        XCTAssertEqual(
            contract.leftContext.width,
            SeyalDesignTokens.Layout.leftContextWidth,
            accuracy: 1
        )
        XCTAssertEqual(
            contract.inspector.width,
            SeyalDesignTokens.Layout.inspectorWidth,
            accuracy: 1
        )
        XCTAssertEqual(contract.inspector.maxX, shell.bounds.maxX, accuracy: 1)
        XCTAssertEqual(contract.pane.maxX + 1, contract.inspector.minX, accuracy: 1)
        XCTAssertGreaterThan(contract.pane.width, 700)
        XCTAssertGreaterThan(contract.composer.width, 650)
        XCTAssertGreaterThanOrEqual(
            contract.composer.height,
            SeyalDesignTokens.Layout.composerMinHeight - 1
        )
    }

    @MainActor
    func testFrozenReferencePaletteIsDarkAndNotSystemAdaptive() {
        let background = SeyalDesignTokens.Palette.windowBackground.usingColorSpace(.deviceRGB)
        let components = background?.cgColor.components ?? []
        XCTAssertGreaterThanOrEqual(components.count, 3)
        if components.count >= 3 {
            XCTAssertLessThan(components[0], 0.12)
            XCTAssertLessThan(components[1], 0.12)
            XCTAssertLessThan(components[2], 0.12)
        }
    }

    @MainActor
    func testTerminalSurfaceHostContainsPermanentMetalSurface() {
        let host = TerminalSurfaceHostView(frame: NSRect(x: 0, y: 0, width: 640, height: 400))
        XCTAssertTrue(host.subviews.contains { $0 === host.metalSurface })
        XCTAssertFalse(host.subviewsRecursively.contains { $0 is NSTextView })
    }

    @MainActor
    func testShellPreviewRequiresDebugConfigurationAndExplicitOptIn() {
        XCTAssertTrue(
            AppDelegate.shouldUseShellPreview(
                arguments: ["Seyal", "--ui-shell-preview"],
                environment: [:],
                buildConfiguration: "Debug"
            )
        )
        XCTAssertTrue(
            AppDelegate.shouldUseShellPreview(
                arguments: ["Seyal"],
                environment: ["SEYAL_UI_SHELL_PREVIEW": "1"],
                buildConfiguration: "Debug"
            )
        )
        XCTAssertFalse(
            AppDelegate.shouldUseShellPreview(
                arguments: ["Seyal", "--ui-shell-preview"],
                environment: [:],
                buildConfiguration: "Release"
            )
        )
        XCTAssertFalse(
            AppDelegate.shouldUseShellPreview(
                arguments: ["Seyal"],
                environment: [:],
                buildConfiguration: "Debug"
            )
        )
    }

    @MainActor
    private func normalizedModifiers(_ item: NSMenuItem) -> NSEvent.ModifierFlags {
        item.keyEquivalentModifierMask.intersection(.deviceIndependentFlagsMask)
    }

    @MainActor
    private func descendants<T: NSView>(of type: T.Type, in root: NSView) -> [T] {
        root.subviews.flatMap { child -> [T] in
            var matches = child is T ? [child as! T] : []
            matches.append(contentsOf: descendants(of: type, in: child))
            return matches
        }
    }
}

private extension NSView {
    var subviewsRecursively: [NSView] {
        subviews + subviews.flatMap(\.subviewsRecursively)
    }
}
