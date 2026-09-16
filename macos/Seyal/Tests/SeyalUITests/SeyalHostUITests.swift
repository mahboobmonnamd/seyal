import XCTest

@MainActor
final class SeyalHostUITests: XCTestCase {
    override func setUp() {
        continueAfterFailure = false
    }

    /// Kill a leftover Seyal.app from the previous case before launch.
    /// Must run on the test's MainActor isolation, not XCTest's sync tearDown.
    private func hostedApp() -> XCUIApplication {
        let app = XCUIApplication()
        app.terminate()
        app.launch()
        return app
    }

    func testApplicationLaunchesOnePaneHost() throws {
        let app = hostedApp()
        XCTAssertTrue(app.wait(for: .runningForeground, timeout: 10))
        // Attach/resize used to recurse into SIGSEGV within ~250ms of a real
        // `open`. Chrome existence is not enough; the process must stay up.
        RunLoop.current.run(until: Date().addingTimeInterval(2))
        XCTAssertEqual(app.state, .runningForeground, "Seyal.app crashed after launch")
        let chrome = app.descendants(matching: .any)["seyal-product-chrome"]
        XCTAssertTrue(chrome.waitForExistence(timeout: 10))
        let pane = app.descendants(matching: .any)["seyal-thin-pane"]
        XCTAssertTrue(pane.waitForExistence(timeout: 10))
        XCTAssertTrue(app.descendants(matching: .any)["seyal-recovery"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.descendants(matching: .any)["terminal-input"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.descendants(matching: .any)["seyal-composer"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.descendants(matching: .any)["seyal-blocks"].waitForExistence(timeout: 5))
        let transcript = app.descendants(matching: .any)["seyal-blocks-scroll"]
        XCTAssertTrue(transcript.waitForExistence(timeout: 5))
        XCTAssertGreaterThan(
            transcript.firstMatch.frame.height,
            120,
            "Flow transcript must fill the Pane, not sit above a Metal viewport"
        )
        XCTAssertGreaterThan(
            transcript.firstMatch.frame.width,
            chrome.firstMatch.frame.width * 0.7,
            "Flow transcript must use the Pane width; sidebar/inspector stay receded"
        )
        XCTAssertFalse(
            app.descendants(matching: .any)["seyal-left-workspaces"].firstMatch.isHittable,
            "Flow shows composer and Blocks only"
        )
        waitForUsablePty(in: app)
    }

    func testFlowSurfaceIsComposerAndBlocks() throws {
        let app = hostedApp()
        XCTAssertTrue(app.descendants(matching: .any)["seyal-composer"].waitForExistence(timeout: 10))
        XCTAssertTrue(app.descendants(matching: .any)["seyal-blocks-scroll"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.descendants(matching: .any)["seyal-blocks"].waitForExistence(timeout: 5))
        XCTAssertFalse(app.descendants(matching: .any)["seyal-inspector"].firstMatch.isHittable)
        XCTAssertFalse(app.descendants(matching: .any)["seyal-tab-strip"].firstMatch.isHittable)
        XCTAssertFalse(app.descendants(matching: .any)["seyal-left-tabs"].firstMatch.isHittable)
        XCTAssertTrue(app.descendants(matching: .any)["seyal-composer-execute"].waitForExistence(timeout: 5))
    }

    func testComposerSubmitAndTerminalFocusStayOnRustEligibility() throws {
        let app = hostedApp()
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
        XCTAssertTrue(app.descendants(matching: .any)["seyal-blocks-scroll"].waitForExistence(timeout: 5))
        // Command blocks are a Rust composer projection of Runtime
        // timeline metadata (OSC 133 / SPEC-008), not PTY bytes copied into
        // Swift. Echo in the Metal pane is not a Block. This test proves the
        // host accepted the composer submit; Blocks appear when Runtime
        // publishes a timeline revision.
        let accepted = NSPredicate(format: "value == nil OR value == ''")
        let cleared = expectation(for: accepted, evaluatedWith: editor.firstMatch, handler: nil)
        XCTAssertEqual(
            XCTWaiter.wait(for: [cleared], timeout: 8),
            .completed,
            "composer submit did not accept the draft; editor=\(editor.firstMatch.value ?? "nil")"
        )
        waitForUsablePty(in: app, timeout: 8)
        let output = app.descendants(matching: .any)["seyal-block-0-body"]
        if output.waitForExistence(timeout: 6) {
            XCTAssertGreaterThan(
                output.firstMatch.frame.height,
                8,
                "Block body must reserve a Metal output region, not only the command header"
            )
        }
    }

    func testAlternateScreenTakeoverDoesNotCrashTheHost() throws {
        let app = hostedApp()
        waitForUsablePty(in: app)
        let composer = app.descendants(matching: .any)["seyal-composer"]
        XCTAssertTrue(composer.waitForExistence(timeout: 12))
        let composerReady = NSPredicate(format: "value == 'available'")
        let becameReady = expectation(for: composerReady, evaluatedWith: composer, handler: nil)
        XCTAssertEqual(
            XCTWaiter.wait(for: [becameReady], timeout: 12),
            .completed,
            "composer never became available; value=\(composer.value ?? "nil")"
        )
        composer.firstMatch.click()
        let editor = app.descendants(matching: .any)["seyal-composer-editor"]
        if editor.waitForExistence(timeout: 2), editor.firstMatch.isHittable {
            editor.firstMatch.click()
            editor.firstMatch.typeText("printf '\\033[?1049h'")
            editor.firstMatch.typeKey("\r", modifierFlags: [])
        } else {
            composer.firstMatch.typeText("printf '\\033[?1049h'")
            app.typeKey("\r", modifierFlags: [])
        }
        RunLoop.current.run(until: Date().addingTimeInterval(2))
        XCTAssertEqual(
            app.state,
            .runningForeground,
            "Seyal.app crashed entering alternate-screen/TUI takeover"
        )
        XCTAssertTrue(app.descendants(matching: .any)["seyal-thin-pane"].exists)
        // Leave Flow, not TUI. A surviving Runtime helper is reused by the next
        // launch; leftover alternate-screen hides the composer and fails later tests.
        let terminal = app.descendants(matching: .any)["terminal-input"]
        XCTAssertTrue(terminal.waitForExistence(timeout: 5))
        terminal.firstMatch.click()
        app.typeText("printf '\\033[?1049l'")
        app.typeKey("\r", modifierFlags: [])
        XCTAssertTrue(
            composer.waitForExistence(timeout: 12),
            "composer did not return after leaving alternate-screen"
        )
    }

    /// Metal does not expose PTY bytes as AX text. The live connection token on
    /// `terminal-input` is the host-observable proof that Runtime attached.

    func testComposerHistoryRecallOpensFiltersInsertsAndDismisses() throws {
        let app = hostedApp()
        waitForUsablePty(in: app)
        let composer = app.descendants(matching: .any)["seyal-composer"]
        XCTAssertTrue(composer.waitForExistence(timeout: 12))
        let composerReady = NSPredicate(format: "value == 'available'")
        let becameReady = expectation(for: composerReady, evaluatedWith: composer, handler: nil)
        XCTAssertEqual(XCTWaiter.wait(for: [becameReady], timeout: 12), .completed)
        let editor = app.descendants(matching: .any)["seyal-composer-editor"]
        XCTAssertTrue(editor.waitForExistence(timeout: 5))
        editor.firstMatch.click()
        let overlay = app.descendants(matching: .any)["seyal-composer-history"]
        let toggle = app.descendants(matching: .any)["seyal-composer-history-toggle"]
        XCTAssertTrue(toggle.waitForExistence(timeout: 5))
        XCTAssertFalse(toggle.firstMatch.isEnabled, "no accepted submit yet: recall is disabled")
        editor.firstMatch.typeKey("r", modifierFlags: [.control])
        XCTAssertFalse(overlay.firstMatch.isHittable, "Rust rejects open with empty history")

        editor.firstMatch.typeText("echo seyal-history-recall")
        editor.firstMatch.typeKey("\r", modifierFlags: [])
        let cleared = expectation(
            for: NSPredicate(format: "value == nil OR value == ''"),
            evaluatedWith: editor.firstMatch,
            handler: nil
        )
        XCTAssertEqual(XCTWaiter.wait(for: [cleared], timeout: 8), .completed)
        let readyAgain = expectation(for: composerReady, evaluatedWith: composer, handler: nil)
        XCTAssertEqual(XCTWaiter.wait(for: [readyAgain], timeout: 12), .completed)

        editor.firstMatch.click()
        editor.firstMatch.typeKey("r", modifierFlags: [.control])
        XCTAssertTrue(overlay.waitForExistence(timeout: 5))
        XCTAssertTrue(overlay.firstMatch.isHittable, "⌃R opens the Rust history overlay")
        let query = app.descendants(matching: .any)["seyal-composer-history-query"]
        XCTAssertTrue(query.waitForExistence(timeout: 5))
        let row = app.descendants(matching: .any)["seyal-composer-history-row-0"]
        XCTAssertTrue(row.waitForExistence(timeout: 5))
        XCTAssertEqual(row.firstMatch.label, "echo seyal-history-recall")

        query.firstMatch.typeText("zzz-no-match")
        let noRows = expectation(for: NSPredicate(format: "value == '0'"), evaluatedWith: overlay.firstMatch, handler: nil)
        XCTAssertEqual(XCTWaiter.wait(for: [noRows], timeout: 5), .completed, "type-to-filter runs in Rust")
        app.typeKey(.escape, modifierFlags: [])
        let dismissed = expectation(for: NSPredicate(format: "isHittable == false"), evaluatedWith: overlay.firstMatch, handler: nil)
        XCTAssertEqual(XCTWaiter.wait(for: [dismissed], timeout: 5), .completed, "Escape closes the overlay")

        toggle.firstMatch.click()
        XCTAssertTrue(overlay.firstMatch.waitForExistence(timeout: 5))
        query.firstMatch.typeText("recall")
        app.typeKey("\r", modifierFlags: [])
        let inserted = expectation(
            for: NSPredicate(format: "value == 'echo seyal-history-recall'"),
            evaluatedWith: editor.firstMatch,
            handler: nil
        )
        XCTAssertEqual(XCTWaiter.wait(for: [inserted], timeout: 5), .completed, "Enter inserts into the draft")
        XCTAssertFalse(overlay.firstMatch.isHittable)
        XCTAssertEqual(app.state, .runningForeground)
    }

    func testSelectingABlockRevealsRustBlockDetailsInInspector() throws {
        let app = hostedApp()
        waitForUsablePty(in: app)
        let composer = app.descendants(matching: .any)["seyal-composer"]
        XCTAssertTrue(composer.waitForExistence(timeout: 12))
        let composerReady = NSPredicate(format: "value == 'available'")
        let becameReady = expectation(for: composerReady, evaluatedWith: composer, handler: nil)
        XCTAssertEqual(XCTWaiter.wait(for: [becameReady], timeout: 12), .completed)
        let inspector = app.descendants(matching: .any)["seyal-inspector"]
        XCTAssertFalse(inspector.firstMatch.isHittable, "inspector is receded before any selection")
        let editor = app.descendants(matching: .any)["seyal-composer-editor"]
        XCTAssertTrue(editor.waitForExistence(timeout: 5))
        editor.firstMatch.click()
        editor.firstMatch.typeText("echo seyal-block-details")
        editor.firstMatch.typeKey("\r", modifierFlags: [])
        let cleared = expectation(
            for: NSPredicate(format: "value == nil OR value == ''"),
            evaluatedWith: editor.firstMatch,
            handler: nil
        )
        XCTAssertEqual(XCTWaiter.wait(for: [cleared], timeout: 8), .completed)

        // Blocks are Runtime timeline metadata (OSC 133); the card appears when
        // Runtime publishes the revision. Without shell integration there is no
        // Block and therefore nothing to select: the test proves the host never
        // fabricates one.
        let card = app.descendants(matching: .any)["seyal-block-0"]
        guard card.waitForExistence(timeout: 10) else {
            XCTAssertFalse(inspector.firstMatch.isHittable, "no Block, no Block details")
            return
        }
        card.firstMatch.click()
        XCTAssertTrue(inspector.waitForExistence(timeout: 5))
        let revealed = expectation(for: NSPredicate(format: "isHittable == true"), evaluatedWith: inspector.firstMatch, handler: nil)
        XCTAssertEqual(XCTWaiter.wait(for: [revealed], timeout: 5), .completed, "selecting a Block reveals the inspector")
        let selected = expectation(for: NSPredicate(format: "value == 'selected'"), evaluatedWith: card.firstMatch, handler: nil)
        XCTAssertEqual(XCTWaiter.wait(for: [selected], timeout: 5), .completed, "card reflects the Rust selected flag")
        let commandRow = inspector.descendants(matching: .staticText)["Block · Command"]
        XCTAssertTrue(commandRow.waitForExistence(timeout: 5), "inspector shows Rust Block rows")
        XCTAssertTrue(inspector.descendants(matching: .staticText)["echo seyal-block-details"].waitForExistence(timeout: 5))
        XCTAssertFalse(inspector.descendants(matching: .staticText)["Block · Duration"].exists, "no fabricated telemetry")

        card.firstMatch.click()
        let deselected = expectation(for: NSPredicate(format: "value == nil OR value == ''"), evaluatedWith: card.firstMatch, handler: nil)
        XCTAssertEqual(XCTWaiter.wait(for: [deselected], timeout: 5), .completed, "clicking again clears the selection")
        XCTAssertFalse(commandRow.exists, "Block rows leave with the selection")
        XCTAssertEqual(app.state, .runningForeground)
    }

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
        assertFlowBlocksOrFail(in: app)
    }

    /// Headed XCUI must stay on Flow/Blocks. A leftover Runtime in alternate
    /// screen, or a Raw-only launch, looks like a normal terminal and is a fail.
    private func assertFlowBlocksOrFail(in app: XCUIApplication, timeout: TimeInterval = 8) {
        let composer = app.descendants(matching: .any)["seyal-composer"]
        let blocks = app.descendants(matching: .any)["seyal-blocks"]
        let transcript = app.descendants(matching: .any)["seyal-blocks-scroll"]
        XCTAssertTrue(
            composer.waitForExistence(timeout: timeout),
            "XCUI must run as Flow/Blocks; composer missing — UI looks like a normal terminal"
        )
        XCTAssertTrue(
            composer.firstMatch.isHittable,
            "XCUI must run as Flow/Blocks; composer not hittable — UI looks like a normal terminal"
        )
        XCTAssertTrue(
            blocks.waitForExistence(timeout: 5),
            "XCUI must run as Flow/Blocks; seyal-blocks missing — UI looks like a normal terminal"
        )
        XCTAssertTrue(
            transcript.waitForExistence(timeout: 5),
            "XCUI must run as Flow/Blocks; transcript missing — UI looks like a normal terminal"
        )
        XCTAssertGreaterThan(
            transcript.firstMatch.frame.height,
            120,
            "Flow transcript must fill the Pane; a short or hidden transcript is a raw-terminal launch"
        )
        XCTAssertFalse(
            app.descendants(matching: .any)["seyal-left-workspaces"].firstMatch.isHittable,
            "Flow shows composer and Blocks only"
        )
    }

    /// Headed #823 smoke: composer submit plus Cmd-C / ArrowUp must stay on
    /// Flow/Blocks. This is not a TerminalKeyV2 byte oracle; headed key-to-PTY
    /// and the six-step IME/Neovim/TUI matrix remain open.
    func testComposerSubmitAndHostShortcutsStayOnFlowBlocks() throws {
        let app = hostedApp()
        waitForUsablePty(in: app)
        assertFlowBlocksOrFail(in: app)

        let composer = app.descendants(matching: .any)["seyal-composer"]
        XCTAssertTrue(composer.waitForExistence(timeout: 12))
        let composerReady = NSPredicate(format: "value == 'available'")
        let becameReady = expectation(for: composerReady, evaluatedWith: composer, handler: nil)
        XCTAssertEqual(
            XCTWaiter.wait(for: [becameReady], timeout: 12),
            .completed,
            "composer never became available; value=\(composer.value ?? "nil")"
        )
        composer.firstMatch.click()
        let editor = app.descendants(matching: .any)["seyal-composer-editor"]
        if editor.waitForExistence(timeout: 2), editor.firstMatch.isHittable {
            editor.firstMatch.click()
            editor.firstMatch.typeText("echo seyal-823-flow-blocks")
            editor.firstMatch.typeKey("\r", modifierFlags: [])
        } else {
            composer.firstMatch.typeText("echo seyal-823-flow-blocks")
            app.typeKey("\r", modifierFlags: [])
        }
        XCTAssertEqual(app.state, .runningForeground, "Seyal.app crashed on composer submit")
        assertFlowBlocksOrFail(in: app)

        app.typeKey("c", modifierFlags: .command)
        waitBriefly(0.3)
        app.typeKey(.upArrow, modifierFlags: [])
        XCTAssertEqual(app.state, .runningForeground, "Seyal.app crashed on Cmd-C / ArrowUp")
        assertFlowBlocksOrFail(in: app)
        XCTAssertTrue(
            composer.firstMatch.isHittable,
            "Cmd-C / ArrowUp must not leave Flow/Blocks for a raw terminal"
        )
    }

    func testCopyPasteAndQuitMenusAreWired() throws {
        let app = hostedApp()
        XCTAssertTrue(app.wait(for: .runningForeground, timeout: 10))
        waitForUsablePty(in: app)
        app.typeKey("c", modifierFlags: .command)
        app.typeKey("v", modifierFlags: .command)
        XCTAssertEqual(app.state, .runningForeground, "Seyal.app crashed on Cmd-C / Cmd-V")
        assertFlowBlocksOrFail(in: app)
        app.typeKey("q", modifierFlags: .command)
        XCTAssertTrue(app.wait(for: .notRunning, timeout: 8))
    }

    func testHostMouseClickStaysOnFlowBlocks() throws {
        let app = hostedApp()
        XCTAssertTrue(app.wait(for: .runningForeground, timeout: 10))
        waitForUsablePty(in: app)
        let surface = app.descendants(matching: .any)["terminal-input"]
        XCTAssertTrue(surface.waitForExistence(timeout: 5))
        surface.firstMatch.click()
        XCTAssertEqual(app.state, .runningForeground, "Seyal.app crashed on host mouse click")
        assertFlowBlocksOrFail(in: app)
    }

    func testCommandPaletteOpensFiltersRunsAndDismisses() throws {
        let app = hostedApp()
        waitForUsablePty(in: app)
        let inspector = app.descendants(matching: .any)["seyal-inspector"]
        XCTAssertFalse(inspector.firstMatch.isHittable, "inspector is receded before any command runs")

        let palette = app.descendants(matching: .any)["seyal-command-palette"]
        app.typeKey("k", modifierFlags: .command)
        XCTAssertTrue(palette.waitForExistence(timeout: 5), "⌘K opens the Rust-backed palette")
        XCTAssertTrue(palette.firstMatch.isHittable)

        let query = app.descendants(matching: .any)["seyal-command-palette-query"]
        XCTAssertTrue(query.waitForExistence(timeout: 5))
        query.firstMatch.click()
        query.firstMatch.typeText("Show Inspector")
        let row = app.descendants(matching: .any)["seyal-command-palette-row-0"]
        let filtered = expectation(
            for: NSPredicate(format: "exists == true"),
            evaluatedWith: row,
            handler: nil
        )
        XCTAssertEqual(XCTWaiter.wait(for: [filtered], timeout: 5), .completed, "type-to-filter runs in Rust")
        XCTAssertEqual(row.label, "Show Inspector")

        app.typeKey("\r", modifierFlags: [])
        let dismissed = expectation(
            for: NSPredicate(format: "isHittable == false"),
            evaluatedWith: palette.firstMatch,
            handler: nil
        )
        XCTAssertEqual(XCTWaiter.wait(for: [dismissed], timeout: 5), .completed, "Enter runs the row and closes")
        let revealed = expectation(
            for: NSPredicate(format: "isHittable == true"),
            evaluatedWith: inspector.firstMatch,
            handler: nil
        )
        XCTAssertEqual(
            XCTWaiter.wait(for: [revealed], timeout: 5),
            .completed,
            "the resolved command actually ran, not just an overlay animation"
        )
        XCTAssertEqual(app.state, .runningForeground)
    }

    func testCommandPaletteEscapeAndClickOutsideBothDismissWithoutRunning() throws {
        let app = hostedApp()
        waitForUsablePty(in: app)
        let palette = app.descendants(matching: .any)["seyal-command-palette"]
        let scrim = app.descendants(matching: .any)["seyal-command-palette-scrim"]

        app.typeKey("k", modifierFlags: .command)
        XCTAssertTrue(palette.waitForExistence(timeout: 5))
        app.typeKey(.escape, modifierFlags: [])
        let closedByEscape = expectation(
            for: NSPredicate(format: "isHittable == false"),
            evaluatedWith: palette.firstMatch,
            handler: nil
        )
        XCTAssertEqual(XCTWaiter.wait(for: [closedByEscape], timeout: 5), .completed)

        app.typeKey("k", modifierFlags: .command)
        XCTAssertTrue(palette.waitForExistence(timeout: 5))
        // Click near the top-left corner of the scrim, well outside the
        // centered card, to exercise click-outside-to-close.
        scrim.firstMatch.coordinate(withNormalizedOffset: CGVector(dx: 0.02, dy: 0.02)).click()
        let closedByClick = expectation(
            for: NSPredicate(format: "isHittable == false"),
            evaluatedWith: palette.firstMatch,
            handler: nil
        )
        XCTAssertEqual(XCTWaiter.wait(for: [closedByClick], timeout: 5), .completed)
        XCTAssertEqual(app.state, .runningForeground)
    }

    private func waitBriefly(_ seconds: TimeInterval) {
        RunLoop.current.run(until: Date().addingTimeInterval(seconds))
    }
}
