import XCTest

final class SeyalNativeGlueUITests: XCTestCase {
    func testDefaultLaunchIsNativeGlueHarnessNotProductShell() {
        let app = XCUIApplication()
        app.launch()

        let harness = app.staticTexts["seyal-native-glue-harness"]
        XCTAssertTrue(
            harness.waitForExistence(timeout: 5),
            "default launch must be the native-glue harness, not a headed product"
        )
        XCTAssertFalse(app.textViews["composer.pane-local"].exists)
        XCTAssertFalse(app.textViews["composer.pane-1"].exists)
        XCTAssertFalse(app.buttons["toggle-left-sidebar"].exists)
        XCTAssertFalse(app.buttons["toggle-inspector"].exists)
    }
}
