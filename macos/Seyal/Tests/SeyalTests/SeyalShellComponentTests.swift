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
            actions: ["Copy", "Rerun", "Pin", "Expand"]
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
        XCTAssertTrue(descendants(of: NSScrollView.self, in: available).isEmpty)
        XCTAssertTrue(descendants(of: NSScrollView.self, in: busy).isEmpty)
    }

    @MainActor
    func testShellHasExactlyOneVerticalTranscriptScrollOwner() {
        let shell = SeyalShellPreviewFactory.make(
            frame: NSRect(x: 0, y: 0, width: 1280, height: 800)
        )
        shell.layoutSubtreeIfNeeded()

        let verticalOwners = descendants(of: NSScrollView.self, in: shell)
            .filter(\.hasVerticalScroller)
        XCTAssertEqual(verticalOwners.count, 1)
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
