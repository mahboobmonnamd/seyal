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
        XCTAssertEqual(seyal_app_destroy(handle), 0)
        let theme = seyal_app_theme(0)
        XCTAssertNotEqual(theme.canvas, theme.text)
        XCTAssertEqual(MemoryLayout<SeyalAppComposer>.size, 40)
        XCTAssertEqual(MemoryLayout<SeyalAppChrome>.size, 24)
        XCTAssertEqual(MemoryLayout<SeyalAppShell>.size, 64)
        XCTAssertEqual(MemoryLayout<SeyalAppRow>.size, 56)
        let live = seyal_app_create()
        let shell = seyal_app_shell(live)
        XCTAssertEqual(shell.workspace_count, 1)
        let workspace = seyal_app_shell_row(live, UInt16(SEYAL_APP_ROW_WORKSPACE), 0)
        XCTAssertGreaterThan(workspace.title_len, 0)
        XCTAssertEqual(seyal_app_destroy(live), 0)
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
        let composer = seyal_app_composer(handle)
        XCTAssertEqual(composer.mode, UInt16(SEYAL_APP_COMPOSER_HIDDEN.rawValue))
        let inspector = seyal_app_chrome_row(handle, UInt16(SEYAL_APP_ROW_INSPECTOR), 0)
        XCTAssertGreaterThan(inspector.title_len, 0)
    }

    func testBundledRuntimeLauncherUsesFixedHelperPath() {
        XCTAssertEqual(BundledRuntimeLauncher.helperRelativePath, "Contents/Helpers/seyal-runtime")
        XCTAssertEqual(BundledRuntimeLauncher.helperIdentifier, "dev.seyal.Seyal.runtime")
    }
}
