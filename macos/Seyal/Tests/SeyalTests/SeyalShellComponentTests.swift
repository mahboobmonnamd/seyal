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
    func testPreviewTabSelectionChangesCanonicalUISelection() {
        let state = SeyalShellPreviewState.makeDefault()

        state.selectTab(id: "tab-agent")

        XCTAssertEqual(state.snapshot.activeTabID, "tab-agent")
        XCTAssertEqual(
            state.snapshot.inspectorRows.first(where: { $0.id == "tab-name" })?.value,
            "Agent Development"
        )
    }

    @MainActor
    func testPreviewNewTabIsRealLocalNavigationState() {
        let state = SeyalShellPreviewState.makeDefault()
        let originalCount = state.snapshot.tabs.count

        let tab = state.createTab()

        XCTAssertEqual(state.snapshot.tabs.count, originalCount + 1)
        XCTAssertEqual(state.snapshot.activeTabID, tab.id)
        XCTAssertEqual(state.activeTab.paneCount, 1)
    }

    @MainActor
    func testPreviewSplitCreatesIndependentPaneDraftState() {
        let state = SeyalShellPreviewState.makeDefault()
        let firstPaneID = state.activeTab.focusedPaneID
        state.updateDraft("first draft", paneID: firstPaneID)

        let secondPane = state.splitFocusedPane(axis: .right)
        state.updateDraft("second draft", paneID: secondPane.id)

        XCTAssertEqual(state.activeTab.paneCount, 2)
        XCTAssertEqual(state.activeTab.layoutDescription, "Split right")
        XCTAssertEqual(state.activeTab.panes[firstPaneID]?.draft, "first draft")
        XCTAssertEqual(state.activeTab.panes[secondPane.id]?.draft, "second draft")
        XCTAssertEqual(state.activeTab.focusedPaneID, secondPane.id)
    }

    @MainActor
    func testPreviewInspectorDoesNotFabricateRuntimeTelemetry() {
        let state = SeyalShellPreviewState.makeDefault()
        let rows = state.snapshot.inspectorRows

        XCTAssertFalse(rows.contains { $0.section == "Runtime" })
        XCTAssertFalse(rows.contains { $0.label == "PID" })
        XCTAssertFalse(rows.contains { $0.label == "CPU" })
        XCTAssertFalse(rows.contains { $0.label == "Memory" })
    }

    @MainActor
    func testFrozenReferenceUsesDenseThreeColumnLayout() throws {
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
