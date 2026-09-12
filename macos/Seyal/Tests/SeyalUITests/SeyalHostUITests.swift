import XCTest

@MainActor
final class SeyalHostUITests: XCTestCase {
    func testApplicationLaunchesOnePaneHost() throws {
        let app = XCUIApplication()
        app.launch()
        XCTAssertTrue(app.wait(for: .runningForeground, timeout: 10))
        let pane = app.descendants(matching: .any)["seyal-thin-pane"]
        XCTAssertTrue(pane.waitForExistence(timeout: 10))
    }
}
