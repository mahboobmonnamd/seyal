import XCTest

final class SeyalShellUITests: XCTestCase {
    private var app: XCUIApplication!

    override func setUpWithError() throws {
        continueAfterFailure = false
        app = XCUIApplication()
        app.launchArguments = ["--ui-shell-preview"]
        app.launch()
    }

    override func tearDownWithError() throws {
        app.terminate()
        app = nil
    }

    func testShellLaunchesWithFrozenCoreHierarchy() {
        let window = app.windows["Seyal — UI Shell Preview"]
        XCTAssertTrue(window.waitForExistence(timeout: 5))

        XCTAssertTrue(app.staticTexts["Seyal"].firstMatch.waitForExistence(timeout: 2))
        XCTAssertTrue(app.staticTexts["WORKSPACES"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.staticTexts["AGENTS"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.staticTexts["TABS"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.staticTexts["Terminal"].firstMatch.waitForExistence(timeout: 2))
        XCTAssertTrue(app.staticTexts["CONTEXT"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.staticTexts["git status --short"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.staticTexts["make test"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.staticTexts["git diff --stat &&\nmake test"].waitForExistence(timeout: 2))
    }

    func testAttentionPopoverIsDrivenByVisibleControl() {
        let attentionButton = app.buttons.matching(
            NSPredicate(format: "label CONTAINS[c] %@", "Attention")
        ).firstMatch
        XCTAssertTrue(attentionButton.waitForExistence(timeout: 5))

        attentionButton.click()

        XCTAssertTrue(app.staticTexts["ATTENTION"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.staticTexts["Agent needs attention"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.staticTexts["Build finished"].waitForExistence(timeout: 2))
    }
}
