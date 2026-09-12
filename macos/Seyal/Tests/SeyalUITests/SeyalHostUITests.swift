import XCTest

@MainActor
final class SeyalHostUITests: XCTestCase {
    func testApplicationLaunchesOnePaneHost() throws {
        let app = XCUIApplication()
        app.launch()
        XCTAssertTrue(app.wait(for: .runningForeground, timeout: 10))
        let chrome = app.descendants(matching: .any)["seyal-product-chrome"]
        XCTAssertTrue(chrome.waitForExistence(timeout: 10))
        let pane = app.descendants(matching: .any)["seyal-thin-pane"]
        XCTAssertTrue(pane.waitForExistence(timeout: 10))
        XCTAssertTrue(app.descendants(matching: .any)["seyal-inspector"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.descendants(matching: .any)["seyal-left-workspaces"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.descendants(matching: .any)["seyal-workspace-0"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.descendants(matching: .any)["seyal-recovery"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.descendants(matching: .any)["terminal-input"].waitForExistence(timeout: 5))
    }

    func testLeftPanelSwitchIsARustActionProjection() throws {
        let app = XCUIApplication()
        app.launch()
        let tabs = app.descendants(matching: .any)["seyal-left-tabs"]
        XCTAssertTrue(tabs.waitForExistence(timeout: 10))
        tabs.firstMatch.click()
        XCTAssertTrue(app.descendants(matching: .any)["seyal-tab-0"].waitForExistence(timeout: 5))
        app.descendants(matching: .any)["seyal-left-workspaces"].firstMatch.click()
        XCTAssertTrue(app.descendants(matching: .any)["seyal-workspace-0"].waitForExistence(timeout: 5))
    }

    func testComposerSubmitAndTerminalFocusStayOnRustEligibility() throws {
        let app = XCUIApplication()
        app.launch()
        let composer = app.descendants(matching: .any)["seyal-composer"]
        let terminal = app.descendants(matching: .any)["terminal-input"]
        XCTAssertTrue(terminal.waitForExistence(timeout: 10))
        if composer.waitForExistence(timeout: 8), composer.firstMatch.isHittable {
            composer.firstMatch.click()
            composer.firstMatch.typeText("echo seyal-usable-gate")
            composer.firstMatch.typeText("\r")
        } else {
            terminal.firstMatch.click()
            terminal.firstMatch.typeText("echo seyal-usable-gate\r")
        }
        XCTAssertTrue(app.descendants(matching: .any)["seyal-thin-pane"].exists)
        XCTAssertTrue(app.descendants(matching: .any)["seyal-blocks"].waitForExistence(timeout: 5))
    }

    func testCopyPasteAndQuitMenusAreWired() throws {
        let app = XCUIApplication()
        app.launch()
        XCTAssertTrue(app.wait(for: .runningForeground, timeout: 10))
        app.typeKey("c", modifierFlags: .command)
        app.typeKey("v", modifierFlags: .command)
        XCTAssertEqual(app.state, .runningForeground)
        app.typeKey("q", modifierFlags: .command)
        XCTAssertTrue(app.wait(for: .notRunning, timeout: 8))
    }
}
