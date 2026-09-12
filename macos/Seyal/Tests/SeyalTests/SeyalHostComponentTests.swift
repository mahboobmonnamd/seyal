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
            if text.contains("SeyalShellState") || text.contains("SeyalShellView") {
                hits.append(file.lastPathComponent)
            }
        }
        XCTAssertTrue(hits.isEmpty, "portable product types leaked into \(hits)")
    }

    func testBundledRuntimeLauncherUsesFixedHelperPath() {
        XCTAssertEqual(BundledRuntimeLauncher.helperRelativePath, "Contents/Helpers/seyal-runtime")
        XCTAssertEqual(BundledRuntimeLauncher.helperIdentifier, "dev.seyal.Seyal.runtime")
    }
}
