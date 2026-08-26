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
        XCTAssertGreaterThan(window.frame.width, 1200)
        XCTAssertGreaterThan(window.frame.height, 760)

        XCTAssertTrue(app.staticTexts["Seyal OSS"].firstMatch.waitForExistence(timeout: 2))
        XCTAssertTrue(app.staticTexts["WORKSPACES"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.staticTexts["AGENTS"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.staticTexts["TABS"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.staticTexts["Core Terminal"].firstMatch.waitForExistence(timeout: 2))
        XCTAssertTrue(app.staticTexts["Agent Development"].firstMatch.waitForExistence(timeout: 2))
        XCTAssertTrue(app.staticTexts["Logs & Monitoring"].firstMatch.waitForExistence(timeout: 2))
        XCTAssertTrue(app.staticTexts["Inspector"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.staticTexts["ACTIVE PANE"].waitForExistence(timeout: 2))

        XCTAssertTrue(app.staticTexts["git status"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.staticTexts["cargo test -p seyal-terminal"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.staticTexts["kubectl get pods -n seyal"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.staticTexts["Type a command, @ agent, / action…"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.staticTexts["⌃R history"].waitForExistence(timeout: 2))

        XCTAssertTrue(app.staticTexts["Copy"].firstMatch.waitForExistence(timeout: 2))
        XCTAssertTrue(app.staticTexts["Rerun"].firstMatch.waitForExistence(timeout: 2))
        XCTAssertTrue(app.staticTexts["Pin"].firstMatch.waitForExistence(timeout: 2))
        XCTAssertTrue(app.staticTexts["Expand"].firstMatch.waitForExistence(timeout: 2))

        let screenshot = XCUIScreen.main.screenshot()
        let attachment = XCTAttachment(screenshot: screenshot)
        attachment.name = "M001 Core Terminal frozen-reference preview"
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    func testCoreRegionsKeepFrozenReferenceOrdering() {
        let window = app.windows["Seyal — UI Shell Preview"]
        XCTAssertTrue(window.waitForExistence(timeout: 5))

        let left = app.staticTexts["WORKSPACES"]
        let center = app.staticTexts["Core Terminal"].firstMatch
        let right = app.staticTexts["Inspector"]

        XCTAssertTrue(left.waitForExistence(timeout: 2))
        XCTAssertTrue(center.waitForExistence(timeout: 2))
        XCTAssertTrue(right.waitForExistence(timeout: 2))

        XCTAssertLessThan(left.frame.midX, center.frame.midX)
        XCTAssertLessThan(center.frame.midX, right.frame.midX)
        XCTAssertGreaterThan(center.frame.width, 60)
    }

    func testAttentionPopoverIsDrivenByVisibleControl() {
        let attentionButton = app.buttons.matching(
            NSPredicate(format: "label CONTAINS[c] %@", "Attention")
        ).firstMatch
        XCTAssertTrue(attentionButton.waitForExistence(timeout: 5))

        attentionButton.click()

        XCTAssertTrue(app.staticTexts["ATTENTION"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.staticTexts["Agent needs attention"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.staticTexts["Logs tab needs attention"].waitForExistence(timeout: 2))
    }
}
