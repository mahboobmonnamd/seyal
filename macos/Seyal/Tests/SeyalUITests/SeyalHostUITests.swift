import XCTest

@MainActor
final class SeyalHostUITests: XCTestCase {
    func testApplicationLaunchesOnePaneHost() throws {
        let app = XCUIApplication()
        app.launch()
        XCTAssertTrue(app.wait(for: .runningForeground, timeout: 10))
        // Attach/resize used to recurse into SIGSEGV within ~250ms of a real
        // `open`. Chrome existence is not enough; the process must stay up.
        RunLoop.current.run(until: Date().addingTimeInterval(2))
        XCTAssertEqual(app.state, .runningForeground, "Seyal.app crashed after launch")
        let chrome = app.descendants(matching: .any)["seyal-product-chrome"]
        XCTAssertTrue(chrome.waitForExistence(timeout: 10))
        let pane = app.descendants(matching: .any)["seyal-thin-pane"]
        XCTAssertTrue(pane.waitForExistence(timeout: 10))
        XCTAssertTrue(app.descendants(matching: .any)["seyal-inspector"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.descendants(matching: .any)["seyal-left-workspaces"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.descendants(matching: .any)["seyal-workspace-0"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.descendants(matching: .any)["seyal-recovery"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.descendants(matching: .any)["terminal-input"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.descendants(matching: .any)["seyal-composer"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.descendants(matching: .any)["seyal-tab-strip"].waitForExistence(timeout: 5))
        waitForUsablePty(in: app)
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
        waitForUsablePty(in: app)
        let composer = app.descendants(matching: .any)["seyal-composer"]
        XCTAssertTrue(composer.waitForExistence(timeout: 12))
        let composerReady = NSPredicate(format: "value == 'available'")
        let becameReady = expectation(for: composerReady, evaluatedWith: composer, handler: nil)
        XCTAssertEqual(
            XCTWaiter.wait(for: [becameReady], timeout: 12),
            .completed,
            "composer never became submittable after PTY attach; value=\(composer.value ?? "nil")"
        )
        composer.firstMatch.click()
        let editor = app.descendants(matching: .any)["seyal-composer-editor"]
        if editor.waitForExistence(timeout: 2), editor.firstMatch.isHittable {
            editor.firstMatch.click()
            editor.firstMatch.typeText("echo seyal-usable-gate")
            editor.firstMatch.typeKey("\r", modifierFlags: [])
        } else {
            composer.firstMatch.typeText("echo seyal-usable-gate")
            app.typeKey("\r", modifierFlags: [])
        }
        XCTAssertEqual(app.state, .runningForeground, "Seyal.app crashed on composer submit")
        XCTAssertTrue(app.descendants(matching: .any)["seyal-thin-pane"].exists)
        XCTAssertTrue(app.descendants(matching: .any)["seyal-blocks"].waitForExistence(timeout: 5))
        // Command-block cards are Runtime timeline metadata (OSC 133 / SPEC-008),
        // not a copy of PTY bytes. Metal does not expose those bytes as AX text.
        // The host-observable proof is: attach stayed live and the composer
        // accepted the submit (draft cleared).
        let accepted = NSPredicate(format: "value == nil OR value == ''")
        let cleared = expectation(for: accepted, evaluatedWith: editor.firstMatch, handler: nil)
        XCTAssertEqual(
            XCTWaiter.wait(for: [cleared], timeout: 8),
            .completed,
            "composer submit did not accept the draft; editor=\(editor.firstMatch.value ?? "nil")"
        )
        waitForUsablePty(in: app, timeout: 8)
    }

    /// Metal does not expose PTY bytes as AX text. The live connection token on
    /// `terminal-input` is the host-observable proof that Runtime attached.
    private func waitForUsablePty(in app: XCUIApplication, timeout: TimeInterval = 20) {
        XCTAssertTrue(app.wait(for: .runningForeground, timeout: 10))
        let terminal = app.descendants(matching: .any)["terminal-input"]
        XCTAssertTrue(terminal.waitForExistence(timeout: 10), "terminal surface missing")
        let usable = NSPredicate(format: "value CONTAINS 'connection=usable'")
        let arrived = expectation(for: usable, evaluatedWith: terminal, handler: nil)
        let result = XCTWaiter.wait(for: [arrived], timeout: timeout)
        XCTAssertEqual(
            app.state,
            .runningForeground,
            "Seyal.app crashed while waiting for the PTY/runtime attach"
        )
        XCTAssertEqual(
            result,
            .completed,
            "PTY/runtime never became usable; terminal AX=\(terminal.value ?? "nil")"
        )
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
