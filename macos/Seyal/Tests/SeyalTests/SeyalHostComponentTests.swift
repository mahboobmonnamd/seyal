import AppKit
import XCTest

@testable import Seyal

final class SeyalHostComponentTests: XCTestCase {
    func testApplicationRootABIMatchesPublishedHeader() {
        XCTAssertEqual(MemoryLayout<SeyalAppAction>.size, Int(MemoryLayout<SeyalAppAction>.stride))
        XCTAssertGreaterThanOrEqual(MemoryLayout<SeyalAppAction>.size, 80)
        XCTAssertGreaterThanOrEqual(MemoryLayout<SeyalAppSnapshot>.size, 64)
        let handle = seyal_app_create()
        XCTAssertNotEqual(handle, 0)
        let snapshot = seyal_app_snapshot(handle)
        XCTAssertEqual(snapshot.version, UInt16(SEYAL_APP_ABI_VERSION))
        XCTAssertEqual(snapshot.eligibility, UInt16(SEYAL_APP_ELIGIBILITY_UNBOUND.rawValue))
        XCTAssertEqual(MemoryLayout<SeyalAppBlockSpan>.size, 16)
        let emptySpan = seyal_app_block_span(handle, 0)
        XCTAssertEqual(emptySpan.start_line, 0)
        XCTAssertEqual(emptySpan.end_line, 0)
        XCTAssertEqual(seyal_app_destroy(handle), 0)
        let theme = seyal_app_theme(0)
        XCTAssertNotEqual(theme.canvas, theme.text)
        XCTAssertEqual(MemoryLayout<SeyalAppComposer>.size, 40)
        XCTAssertEqual(MemoryLayout<SeyalAppChrome>.size, 24)
        XCTAssertEqual(MemoryLayout<SeyalAppShell>.size, 64)
        XCTAssertEqual(MemoryLayout<SeyalAppRow>.size, 56)
        let live = seyal_app_create()
        let chrome = seyal_app_chrome(live)
        XCTAssertEqual(chrome.reserved & UInt32(SEYAL_APP_CHROME_LEFT_VISIBLE), 0)
        XCTAssertEqual(chrome.reserved & UInt32(SEYAL_APP_CHROME_INSPECTOR_VISIBLE), 0)
        XCTAssertEqual(chrome.reserved & UInt32(SEYAL_APP_CHROME_TAB_STRIP_VISIBLE), 0)
        let shell = seyal_app_shell(live)
        XCTAssertEqual(shell.workspace_count, 1)
        let workspace = seyal_app_shell_row(live, UInt16(SEYAL_APP_ROW_WORKSPACE), 0)
        XCTAssertGreaterThan(workspace.title_len, 0)
        XCTAssertEqual(seyal_app_destroy(live), 0)
        let dark = seyal_app_theme(0)
        let light = seyal_app_theme(1)
        XCTAssertNotEqual(dark.canvas, light.canvas)
        XCTAssertNotEqual(dark.canvas, dark.text)
        XCTAssertNotEqual(dark.accent, 0)
    }

    func testHostHasNoSeyalShellProductTypes() {
        let sourceRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("Sources", isDirectory: true)
        let enumerator = FileManager.default.enumerator(at: sourceRoot, includingPropertiesForKeys: nil)!
        var hits: [String] = []
        for case let file as URL in enumerator where file.pathExtension == "swift" {
            let text = (try? String(contentsOf: file, encoding: .utf8)) ?? ""
            let forbidden = [
                "SeyalShellState",
                "SeyalShellView",
                "SeyalShellPreviewFactory",
                "SeyalShellProductionFactory",
                "SeyalShellModel",
                "PanePresentationSession",
            ]
            if forbidden.contains(where: { text.contains($0) }) {
                hits.append(file.lastPathComponent)
            }
        }
        XCTAssertTrue(hits.isEmpty, "portable product types leaked into \(hits)")
    }

    func testNativeTestsConsumeRustApplicationRootFixturesOnly() {
        let handle = seyal_app_create()
        defer { XCTAssertEqual(seyal_app_destroy(handle), 0) }
        let shell = seyal_app_shell(handle)
        XCTAssertEqual(shell.workspace_count, 1)
        XCTAssertEqual(shell.tab_count, 1)
        XCTAssertEqual(shell.pane_count, 1)
        let chrome = seyal_app_chrome(handle)
        XCTAssertEqual(chrome.left_panel, 0)
        XCTAssertEqual(chrome.reserved, 0)
        let composer = seyal_app_composer(handle)
        XCTAssertEqual(composer.mode, UInt16(SEYAL_APP_COMPOSER_HIDDEN.rawValue))
        let placeholder = seyal_app_copy(handle, UInt16(SEYAL_APP_COPY_COMPOSER_PLACEHOLDER))
        let execute = seyal_app_copy(handle, UInt16(SEYAL_APP_COPY_COMPOSER_EXECUTE))
        let prompt = seyal_app_copy(handle, UInt16(SEYAL_APP_COPY_BLOCK_PROMPT))
        XCTAssertEqual(utf8(placeholder), "Type a command...")
        XCTAssertEqual(utf8(execute), "⏎")
        XCTAssertEqual(utf8(prompt), "$")
        let inspector = seyal_app_chrome_row(handle, UInt16(SEYAL_APP_ROW_INSPECTOR), 0)
        XCTAssertGreaterThan(inspector.title_len, 0)
    }

    func testBundledRuntimeLauncherUsesFixedHelperPath() {
        XCTAssertEqual(BundledRuntimeLauncher.helperRelativePath, "Contents/Helpers/seyal-runtime")
        XCTAssertEqual(BundledRuntimeLauncher.helperIdentifier, "dev.seyal.Seyal.runtime")
    }

    func testTranscriptFrameRejectsZeroBlockIdentity() {
        let invalid = NativeTranscriptFrame(
            revision: 1,
            regions: [NativeTranscriptRegion(id: 0, origin: .zero, clip: .zero)]
        )
        XCTAssertFalse(invalid.isValid)
        let valid = NativeTranscriptFrame(
            revision: 1,
            regions: [
                NativeTranscriptRegion(
                    id: 7,
                    origin: NSPoint(x: 0, y: 12),
                    clip: NSRect(x: 0, y: 12, width: 80, height: 24)
                )
            ]
        )
        XCTAssertTrue(valid.isValid)
        XCTAssertEqual(valid.regionIDs, [7])
    }
}

private func utf8(_ row: SeyalAppRow) -> String {
    guard row.title_len > 0, let title = row.title else { return "" }
    return String(decoding: UnsafeBufferPointer(start: title, count: Int(row.title_len)), as: UTF8.self)
}
